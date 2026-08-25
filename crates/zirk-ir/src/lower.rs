//! Lowering from the verified tree into IR.
//!
//! The input is a tree whose names are resolved and whose types are checked, so
//! **nothing is re-checked here**. Expression types are derived from the shape
//! of the tree, which is total on a verified program: a literal knows its type,
//! a path reads it from its slot, an operator computes it from its operands,
//! and a call from its signature.
//!
//! Deriving is not checking. Everything the checker would have rejected never
//! reaches this point.

use crate::ir::*;
use std::collections::HashMap;
use zirk_ast as ast;
use zirk_diagnostics::Span;
use zirk_sema::{
    AssociatedFieldInfo, Base, Capture, CheckedProgram, ClassType, EnumType, EnumVariantInfo,
    FieldInfo, FloatWidth as SemaFloatWidth, FnType, IntWidth as SemaIntWidth, MethodInfo,
    ParamInfo, Type,
};

/// The symbol every `abstract class`'s own method-table slot names (roadmap
/// Phase 4b) — see `lower`'s own doc comment on the dummy function it
/// points to.
const UNREACHABLE_ABSTRACT_METHOD: &str = "zk.unreachable_abstract_method";

/// Lowers a verified program into an IR module.
pub fn lower(program: &ast::Program, checked: &CheckedProgram) -> Module {
    // Specialization comes first (roadmap task 11.1, D5): one concrete copy
    // of a generic class per combination of type arguments the program
    // actually uses (`checked.generic_instances`, deduplicated by the
    // checker's own interning — task 11.2's "reuse when repeated" is already
    // free there). Appended after every ordinary class, so `instance_base`
    // is where `Base::Instance(id)` always lands: `instance_base + id`.
    let instance_base = checked.classes.len() as u32;
    let mut checked = checked.clone();
    let specialized: Vec<ClassType> = checked
        .generic_instances
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, instance)| specialize_class(&checked, instance, index))
        .collect();
    checked.classes.extend(specialized);

    // The same specialization, for a generic enum's own instantiations
    // (roadmap task 13.5: `Iteration<T>` is the only one a program can name
    // today, `resolve_enum_reference`'s own `NOT_LOWERED` gate keeps any
    // other generic enum from reaching this point). Unlike a contract's
    // dispatch table, an enum's representation depends on `T` — its payload
    // is inline — so it needs a concrete `EnumLayout` the same way a generic
    // class's fields do.
    let enum_instance_base = checked.enums.len() as u32;
    let specialized_enums: Vec<EnumType> = checked
        .enum_instances
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, instance)| specialize_enum(&checked, instance, index))
        .collect();
    checked.enums.extend(specialized_enums);
    let checked = &checked;

    // The layouts come first: a function that builds an object needs the
    // layout it allocates, and a field access needs its offsets.
    //
    // A record or value class lives in `values`, not `objects` — its
    // `ObjectLayout` entry here is an empty placeholder that is never read
    // (`IrType::Value`, not `Object`, is what a value-kind class resolves
    // to; see `ir_type`), the same way a still-generic template's is.
    let objects = checked
        .classes
        .iter()
        .enumerate()
        .map(|(id, class)| {
            let id = id as u32;
            // A generic class's own template — as opposed to one of the
            // specialized copies just appended, which have none of their
            // own — is never instantiated as itself: `this` inside a
            // specialized copy's body resolves through that copy's id, not
            // the template's (see `run_constructor`/`run_method` below), so
            // nothing ever addresses this layout. Left empty rather than
            // built: its fields still name `T` (`Base::Param`), which
            // `ir_type` has nothing to substitute here, and its methods'
            // bodies are never lowered (skipped in the loop below), so a
            // symbol in its table would name a function that does not exist.
            if !class.type_params.is_empty()
                || matches!(
                    class.kind,
                    ast::ClassKind::Record | ast::ClassKind::ValueClass
                )
            {
                return ObjectLayout {
                    name: class.name.clone(),
                    fields: Vec::new(),
                    methods: Vec::new(),
                    contracts: Vec::new(),
                    ancestors: Vec::new(),
                };
            }

            // Itself, then every base transitively — what a checked cast
            // searches (roadmap task 11.6). A specialized generic copy
            // never `extends` (gated at its own declaration, task 11.1), so
            // this is always just itself for one.
            //
            // Also every `implements`-adopted `abstract class`, transitively
            // through its own `abstract_bases` (roadmap Phase 4b): `catch
            // Throwable(e)` needs a *runtime* test that a concrete
            // exception's actual class is `Throwable` or a descendant, and
            // `abstract_bases` alone is not transitive the way `extends` is
            // (`Checker::implements_abstract_class`'s own doc comment on
            // exactly this — `class Foo implements RuntimeError` gives
            // `Foo.abstract_bases` only `[runtime_error]`, not
            // `[error, throwable, runtime_error]`). Nothing before this
            // phase ever tested an object against an abstract-class target
            // at runtime, so widening `ancestors` to include these is purely
            // additive.
            let mut ancestors = vec![id];
            let mut current = class.base;
            while let Some(base_id) = current {
                ancestors.push(base_id);
                current = checked.classes[base_id as usize].base;
            }
            let mut frontier = class.abstract_bases.clone();
            while let Some(abstract_id) = frontier.pop() {
                if ancestors.contains(&abstract_id) {
                    continue;
                }
                ancestors.push(abstract_id);
                frontier.extend(checked.classes[abstract_id as usize].abstract_bases.clone());
            }

            ObjectLayout {
                name: class.name.clone(),
                ancestors,
                fields: class
                    .fields
                    .iter()
                    .map(|field| ObjectField {
                        name: field.name.clone(),
                        ty: ir_type(field.ty, instance_base, enum_instance_base, checked),
                    })
                    .collect(),
                // Ordered by index, which is what makes the table of a
                // subclass start with its base's. An `abstract class`'s own
                // entries all point at the shared dummy body instead
                // (`UNREACHABLE_ABSTRACT_METHOD`'s own doc comment) — none
                // of them has a real one.
                methods: {
                    let mut table = vec![String::new(); class.methods.len()];
                    for method in &class.methods {
                        table[method.index] = if class.kind == ast::ClassKind::Abstract {
                            UNREACHABLE_ABSTRACT_METHOD.to_string()
                        } else {
                            body_symbol(checked, method)
                        };
                    }
                    table
                },
                // One table per contract, in the contract's own method
                // order, so a call through it indexes the same way whatever
                // class answers.
                contracts: class
                    .contracts
                    .iter()
                    .map(|id| ContractTable {
                        contract: *id,
                        methods: checked.contracts[*id as usize]
                            .methods
                            .iter()
                            .map(|required| {
                                let supplied = class
                                    .method(&required.name)
                                    .expect("the checker verified conformance");
                                body_symbol(checked, supplied)
                            })
                            .collect(),
                    })
                    .collect(),
            }
        })
        .collect();

    // A record or value class's own layout — indexed the same way `objects`
    // is, sharing `checked.classes`' id space (roadmap task 11.5); an
    // ordinary class's or a still-generic template's entry here is the
    // empty placeholder, symmetric with `objects` above.
    let values = checked
        .classes
        .iter()
        .map(|class| {
            if !class.type_params.is_empty()
                || !matches!(
                    class.kind,
                    ast::ClassKind::Record | ast::ClassKind::ValueClass
                )
            {
                return ValueLayout {
                    name: class.name.clone(),
                    fields: Vec::new(),
                };
            }
            ValueLayout {
                name: class.name.clone(),
                fields: class
                    .fields
                    .iter()
                    .map(|field| ObjectField {
                        name: field.name.clone(),
                        ty: ir_type(field.ty, instance_base, enum_instance_base, checked),
                    })
                    .collect(),
            }
        })
        .collect();

    // An algebraic enum's own layout, indexed the same way `checked.enums`
    // is (roadmap task 11.3). A traditional enum (no payload anywhere) or a
    // still-generic one gets the empty placeholder: neither is ever
    // addressed as `IrType::Enum` — `ir_type` only produces that once
    // `enum_has_payload` is true, and a generic enum is separately gated at
    // its own declaration.
    let enums = checked
        .enums
        .iter()
        .map(|e| {
            // A specialized copy whose own argument was itself still a bare
            // `T` (roadmap task 13.5) is not a real instantiation: the
            // native `Iterable`/`Iterator` contracts intern one for their
            // own template signatures (`iterator(): Iterator<T>`,
            // `next(): Iteration<T>`) unconditionally, before any program
            // is checked, so it exists in the table whether or not a
            // program ever names a concrete `Iteration<T>` — nothing real
            // ever addresses it as `Base::EnumInstance`, so it gets the
            // same empty placeholder a still-generic template does.
            let unresolved = e
                .variants
                .iter()
                .flat_map(|v| &v.associated)
                .any(|f| matches!(f.ty.base, Base::Param(_)));
            if !e.type_params.is_empty()
                || unresolved
                || !e.variants.iter().any(|v| !v.associated.is_empty())
            {
                return EnumLayout {
                    name: e.name.clone(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                };
            }

            let mut fields = Vec::new();
            let mut variants = Vec::new();
            for variant in &e.variants {
                let mut indices = Vec::new();
                for field in &variant.associated {
                    indices.push(fields.len() as u32);
                    fields.push(ObjectField {
                        name: field.name.clone(),
                        ty: ir_type(field.ty, instance_base, enum_instance_base, checked),
                    });
                }
                variants.push(indices);
            }
            EnumLayout {
                name: e.name.clone(),
                fields,
                variants,
            }
        })
        .collect();

    let mut module = Module {
        objects,
        values,
        enums,
        ..Module::default()
    };

    // `checked.pointer_types` is pushed first, in the same order, so
    // `Base::Pointer(id)`/`IrType::Pointer(id)` always name the same
    // pointee — see `ir_type`'s own comment on this arm.
    module.pointer_types = checked
        .pointer_types
        .iter()
        .map(|&pointee| ir_type(pointee, instance_base, enum_instance_base, checked))
        .collect();

    // `extern "C" fn` declarations (roadmap Phase 4e, design D7, `ADR-015`):
    // no body to lower, only a declaration codegen turns into an LLVM
    // `declare`.
    module.externs = program
        .externs
        .iter()
        .filter_map(|e| {
            let signature = checked.externs.get(&e.name.name)?;
            Some(ExternFn {
                name: e.name.name.clone(),
                params: signature
                    .params
                    .iter()
                    .map(|&t| ir_type(t, instance_base, enum_instance_base, checked))
                    .collect(),
                return_type: ir_type(
                    signature.returns,
                    instance_base,
                    enum_instance_base,
                    checked,
                ),
            })
        })
        .collect();

    // One entry per `checked.fn_types` id, pushed first and in order so
    // `Base::Function(id)` and `IrType::Closure(id)` share the same number
    // (`Self::ir_type`'s own `Base::Function` arm relies on this).
    //
    // Most ids get their real content right here: the uniform, capture-less
    // `ClosureLayout` design D12 gives a named function reference or a
    // capture-less lambda — `{function pointer}`, nothing else — shared by
    // every value of that shape (`Self::lower_expr`'s `ast::Expr::Path` arm,
    // `Self::lower_lambda`'s capture-less branch). `function` is left empty:
    // nothing calls through a canonical layout's *own* stored target —
    // `InstKind::MakeClosure` carries the target it embeds explicitly
    // (`MakeClosure::target`), precisely so many differently-targeted values
    // (`double`, `triple`, ...) can share one canonical shape instead of
    // each needing a layout of its own.
    //
    // An id a *capturing* lambda literal owns (design D14) instead gets a
    // placeholder here — its real capture types are only known once that
    // one lambda occurrence is actually lowered in its enclosing function's
    // own body (each capture's type comes from the *slot* it lives in
    // there, not from anything this pre-pass can see) — `Self::lower_lambda`
    // overwrites `module.closures[id]` in place when it reaches it.
    let capturing_fn_types: std::collections::HashSet<u32> = checked
        .lambdas
        .values()
        .filter(|info| !info.captures.is_empty())
        .map(|info| info.fn_type)
        .collect();

    for (index, shape) in checked.fn_types.iter().enumerate() {
        if capturing_fn_types.contains(&(index as u32)) {
            module.closures.push(ClosureLayout {
                captures: Vec::new(),
                params: Vec::new(),
                returns: IrType::Void,
            });
            continue;
        }
        module.closures.push(ClosureLayout {
            captures: Vec::new(),
            params: shape
                .params
                .iter()
                .map(|t| ir_type(*t, instance_base, enum_instance_base, checked))
                .collect(),
            returns: ir_type(shape.returns, instance_base, enum_instance_base, checked),
        });
    }

    // A dummy body every `abstract class`'s own (never-invoked) method-table
    // slot points to (roadmap Phase 4b) — the class itself is never
    // allocated, `Checker`'s own construction check sees to that, so no real
    // body exists for `message`/`stack_trace`/etc. to name. Dead data at
    // runtime: dispatch through a value statically typed as an abstract
    // class (`catch Throwable(e)`, `e.message()`) is `InstKind::CallVirtual`,
    // which reads the pointer out of the *concrete* object's own descriptor,
    // never this one's. But codegen still builds a descriptor for every
    // `ObjectLayout` unconditionally, and that build needs every symbol its
    // table names to actually exist — one shared, empty function is cheaper
    // than teaching that pass which layouts are provably dead.
    module.functions.push(Function {
        name: UNREACHABLE_ABSTRACT_METHOD.to_string(),
        params: Vec::new(),
        return_type: IrType::Void,
        slots: Vec::new(),
        blocks: vec![Block {
            id: BlockId(0),
            instructions: Vec::new(),
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span: Span::empty(0),
        gc_roots: Vec::new(),
    });

    // Default values are written in the declaration but evaluated at the call
    // site, so the lowering needs the declarations while lowering the calls.
    let declarations: HashMap<&str, &ast::FnDecl> = program
        .functions
        .iter()
        .map(|f| (f.name.name.as_str(), f))
        .collect();

    // The sixteen hand-built method bodies of the four native failure
    // classes (`fase-4d-runtimeerror`, D8) — the same "no `program.classes`
    // entry, build the IR directly" shape `UNREACHABLE_ABSTRACT_METHOD`
    // above already uses, just with sixteen real (reachable) bodies instead
    // of one unreachable one. `construct` needs no body here: a
    // `Type("reason")` construction of one of these four is handled by
    // `Self::lower_construction`'s own special case, the same way
    // `StackTrace()` already skips calling a constructor symbol.
    synthesize_native_failure_bodies(
        &mut module,
        checked,
        &declarations,
        instance_base,
        enum_instance_base,
    );

    // A constructor becomes an ordinary function whose first parameter is the
    // object being built. Emitting it once and calling it beats inlining the
    // body at every construction site, and it is the same shape a method call
    // will need.
    for class in &program.classes {
        let Some(id) = checked
            .classes
            .iter()
            .position(|c| c.name == class.name.name)
        else {
            continue;
        };

        // The template itself is never lowered — see the comment on its
        // (empty) `ObjectLayout` above. Each specialization below lowers
        // this same declaration's AST again, once per instantiation, under
        // its own id: the type-level substitution already happened when
        // `specialize_class` built its `ClassType`, so what changes here is
        // only which class id `this`/a field/a method resolves against.
        if !checked.classes[id].type_params.is_empty() {
            continue;
        }

        lower_class_body(
            &mut module,
            checked,
            &declarations,
            class,
            id as u32,
            instance_base,
            enum_instance_base,
        );
    }

    for (index, instance) in checked.generic_instances.iter().enumerate() {
        let specialized_id = instance_base + index as u32;
        let original = &program.classes[class_ast_index(program, checked, instance.class)];
        lower_class_body(
            &mut module,
            checked,
            &declarations,
            original,
            specialized_id,
            instance_base,
            enum_instance_base,
        );
    }

    // A trait's default body belongs to the contract, not to any class that
    // adopts it: one body, however many classes reuse it.
    for contract in &program.contracts {
        let Some(id) = checked
            .contracts
            .iter()
            .position(|c| c.name == contract.name.name)
        else {
            continue;
        };
        for method in &contract.methods {
            let Some(body) = &method.body else { continue };
            let lowering = FunctionLowering::new(
                &mut module,
                checked,
                &declarations,
                instance_base,
                enum_instance_base,
            );
            let (lowered, lifted) =
                lowering.run_contract_default(contract, method, body, id as u32);
            module.functions.push(lowered);
            module.functions.extend(lifted);
        }
    }

    for function in &program.functions {
        let lowering = FunctionLowering::new(
            &mut module,
            checked,
            &declarations,
            instance_base,
            enum_instance_base,
        );
        let (lowered, lifted) = lowering.run(function);
        module.functions.push(lowered);
        // Lambdas become module functions of their own: the closure value only
        // carries a pointer to one plus the captures. Decision D10.
        module.functions.extend(lifted);
    }

    module
}

/// Builds the sixteen method bodies of the four native failure classes by
/// hand (`fase-4d-runtimeerror`, design D8): `message`/`code`/`cause`/
/// `stack_trace` for `DivisionByZeroError`, `InvalidShiftError`,
/// `InvalidRepeatError` and `FloatNanError`. None of the four has a
/// `program.classes` entry — `Checker::register_native_exception_hierarchy`
/// injects them the same "table directly" way `Error`/`Throwable`/
/// `RuntimeError`/`StackTrace` already are — so nothing in the ordinary
/// per-class lowering loop below ever reaches them; this is their only
/// source of a real body.
fn synthesize_native_failure_bodies<'a>(
    module: &mut Module,
    checked: &'a CheckedProgram,
    declarations: &HashMap<&'a str, &'a ast::FnDecl>,
    instance_base: u32,
    enum_instance_base: u32,
) {
    let Some(native) = checked.native_exceptions else {
        return;
    };

    let classes = [
        (native.division_by_zero, "E_DIVISION_BY_ZERO"),
        (native.invalid_shift, "E_INVALID_SHIFT"),
        (native.invalid_repeat, "E_INVALID_REPEAT"),
        (native.float_nan, "E_FLOAT_NAN"),
    ];

    for (class_id, code) in classes {
        let message = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        )
        .build_native_message(class_id);
        module.functions.push(message);

        let code_fn = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        )
        .build_native_code(class_id, code);
        module.functions.push(code_fn);

        let cause = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        )
        .build_native_cause(class_id, native.error);
        module.functions.push(cause);

        let stack_trace = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        )
        .build_native_stack_trace(class_id, native.stack_trace);
        module.functions.push(stack_trace);
    }
}

/// Lowers every constructor and method of one class declaration under `id` —
/// shared between an ordinary class and a generic one's specialized copy,
/// which relowers the same AST under its own id (see callers).
fn lower_class_body<'a>(
    module: &mut Module,
    checked: &'a CheckedProgram,
    declarations: &HashMap<&'a str, &'a ast::FnDecl>,
    class: &ast::ClassDecl,
    id: u32,
    instance_base: u32,
    enum_instance_base: u32,
) {
    for (index, constructor) in class.constructors.iter().enumerate() {
        let lowering = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        );
        let (lowered, lifted) = lowering.run_constructor(class, constructor, id, index);
        module.functions.push(lowered);
        module.functions.extend(lifted);
    }

    // A method is the same shape as a constructor: a function whose first
    // parameter is the receiver.
    for (index, method) in class.methods.iter().enumerate() {
        let Some(body) = &method.body else { continue };
        let lowering = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        );
        let (lowered, lifted) = lowering.run_method(class, method, body, id, index);
        module.functions.push(lowered);
        module.functions.extend(lifted);
    }
}

/// The AST declaration a checked class id names, by matching its name — the
/// same lookup the ordinary (non-generic) loop above does.
fn class_ast_index(program: &ast::Program, checked: &CheckedProgram, id: u32) -> usize {
    let name = &checked.classes[id as usize].name;
    program
        .classes
        .iter()
        .position(|c| &c.name.name == name)
        .expect("a checked class is declared in the program")
}

/// One generic class's own `T` replaced by one instantiation's concrete
/// arguments (roadmap task 11.1) — a specialized `ClassType` lowering treats
/// exactly like an ordinary, non-generic one from here on.
///
/// Only a direct reference substitutes (`T` itself): the checker's
/// `generic_class_is_directly_specializable` guarantees nothing here is
/// nested inside another generic type, so no recursion is needed the way the
/// checker's own `substitute_type` (for `Iterable<T>` and friends) needs.
fn specialize_class(
    checked: &CheckedProgram,
    instance: zirk_sema::GenericInstance,
    index: usize,
) -> ClassType {
    let original = &checked.classes[instance.class as usize];
    let subst: Vec<(u32, Type)> = original
        .type_params
        .iter()
        .copied()
        .zip(instance.args.iter().copied())
        .collect();
    let specialized_id = checked.classes.len() as u32 + index as u32;

    let substitute = |ty: Type| -> Type {
        if let Base::Param(id) = ty.base
            && let Some(&(_, replacement)) = subst.iter().find(|(pid, _)| *pid == id)
        {
            return if ty.nullable {
                replacement.as_nullable()
            } else {
                replacement
            };
        }
        ty
    };

    ClassType {
        name: format!("{}${}", original.name, index),
        kind: original.kind,
        base: None,
        fields: original
            .fields
            .iter()
            .map(|f| FieldInfo {
                ty: substitute(f.ty),
                owner: specialized_id,
                ..f.clone()
            })
            .collect(),
        constructors: original
            .constructors
            .iter()
            .map(|params| {
                params
                    .iter()
                    .map(|p| ParamInfo {
                        ty: substitute(p.ty),
                        ..p.clone()
                    })
                    .collect()
            })
            .collect(),
        methods: original
            .methods
            .iter()
            .map(|m| MethodInfo {
                params: m
                    .params
                    .iter()
                    .map(|p| ParamInfo {
                        ty: substitute(p.ty),
                        ..p.clone()
                    })
                    .collect(),
                returns: substitute(m.returns),
                owner: specialized_id,
                from_contract: None,
                overridden: false,
                ..m.clone()
            })
            .collect(),
        contracts: Vec::new(),
        abstract_bases: Vec::new(),
        contract_instances: Vec::new(),
        type_params: Vec::new(),
        shared: original.shared,
        span: original.span,
    }
}

/// One generic enum's own `T` replaced by one instantiation's concrete
/// arguments (roadmap task 13.5) — the enum equivalent of
/// [`specialize_class`], for the same reason: `Iteration<T>`'s payload is
/// inline, so its representation genuinely depends on `T` the way a
/// contract's dispatch table never does.
fn specialize_enum(
    checked: &CheckedProgram,
    instance: zirk_sema::GenericEnumInstance,
    index: usize,
) -> EnumType {
    let original = &checked.enums[instance.enum_id as usize];
    let subst: Vec<(u32, Type)> = original
        .type_params
        .iter()
        .copied()
        .zip(instance.args.iter().copied())
        .collect();

    let substitute = |ty: Type| -> Type {
        if let Base::Param(id) = ty.base
            && let Some(&(_, replacement)) = subst.iter().find(|(pid, _)| *pid == id)
        {
            return if ty.nullable {
                replacement.as_nullable()
            } else {
                replacement
            };
        }
        ty
    };

    EnumType {
        name: format!("{}${}", original.name, index),
        variants: original
            .variants
            .iter()
            .map(|v| EnumVariantInfo {
                associated: v
                    .associated
                    .iter()
                    .map(|f| AssociatedFieldInfo {
                        ty: substitute(f.ty),
                        ..f.clone()
                    })
                    .collect(),
                ..v.clone()
            })
            .collect(),
        type_params: Vec::new(),
        shared: original.shared,
        span: original.span,
    }
}

/// The name a constructor is emitted under.
///
/// The class name plus the index of the `construct` in its declaration order,
/// because a class may have several and they must not collide. The `$` cannot
/// appear in a Zirk identifier, so a user function can never take the name.
pub fn constructor_symbol(class: &str, index: usize) -> String {
    format!("{class}$construct${index}")
}

/// The name a method is emitted under.
///
/// Its own name suffices: a class cannot declare two methods with the same
/// one, since there is no overloading.
pub fn method_symbol(class: &str, method: &str) -> String {
    format!("{class}${method}")
}

/// The name a trait's own default body is emitted under.
///
/// It belongs to the contract, not to any class that adopts it: one body,
/// however many classes reuse it.
pub fn contract_method_symbol(contract: &str, method: &str) -> String {
    format!("{contract}$default${method}")
}

/// The symbol whose body a method entry actually reaches.
///
/// A class that adopted a trait's default has an entry of its own, but the
/// body belongs to the trait: one body, however many classes reuse it.
fn body_symbol(checked: &CheckedProgram, method: &zirk_sema::MethodInfo) -> String {
    match method.from_contract {
        Some(contract) => {
            contract_method_symbol(&checked.contracts[contract as usize].name, &method.name)
        }
        None => method_symbol(&checked.classes[method.owner as usize].name, &method.name),
    }
}

/// Whether any variant of this enum carries associated data — the line
/// between the plain `Int32` representation and `IrType::Enum` (task 11.3).
/// The IR's own `IntWidth` for the checker's — two separate types, on
/// purpose (see `IntWidth`'s own doc comment in `zirk-ir/src/ir.rs`), so
/// this is the one place that translates between them.
const fn ir_int_width(width: SemaIntWidth) -> IntWidth {
    match width {
        SemaIntWidth::I8 => IntWidth::I8,
        SemaIntWidth::I16 => IntWidth::I16,
        SemaIntWidth::I32 => IntWidth::I32,
        SemaIntWidth::I64 => IntWidth::I64,
        SemaIntWidth::I128 => IntWidth::I128,
        SemaIntWidth::U8 => IntWidth::U8,
        SemaIntWidth::U16 => IntWidth::U16,
        SemaIntWidth::U32 => IntWidth::U32,
        SemaIntWidth::U64 => IntWidth::U64,
        SemaIntWidth::U128 => IntWidth::U128,
    }
}

/// A float literal's width, from its optional suffix — mirrors the checker's
/// own `check_float_literal`, since lowering re-derives a literal's type from
/// the tree rather than re-checking it.
fn float_literal_width(lit: &ast::FloatLit) -> FloatWidth {
    match lit.width.as_deref() {
        Some("f16") => FloatWidth::F16,
        Some("f32") => FloatWidth::F32,
        Some("f128") => FloatWidth::F128,
        _ => FloatWidth::F64,
    }
}

/// Same idea as [`ir_int_width`], for the float family.
const fn ir_float_width(width: SemaFloatWidth) -> FloatWidth {
    match width {
        SemaFloatWidth::F16 => FloatWidth::F16,
        SemaFloatWidth::F32 => FloatWidth::F32,
        SemaFloatWidth::F64 => FloatWidth::F64,
        SemaFloatWidth::F128 => FloatWidth::F128,
    }
}

fn enum_has_payload(checked: &CheckedProgram, id: u32) -> bool {
    checked.enums[id as usize]
        .variants
        .iter()
        .any(|v| !v.associated.is_empty())
}

/// Converts a frontend type into an IR type.
///
/// `instance_base` is where the specialized copies of generic classes start
/// in `module.objects`/`checked.classes`: one is appended per entry of
/// `checked.generic_instances`, in the same order, so `Base::Instance(id)`
/// is always at `instance_base + id` (roadmap task 11.1). Every other class
/// keeps its own stable index ahead of that point, untouched.
///
/// `enum_instance_base` is the same idea for `specialize_enum`'s copies —
/// one per `checked.enum_instances` entry, so `Base::EnumInstance(id)` is
/// always at `enum_instance_base + id` (roadmap task 13.5).
fn ir_type(
    ty: Type,
    instance_base: u32,
    enum_instance_base: u32,
    checked: &CheckedProgram,
) -> IrType {
    let base = match ty.base {
        Base::Void => IrType::Void,
        Base::Never => IrType::Never,
        Base::Int(width) => IrType::Int(ir_int_width(width)),
        Base::Float(width) => IrType::Float(ir_float_width(width)),
        Base::Boolean => IrType::Boolean,
        Base::String => IrType::String,
        Base::Char => IrType::Char,
        // A traditional enum — none of its variants carry data — is exactly
        // its discriminant. One with at least one algebraic variant gets a
        // representation of its own (roadmap task 11.3).
        Base::Enum(id) if enum_has_payload(checked, id) => IrType::Enum(id),
        Base::Enum(_) => IrType::Int(IntWidth::I32),
        // A record or value class is a value, not a reference: neither has
        // identity (roadmap task 11.5). An ordinary class is reached through
        // its address, which is its identity.
        Base::Class(id)
            if matches!(
                checked.classes[id as usize].kind,
                ast::ClassKind::Record | ast::ClassKind::ValueClass
            ) =>
        {
            IrType::Value(id)
        }
        Base::Class(id) => IrType::Object(id),
        Base::Contract(id) => IrType::Contract(id),
        // A contract's dispatch table never depended on its own type
        // arguments — only the id and the method index do (task 10.7) — so
        // naming a generic contract instantiation resolves to exactly the
        // same `IrType::Contract` an unparameterized reference would
        // (roadmap task 13.5, `Iterator<T>`).
        Base::ContractInstance(id) => {
            IrType::Contract(checked.contract_instances[id as usize].contract)
        }
        // The specialized copy this instantiation lowered to — see
        // `instance_base` above.
        Base::Instance(id) => IrType::Object(instance_base + id),
        // The specialized copy `specialize_enum` built for this
        // instantiation — see `enum_instance_base` above (roadmap task
        // 13.5, `Iteration<T>`). A payload-free instantiation would stay
        // `Int32`, the same rule an ordinary generic-free enum follows,
        // though nothing yet interns one: every native variant here carries
        // data.
        Base::EnumInstance(id) => {
            let specialized = enum_instance_base + id;
            if enum_has_payload(checked, specialized) {
                IrType::Enum(specialized)
            } else {
                IrType::Int(IntWidth::I32)
            }
        }
        // A verified program contains none of these: the checker reports and
        // the pipeline stops before reaching lowering. A bare generic type
        // parameter is among them: a class's own `T` only reaches lowering
        // once substituted into a concrete argument, by the specialization
        // pass that builds the entries `Base::Instance` above indexes into
        // — and a class not simple enough for that pass to specialize is
        // unconditionally reported with `NOT_LOWERED` at its own declaration
        // (`Self::generic_class_is_directly_specializable` in the checker),
        // so its body never reaches here either.
        // A `Fn(...) => R` position maps to the *canonical*, capture-less
        // `ClosureLayout` `Self::lower` pre-populates for every checker
        // `fn_types` entry, one-to-one by index (roadmap Phase 4d, design
        // D12) — a named function reference or a capture-less lambda always
        // targets it (`Self::lower_expr`'s `ast::Expr::Path` arm,
        // `Self::lower_lambda`'s capture-less branch). A *capturing* lambda
        // literal written directly at a `Fn(...) => R` position (design
        // D14) instead keeps its own, separately-pushed `ClosureLayout`
        // (`Self::lower_lambda`'s capturing branch) — its slot's IR type is
        // read off that value directly, never through this generic
        // conversion (see `Self::lower_lambda`'s own comment on why captures
        // read the *slot's* type, not the checker's).
        Base::Function(id) => IrType::Closure(id),

        // `checked.pointer_types` and `module.pointer_types` are populated in
        // the same order, once, before any function is lowered (`Self::lower`,
        // right after `module` is built) — so a `Base::Pointer` id and the
        // `IrType::Pointer` id it maps to are always the same number, the
        // same "pushed first, in order" trick `checked.fn_types`/
        // `module.closures` already share (roadmap Phase 4e, design D1).
        Base::Pointer(id) => IrType::Pointer(id),

        Base::Unknown | Base::Null | Base::Range | Base::Param(_) | Base::Union(_) => {
            unreachable!("lowering received a construct the checker should have rejected")
        }
    };

    if !ty.nullable {
        return base;
    }

    IrType::Nullable(Nullable::of(base).expect("the checker rejects `Void?`"))
}

/// The static shadow-stack root descriptor for a finished function's own slot
/// table (`fase-4e-colector-mark-sweep`, design D2): every slot — named or
/// D4's synthetic ones, [`FunctionLowering::emit`] already spilled every
/// managed-reference-typed instruction result to one — whose type is
/// [`IrType::is_managed_reference`].
fn gc_roots_of(module: &Module, slots: &[Slot]) -> Vec<SlotId> {
    slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.ty.is_managed_reference(module))
        .map(|(index, _)| SlotId(index as u32))
        .collect()
}

struct FunctionLowering<'a> {
    module: &'a mut Module,
    checked: &'a CheckedProgram,
    declarations: &'a HashMap<&'a str, &'a ast::FnDecl>,
    /// See `ir_type`'s `instance_base` parameter.
    instance_base: u32,
    /// See `ir_type`'s `enum_instance_base` parameter.
    enum_instance_base: u32,

    slots: Vec<Slot>,
    blocks: Vec<Block>,
    /// Block instructions are appended to.
    current: BlockId,
    next_value: u32,

    /// Name to slot, per nesting level, to resolve shadowing exactly as the
    /// checker did.
    scopes: Vec<HashMap<String, SlotId>>,
    return_type: IrType,
    /// Where `break` and `continue` jump, innermost loop last.
    loops: Vec<LoopTargets>,
    /// Lambda bodies lifted out of the function being lowered.
    lifted: Vec<Function>,
    /// The type of each value emitted, so a destination can ask for it instead
    /// of deriving it from the tree a second time.
    value_types: HashMap<ValueId, IrType>,
    /// The binding slot (and its type) of the `catch` directly enclosing the
    /// statement being lowered, if any (roadmap Phase 4b) — what a bare
    /// `throw;` rethrows.
    current_catch: Option<(SlotId, IrType)>,
    /// Active `try` frames, innermost last (roadmap Phase 4b, design D1) —
    /// what a throwing call's post-call check walks to decide where a
    /// pending exception dispatches: the nearest `catch` that covers it, or
    /// (having run every enclosing `finally` it passes on the way) an early
    /// return from the current function.
    try_stack: Vec<TryFrame>,
    /// Set only while lowering a *recursive* lambda's own lifted body
    /// (`ZIRK_LANGUAGE_SPEC.md` section 6, roadmap Phase 4d): the source
    /// name that refers to the lambda itself, this lifted function's own
    /// symbol, the slots (already declared, this function's own leading
    /// parameters) to forward unchanged, and the ordinary parameter types
    /// that follow them. A call to that name inside the body becomes a
    /// direct, ordinary recursive call to this same symbol
    /// (`Self::lower_expr`'s own `ast::Expr::Call` arm) instead of going
    /// through a closure value — there is no such value to call *through*
    /// yet at the point this closure is still being built
    /// (`Self::lower_lambda`'s own doc comment on why the self-reference is
    /// never a real runtime capture).
    recursive_call: Option<(String, String, Vec<SlotId>, Vec<IrType>)>,
}

/// One active `try`'s catches and `finally`, as [`FunctionLowering::try_stack`]
/// tracks it (roadmap Phase 4b). `finally` is a clone of the AST block
/// rather than a borrow: threading its `'a` lifetime through every
/// statement-lowering call in the recursive descent this frame is pushed
/// across would touch far more of this file than the clone costs.
#[derive(Clone)]
struct TryFrame {
    catches: Vec<CatchFrame>,
    finally: Option<ast::Block>,
}

/// One `catch Type(name)` arm, pre-built before its `try`'s body is lowered
/// (roadmap Phase 4b) — its handler block and binding slot must already
/// exist for a throwing call deep inside the body to jump into.
#[derive(Clone)]
struct CatchFrame {
    /// The class `catch` named, by its module object id — [`InstKind::IsInstance`]'s
    /// own target.
    class: u32,
    handler: BlockId,
    slot: SlotId,
    name: String,
}

/// The two blocks a loop exposes to the jumps inside it.
struct LoopTargets {
    break_to: BlockId,
    continue_to: BlockId,
    /// How many `try` frames were active when this loop started (roadmap
    /// Phase 4b) — `break`/`continue` only exits the `try`s *nested inside*
    /// the loop, never ones enclosing it, so only `try_stack[try_depth..]`
    /// runs its `finally` on the way out (`Self::lower_break`/`lower_continue`).
    try_depth: usize,
}

impl<'a> FunctionLowering<'a> {
    fn new(
        module: &'a mut Module,
        checked: &'a CheckedProgram,
        declarations: &'a HashMap<&'a str, &'a ast::FnDecl>,
        instance_base: u32,
        enum_instance_base: u32,
    ) -> Self {
        Self {
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
            slots: Vec::new(),
            blocks: Vec::new(),
            current: BlockId(0),
            next_value: 0,
            scopes: Vec::new(),
            return_type: IrType::Void,
            loops: Vec::new(),
            lifted: Vec::new(),
            value_types: HashMap::new(),
            current_catch: None,
            try_stack: Vec::new(),
            recursive_call: None,
        }
    }

    /// [`ir_type`], with this lowering's own `instance_base`/`enum_instance_base` applied.
    fn ir_type(&self, ty: Type) -> IrType {
        ir_type(
            ty,
            self.instance_base,
            self.enum_instance_base,
            self.checked,
        )
    }

    // --- Construction helpers ---------------------------------------------

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(Block::new(id));
        id
    }

    fn new_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value);
        self.next_value += 1;
        id
    }

    fn block_mut(&mut self, id: BlockId) -> &mut Block {
        self.blocks
            .iter_mut()
            .find(|b| b.id == id)
            .expect("block created by this lowering")
    }

    /// Appends an instruction that produces a value.
    ///
    /// Design D4 of `fase-4e-colector-mark-sweep` (the correctness-critical
    /// finding of that change): whenever `ty` is a collector-managed
    /// reference (directly, or nested inside a `Value`/`Enum`/`Closure`,
    /// [`IrType::is_managed_reference`]), the result is immediately spilled
    /// to its own synthetic slot, unconditionally — the same
    /// `declare_slot` + `Store` move `Self::lower_throws_check` (`fase-4b`)
    /// already uses to keep a value valid across a block boundary its own
    /// lowering introduces, reused here to keep it visible to the shadow
    /// stack across anything downstream that might allocate and trigger a
    /// collection. This runs for *every* such instruction, including a
    /// plain `Load` of an already-named local: the value in hand right now
    /// is what must survive, not merely the slot it happened to come from,
    /// since that slot can be overwritten before the next collection point.
    fn emit(&mut self, kind: InstKind, ty: IrType, span: Span) -> Operand {
        let result = self.new_value();
        self.value_types.insert(result, ty);
        let current = self.current;
        self.block_mut(current).instructions.push(Instruction {
            result: Some(result),
            kind,
            ty,
            span,
        });
        let operand = Operand(result);

        if ty.is_managed_reference(self.module) {
            let slot = self.declare_slot("<gc_root>", ty, span);
            self.emit_effect(InstKind::Store(slot, operand), span);
        }

        operand
    }

    /// Appends an instruction executed for its effect.
    fn emit_effect(&mut self, kind: InstKind, span: Span) {
        let current = self.current;
        self.block_mut(current).instructions.push(Instruction {
            result: None,
            kind,
            ty: IrType::Void,
            span,
        });
    }

    /// Closes the current block, if it is not closed already.
    ///
    /// Code after a `return` is unreachable and produces no terminator: the
    /// first one wins.
    fn terminate(&mut self, terminator: Terminator) {
        let current = self.current;
        let block = self.block_mut(current);
        if block.terminator.is_none() {
            block.terminator = Some(terminator);
        }
    }

    fn is_terminated(&self, id: BlockId) -> bool {
        self.blocks
            .iter()
            .find(|b| b.id == id)
            .is_some_and(|b| b.terminator.is_some())
    }

    fn declare_slot(&mut self, name: &str, ty: IrType, span: Span) -> SlotId {
        let id = SlotId(self.slots.len() as u32);
        self.slots.push(Slot {
            name: name.to_string(),
            ty,
            span,
        });
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), id);
        }
        id
    }

    /// An `Int64` constant. `ConstInt` always declares `Int32` (a literal
    /// always types `Int32` — `ast::Expr::Int`'s own lowering does the
    /// same), so a wider constant goes through `IntCast` from one, exactly
    /// the same as any other `Int32` value reaching a wider destination.
    fn const_i64(&mut self, value: i32, span: Span) -> Operand {
        let narrow = self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I32), span);
        self.emit(InstKind::IntCast(narrow), IrType::Int(IntWidth::I64), span)
    }

    fn lookup_slot(&self, name: &str) -> SlotId {
        self.try_lookup_slot(name)
            .expect("a verified program only names declared variables")
    }

    fn try_lookup_slot(&self, name: &str) -> Option<SlotId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
    }

    /// The callable id and target symbol of a bare name that is a *named
    /// function* used as a value, not called (`Checker::check_path`'s own
    /// "a bare function name is a value" rule, roadmap Phase 4d) — `None`
    /// when `name` is not one (a local shadows it, or it names nothing).
    ///
    /// The id is found the same way `Self::resolve_written_type` finds a
    /// written `Fn(...) => R` annotation's id: by structural search over
    /// `checked.fn_types`, since a named function's own callable type is
    /// interned there exactly like a written annotation of the same shape
    /// (`Checker::intern_fn_type`, design D12) — not pushed fresh the way a
    /// capturing lambda's is.
    fn named_function_value(&self, name: &str, span: Span) -> Option<(u32, String)> {
        let declared = self.declaration_of(name, span);
        let signature = self.checked.functions.get(&declared)?;
        let shape = FnType {
            params: signature.param_types(),
            returns: signature.returns,
        };
        let id = self
            .checked
            .fn_types
            .iter()
            .position(|f| *f == shape)
            .expect("the checker interned this named function's own callable type")
            as u32;
        Some((id, declared))
    }

    fn slot_type(&self, id: SlotId) -> IrType {
        self.slots[id.0 as usize].ty
    }

    /// The IR type a written annotation names.
    ///
    /// Enums are not built in, so the name is resolved against the table the
    /// checker produced rather than against a fixed list.
    /// The declaration a call names, with any import alias already applied.
    ///
    /// The checker recorded the resolution, so lowering does not need to know
    /// that aliases exist.
    fn callee_name(&self, call: &ast::CallExpr) -> String {
        let ident = callee_ident(call);
        self.declaration_of(&ident.name, ident.span)
    }

    /// The declaration a written name refers to, following import aliases.
    fn declaration_of(&self, written: &str, at: Span) -> String {
        self.checked
            .aliases
            .get(&at)
            .cloned()
            .unwrap_or_else(|| written.to_string())
    }

    fn ir_type_from_ref(&self, reference: &ast::TypeRef) -> IrType {
        let mut ty = self.resolve_written_type(reference);
        ty.nullable = reference.nullable;
        self.ir_type(ty)
    }

    /// Resolves a written type reference the same way the checker's own
    /// `resolve_type` did, but against lowering's tables — needed because a
    /// generic class's annotation (`Box<Int32>`) names its arguments in the
    /// source, and only the checker's `generic_instances` table (already
    /// deduplicated by content) says which specialized copy that is
    /// (roadmap task 11.1). A bare `Box` without arguments is the ordinary,
    /// unspecialized class it always was.
    fn resolve_written_type(&self, reference: &ast::TypeRef) -> Type {
        // `Fn(P...) => R` / `Function(P...) => R` (roadmap Phase 4d): the
        // checker already interned every shape it type-checked
        // (`Checker::intern_fn_type`, or a capturing lambda's own reserved
        // id, design D14) — this looks the same shape up in
        // `checked.fn_types` by structural equality rather than interning a
        // second time, so it always lands on the exact id the checker used.
        if let Some(function) = &reference.function {
            let params: Vec<Type> = function
                .params
                .iter()
                .map(|p| {
                    let mut ty = self.resolve_written_type(&p.ty);
                    ty.nullable = p.ty.nullable;
                    ty
                })
                .collect();
            let mut returns = self.resolve_written_type(&function.returns);
            returns.nullable = function.returns.nullable;
            let shape = FnType { params, returns };
            let id = self
                .checked
                .fn_types
                .iter()
                .position(|f| *f == shape)
                .expect("the checker interned every callable type it type-checked")
                as u32;
            return Type::of(Base::Function(id));
        }

        // `Pointer<T>` (roadmap Phase 4e, design D1): interned the same way
        // `Fn(...) => R` above is — looked up in `checked.pointer_types` by
        // structural equality, matching the exact id `Checker::resolve_pointer_type_ref`
        // used, rather than interning a second, unrelated table here.
        if reference.name == "Pointer" {
            let pointee = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .pointer_types
                .iter()
                .position(|&t| t == pointee)
                .expect("the checker interned every Pointer<T> it type-checked")
                as u32;
            return Type::of(Base::Pointer(id));
        }

        let declared = self.declaration_of(&reference.name, reference.span);
        if let Some(ty) = Type::from_name(&reference.name) {
            return ty;
        }
        if let Some(id) = self.checked.enums.iter().position(|e| e.name == declared) {
            if reference.arguments.is_empty() {
                return Type::of(Base::Enum(id as u32));
            }
            let args: Vec<Type> = reference
                .arguments
                .iter()
                .map(|a| self.resolve_written_type(a))
                .collect();
            let instance = self
                .checked
                .enum_instances
                .iter()
                .position(|gi| gi.enum_id == id as u32 && gi.args == args)
                .expect("the checker interned every instantiation it type-checked");
            return Type::of(Base::EnumInstance(instance as u32));
        }
        if let Some(id) = self.checked.classes.iter().position(|c| c.name == declared) {
            if reference.arguments.is_empty() {
                return Type::of(Base::Class(id as u32));
            }
            let args: Vec<Type> = reference
                .arguments
                .iter()
                .map(|a| self.resolve_written_type(a))
                .collect();
            let instance = self
                .checked
                .generic_instances
                .iter()
                .position(|gi| gi.class == id as u32 && gi.args == args)
                .expect("the checker interned every instantiation it type-checked");
            return Type::of(Base::Instance(instance as u32));
        }
        if let Some(id) = self
            .checked
            .contracts
            .iter()
            .position(|c| c.name == declared)
        {
            return Type::of(Base::Contract(id as u32));
        }
        unreachable!("a verified program only names known types")
    }

    /// Lowers an expression whose value must survive lowering a later one.
    ///
    /// Values do not cross blocks in this IR (ADR-007), and `??`, `if`, `match`
    /// and the logical operators all open blocks of their own. When the later
    /// expression is one of those, the earlier value goes through a slot and is
    /// reloaded where it is used; when it is not, nothing is spilled and the
    /// IR stays as it was.
    fn lower_and_hold(&mut self, expr: &ast::Expr, will_branch: bool) -> Held {
        let value = self.lower_expr(expr);
        if !will_branch {
            return Held::Value(value);
        }

        let ty = self.type_of_operand(value);
        let slot = self.declare_slot("<held>", ty, expr.span());
        self.emit_effect(InstKind::Store(slot, value), expr.span());
        Held::Spilled(slot, ty)
    }

    /// Lowers a list of arguments, holding each across the ones that follow.
    ///
    /// An argument that opens blocks would strand every value already computed,
    /// so the earlier ones go through slots and are reloaded once the last has
    /// been lowered.
    fn lower_held_args(&mut self, args: &[ast::Arg], expected: &[IrType]) -> Vec<Operand> {
        let mut held = Vec::with_capacity(args.len());

        for (position, arg) in args.iter().enumerate() {
            let ty = expected
                .get(position)
                .copied()
                .unwrap_or(IrType::Int(IntWidth::I32));
            let branches_later = args[position + 1..]
                .iter()
                .any(|later| self.opens_blocks(&later.value));
            held.push(self.lower_and_hold_as(&arg.value, ty, branches_later));
        }

        held.into_iter()
            .zip(args)
            .map(|(value, arg)| self.reload(value, arg.span))
            .collect()
    }

    /// [`Self::lower_and_hold`], lowering into an expected type.
    fn lower_and_hold_as(&mut self, expr: &ast::Expr, ty: IrType, will_branch: bool) -> Held {
        let value = self.lower_expr_as(expr, ty);
        if !will_branch {
            return Held::Value(value);
        }

        let slot = self.declare_slot("<arg>", ty, expr.span());
        self.emit_effect(InstKind::Store(slot, value), expr.span());
        Held::Spilled(slot, ty)
    }

    /// Recovers a held value in whatever block is current now.
    fn reload(&mut self, held: Held, span: Span) -> Operand {
        match held {
            Held::Value(operand) => operand,
            Held::Spilled(slot, ty) => self.emit(InstKind::Load(slot), ty, span),
        }
    }

    /// Lowers an expression into the type its destination expects.
    ///
    /// `T` is accepted where `T?` is expected, `null` fits any nullable type,
    /// and a subclass or a contract implementation is accepted where a base
    /// class or that contract is expected (D5's ordinary subtyping — an
    /// object's own address does not change shape between the two, only
    /// which methods a static type promises). None of those is a conversion
    /// the IR performs implicitly: this is where the widening becomes an
    /// instruction.
    fn lower_expr_as(&mut self, expr: &ast::Expr, expected: IrType) -> Operand {
        // A diverging source (`fatalError(...)`, roadmap Phase 4a) never
        // actually produces a value of `expected` — the checker's
        // `Type::accepts`/`Type::unify` already let `Never` fit anywhere, so
        // lowering has to make good on that. The expression is still
        // lowered for its own diverging effect (ending in
        // `Terminator::Unreachable`); whatever the caller does with the
        // returned operand afterward runs in code that can never execute,
        // so a type-correct placeholder is enough there — reusing
        // `default_value` is what keeps this from needing its own
        // special-case machinery for every scalar. `default_value` has
        // nothing for `Object`/`Contract`/`Value`/`Enum`/`Char`/`Closure`
        // (no zero-arg constructor to call safely), so a diverging
        // initializer of one of those types is a narrower, documented gap
        // rather than something this silently gets wrong: the `Never`-typed
        // operand itself is returned, which the verifier then reports as a
        // type mismatch instead of miscompiling.
        // `Never` can only originate from a `fatalError(...)` call and
        // propagate through an `if`/ternary join (roadmap Phase 4a covers
        // those two; `match` is left out on purpose — asking `type_of` a
        // `Match` here runs into assumptions about its own arms' scope that
        // do not hold this early, unrelated to `Never` itself, so a
        // diverging `match` arm is a known, narrower gap than the other
        // two). Restricted to these expression kinds so this also never
        // calls `type_of` on a `Lambda`/`Null`/`Range`, which it cannot
        // answer for directly (its own doc comment: "the type of this
        // expression comes from the value it produced").
        if matches!(
            expr,
            ast::Expr::Call(_) | ast::Expr::If(_) | ast::Expr::Ternary(_)
        ) && self.type_of(expr, expr.span()) == IrType::Never
        {
            let diverged = self.lower_expr(expr);
            return self
                .default_value(expected, expr.span())
                .unwrap_or(diverged);
        }

        // A bare variant access with no associated data (`Iteration.Done`)
        // resolves here against the destination's own enum id, rather than
        // through `lower_expr`'s ordinary path (below): the checker types
        // it against the enum's own bare declaration (`Self::check_variant`
        // never learns which instantiation a caller has in mind — that is
        // exactly what `bare_enum_matches_instance` exists to sidestep on
        // the checking side), so building it against that bare id would use
        // a still-generic enum's empty, unspecialized layout (roadmap task
        // 13.5, `Iteration<T>`'s `Done`). The destination already knows the
        // concrete one.
        if let ast::Expr::Field(field) = expr
            && self.checked.variant_accesses.contains(&field.span)
            && let IrType::Enum(enum_id) = expected.unwrapped()
        {
            let variant = self.checked.enums[enum_id as usize]
                .discriminant(&field.name.name)
                .expect("the checker resolved this variant");
            let value = self.emit(
                InstKind::BuildEnum {
                    enum_id,
                    variant,
                    fields: Vec::new(),
                },
                IrType::Enum(enum_id),
                field.span,
            );
            return match expected {
                IrType::Nullable(base) => {
                    self.emit(InstKind::Wrap { base, value }, expected, field.span)
                }
                _ => value,
            };
        }

        if let IrType::Nullable(base) = expected {
            // `null` has no type of its own: the destination supplies it.
            if matches!(expr, ast::Expr::Null(_)) {
                return self.emit(InstKind::NullValue(base), expected, expr.span());
            }

            let value = self.lower_expr(expr);
            let actual = self.type_of(expr, expr.span());
            if actual == expected {
                return value;
            }

            // A bare `T` widens to `T?` by wrapping; a subclass or contract
            // implementation additionally needs its declared type retagged
            // first, and a narrower integer needs its bits actually
            // extended first (both mirror the non-nullable branch below),
            // since `Wrap` itself only adds the present flag, not the
            // widening.
            let value = if actual == base.inner() {
                value
            } else if matches!(actual, IrType::Object(_) | IrType::Contract(_))
                && matches!(base, Nullable::Object(_) | Nullable::Contract(_))
            {
                self.emit(InstKind::Retype(value), base.inner(), expr.span())
            } else if matches!(actual, IrType::Int(_)) && matches!(base, Nullable::Int(_)) {
                self.emit(InstKind::IntCast(value), base.inner(), expr.span())
            } else if matches!(actual, IrType::Float(_)) && matches!(base, Nullable::Float(_)) {
                self.emit(InstKind::FloatCast(value), base.inner(), expr.span())
            } else {
                value
            };

            return self.emit(InstKind::Wrap { base, value }, expected, expr.span());
        }

        let value = self.lower_expr(expr);
        if self.type_of_operand(value) == expected {
            return value;
        }

        // The checker already proved this is a subclass, or a contract
        // implementation, of what is expected — that is the only way the
        // types could differ here without `expect_assignable` having
        // rejected the program. Both share the same representation (an
        // object's own address), so there is nothing to check at runtime,
        // only the declared type changes from here on — unlike `as`, whose
        // relation is not proven ahead of time (roadmap task 11.6).
        if matches!(expected, IrType::Object(_) | IrType::Contract(_)) {
            return self.emit(InstKind::Retype(value), expected, expr.span());
        }

        // The checker's own safe-widening rule (`Type::accepts`, roadmap
        // Phase 3b task 4.3): a narrower integer of the same signedness
        // fits a wider one implicitly, but the bits still need to be
        // sign/zero-extended at the IR level — unlike an object reference,
        // an integer's representation does change size.
        if matches!(expected, IrType::Int(_)) {
            return self.emit(InstKind::IntCast(value), expected, expr.span());
        }
        // Same idea, one family over: `Float` widening is always exact
        // (`Type::accepts`'s float arm), but the bits still need extending.
        if matches!(expected, IrType::Float(_)) {
            return self.emit(InstKind::FloatCast(value), expected, expr.span());
        }

        value
    }

    // --- Function ---------------------------------------------------------

    fn run(mut self, function: &ast::FnDecl) -> (Function, Vec<Function>) {
        // Read off `checked.functions`, not re-derived from the written
        // annotation (`Self::ir_type_from_ref`): when the declared return
        // type is a capturing `Fn(...) => R`, the checker narrows the
        // signature's own `returns` to the one literal's exact id once its
        // body accepts it (`Checker::check_function`'s own post-body
        // fix-up, design D14) — re-deriving from the written text here
        // would instead find *a* same-shaped id (very possibly the
        // canonical, capture-less one another position uses), not
        // necessarily the specific one this function's body actually
        // builds.
        self.return_type = self
            .checked
            .functions
            .get(&function.name.name)
            .map(|s| self.ir_type(s.returns))
            .expect("the checker records every declared function");

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        // The parameter types come from the checker, not from what was
        // written: `name?: T` is declared as `T` and resolved as `T?`, and the
        // slot has to hold the resolved one.
        let resolved: Vec<IrType> = self
            .checked
            .functions
            .get(&function.name.name)
            .map(|s| s.params.iter().map(|p| self.ir_type(p.ty)).collect())
            .expect("the checker records every declared function");

        let params: Vec<SlotId> = function
            .params
            .iter()
            .zip(resolved)
            .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span))
            .collect();

        self.lower_block(&function.body);

        // A `Void` function may end without an explicit return. In a non-`Void`
        // one the checker already proved every path returns, so an open block
        // here is one nothing reaches — the block after `loop { return x; }`
        // being the ordinary case.
        if self.return_type == IrType::Void {
            self.terminate(Terminator::Return(None));
        } else {
            self.terminate(Terminator::Unreachable);
        }

        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let gc_roots = gc_roots_of(self.module, &self.slots);
        let lowered = Function {
            name: function.name.name.clone(),
            params,
            return_type: self.return_type,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: function.span,
            gc_roots,
        };

        (lowered, lifted)
    }

    /// Lowers one `construct` into a function over the object being built.
    ///
    /// `this` is an ordinary slot holding the object's address, which is what
    /// makes `this.name = value` a field write like any other rather than a
    /// form of its own.
    fn run_constructor(
        mut self,
        class: &ast::ClassDecl,
        constructor: &ast::ConstructDecl,
        id: u32,
        index: usize,
    ) -> (Function, Vec<Function>) {
        self.return_type = IrType::Void;

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let this = self.declare_slot("this", IrType::Object(id), class.name.span);
        self.initialize_defaults(this, id, class.name.span);

        let resolved: Vec<IrType> = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.constructors.get(index))
            .map(|params| params.iter().map(|p| self.ir_type(p.ty)).collect())
            .expect("the checker records every constructor");

        let mut params = vec![this];
        params.extend(
            constructor
                .params
                .iter()
                .zip(resolved)
                .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span)),
        );

        self.lower_block(&constructor.body);
        self.terminate(Terminator::Return(None));
        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            // The checked class's own name, not the written declaration's:
            // a specialized copy's is `Box$0`, not `Box` (roadmap task
            // 11.1) — using the AST's bare name here would collide every
            // instantiation's constructor onto the same symbol.
            name: constructor_symbol(&self.checked.classes[id as usize].name, index),
            params,
            return_type: IrType::Void,
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: constructor.span,
        };

        (lowered, lifted)
    }

    /// Lowers a trait's default body into a function over the receiver.
    ///
    /// Its `this` is typed by the contract rather than by any class: the body
    /// was written knowing only what the contract declares, which is exactly
    /// what it may reach.
    fn run_contract_default(
        mut self,
        contract: &ast::ContractDecl,
        method: &ast::MethodDecl,
        body: &ast::Block,
        id: u32,
    ) -> (Function, Vec<Function>) {
        self.return_type = self.ir_type_from_ref(&method.return_type);

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let this = self.declare_slot("this", IrType::Contract(id), contract.name.span);

        let resolved: Vec<IrType> = self
            .checked
            .contracts
            .get(id as usize)
            .and_then(|c| c.method(&method.name.name))
            .map(|m| m.params.iter().map(|p| self.ir_type(p.ty)).collect())
            .expect("the checker records every contract method");

        let mut params = vec![this];
        params.extend(
            method
                .params
                .iter()
                .zip(resolved)
                .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span)),
        );

        self.lower_block(body);
        if self.return_type == IrType::Void {
            self.terminate(Terminator::Return(None));
        } else {
            self.terminate(Terminator::Unreachable);
        }
        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            name: contract_method_symbol(&contract.name.name, &method.name.name),
            params,
            return_type: self.return_type,
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: method.span,
        };

        (lowered, lifted)
    }

    /// Writes every attribute's type default into a fresh object.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 7: an omitted attribute receives its
    /// default *before* any explicit initializer or the constructor runs.
    ///
    /// Zeroed memory is not a substitute. It happens to spell `0` and `false`,
    /// but a zeroed `String` is a null handle, not the empty string — and the
    /// difference shows the moment anyone compares it to `""`.
    fn initialize_defaults(&mut self, this: SlotId, id: u32, span: Span) {
        let fields: Vec<(u32, IrType)> = self.module.objects[id as usize]
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| (index as u32, field.ty))
            .collect();

        for (index, ty) in fields {
            let Some(value) = self.default_value(ty, span) else {
                // No default: the checker already required the constructor to
                // write it.
                continue;
            };
            let object = self.emit(InstKind::Load(this), IrType::Object(id), span);
            self.emit_effect(
                InstKind::StoreField {
                    object,
                    index,
                    value,
                },
                span,
            );
        }
    }

    /// The default value of a type, if it has one.
    fn default_value(&mut self, ty: IrType, span: Span) -> Option<Operand> {
        Some(match ty {
            // `0` needs no width-specific bit pattern, so one `ConstInt`
            // serves every width — only `ty` (already the right one) says
            // which.
            IrType::Int(_) => self.emit(InstKind::ConstInt(0), ty, span),
            IrType::Float(width) => {
                self.emit(InstKind::ConstFloat(width, "0".to_string()), ty, span)
            }
            IrType::Boolean => self.emit(InstKind::ConstBool(false), ty, span),
            IrType::String => {
                let id = self.module.intern_string("");
                self.emit(InstKind::ConstString(id), ty, span)
            }
            // Absence is exactly what a nullable type defaults to.
            IrType::Nullable(base) => self.emit(InstKind::NullValue(base), ty, span),
            // `Char` has no default (`docs/handbook/11-reference/03-built-in-types.md`:
            // "invalid without an explicit value") — unlike `String`'s `""`,
            // there is no empty grapheme to fall back to.
            IrType::Void
            | IrType::Never
            | IrType::Char
            | IrType::Closure(_)
            | IrType::Object(_)
            | IrType::Contract(_)
            | IrType::Value(_)
            | IrType::Enum(_)
            // A `Pointer<T>` field is unreachable in a verified program: the
            // checker's escape rule (design D4) rejects every assignment of
            // a `Pointer<T>` value into a field, so nothing here ever needs
            // a default for one.
            | IrType::Pointer(_) => {
                return None;
            }
        })
    }

    /// Lowers one method into a function whose first parameter is the receiver.
    fn run_method(
        mut self,
        class: &ast::ClassDecl,
        method: &ast::MethodDecl,
        body: &ast::Block,
        id: u32,
        index: usize,
    ) -> (Function, Vec<Function>) {
        // Read from the checked class rather than the written annotation: a
        // specialized copy's method returns `Int32` where the AST still says
        // `T` (roadmap task 11.1) — `checked.classes[id]` already carries
        // the substituted type, the same table `resolved` below reads for
        // the parameters.
        let found = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.methods.iter().find(|m| m.index == index))
            .expect("the checker records every method");
        self.return_type = self.ir_type(found.returns);
        let resolved: Vec<IrType> = found.params.iter().map(|p| self.ir_type(p.ty)).collect();

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        // A record or value class's method receives `this` by value too
        // (roadmap task 11.5): there is nothing to point at.
        let this_ty = self.ir_type(Type::of(Base::Class(id)));
        let this = self.declare_slot("this", this_ty, class.name.span);

        let mut params = vec![this];
        params.extend(
            method
                .params
                .iter()
                .zip(resolved)
                .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span)),
        );

        self.lower_block(body);

        // Same rule as an ordinary function: a `Void` one may end without an
        // explicit return, and in any other the checker already proved every
        // path returns.
        if self.return_type == IrType::Void {
            self.terminate(Terminator::Return(None));
        } else {
            self.terminate(Terminator::Unreachable);
        }

        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            // See the identical note in `run_constructor`.
            name: method_symbol(&self.checked.classes[id as usize].name, &method.name.name),
            params,
            return_type: self.return_type,
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: method.span,
        };

        (lowered, lifted)
    }

    /// `message(): String { return this.reason; }` (D8) — `reason` is
    /// always field `0`, the class's only one, written by whatever built the
    /// object (`Self::lower_construction`'s special case for a user's own
    /// `Type("...")`, or `Self::build_native_failure` for a compiler-thrown
    /// one).
    fn build_native_message(mut self, class_id: u32) -> Function {
        let span = Span::empty(0);
        let entry = self.new_block();
        self.current = entry;
        let this_ty = IrType::Object(class_id);
        let this = self.declare_slot("this", this_ty, span);
        let object = self.emit(InstKind::Load(this), this_ty, span);
        let value = self.emit(
            InstKind::LoadField { object, index: 0 },
            IrType::String,
            span,
        );
        self.terminate(Terminator::Return(Some(value)));

        Function {
            name: method_symbol(&self.checked.classes[class_id as usize].name, "message"),
            params: vec![this],
            return_type: IrType::String,
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span,
        }
    }

    /// `code(): String { return "<fixed code>"; }` (D8) — one constant per
    /// class (`E_DIVISION_BY_ZERO`, …), unrelated to the instance's own
    /// `reason`.
    fn build_native_code(mut self, class_id: u32, code: &str) -> Function {
        let span = Span::empty(0);
        let entry = self.new_block();
        self.current = entry;
        let this_ty = IrType::Object(class_id);
        let this = self.declare_slot("this", this_ty, span);
        let text = self.module.intern_string(code);
        let value = self.emit(InstKind::ConstString(text), IrType::String, span);
        self.terminate(Terminator::Return(Some(value)));

        Function {
            name: method_symbol(&self.checked.classes[class_id as usize].name, "code"),
            params: vec![this],
            return_type: IrType::String,
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span,
        }
    }

    /// `cause(): Error? { return null; }` (D8) — none of the four native
    /// failures ever wraps an earlier one.
    fn build_native_cause(mut self, class_id: u32, error_id: u32) -> Function {
        let span = Span::empty(0);
        let entry = self.new_block();
        self.current = entry;
        let this_ty = IrType::Object(class_id);
        let this = self.declare_slot("this", this_ty, span);
        let nullable = Nullable::Object(error_id);
        let value = self.emit(
            InstKind::NullValue(nullable),
            IrType::Nullable(nullable),
            span,
        );
        self.terminate(Terminator::Return(Some(value)));

        Function {
            name: method_symbol(&self.checked.classes[class_id as usize].name, "cause"),
            params: vec![this],
            return_type: IrType::Nullable(nullable),
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span,
        }
    }

    /// `stack_trace(): StackTrace { return StackTrace(); }` (D8) — the same
    /// `Alloc`-with-no-constructor `Self::lower_construction` already builds
    /// for a user-written `StackTrace()`, reused here since this body never
    /// goes through an `ast::CallExpr` to reach that path itself.
    fn build_native_stack_trace(mut self, class_id: u32, stack_trace_id: u32) -> Function {
        let span = Span::empty(0);
        let entry = self.new_block();
        self.current = entry;
        let this_ty = IrType::Object(class_id);
        let this = self.declare_slot("this", this_ty, span);
        let value = self.emit(
            InstKind::Alloc(stack_trace_id),
            IrType::Object(stack_trace_id),
            span,
        );
        self.terminate(Terminator::Return(Some(value)));

        Function {
            name: method_symbol(&self.checked.classes[class_id as usize].name, "stack_trace"),
            params: vec![this],
            return_type: IrType::Object(stack_trace_id),
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span,
        }
    }

    fn lower_block(&mut self, block: &ast::Block) {
        self.scopes.push(HashMap::new());
        for stmt in &block.statements {
            self.lower_stmt(stmt);
        }
        self.scopes.pop();
    }

    // --- Statements -------------------------------------------------------

    fn lower_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Let(s) => self.lower_let(s),
            ast::Stmt::MultiLet(s) => self.lower_multi_let(s),
            ast::Stmt::Assign(s) => self.lower_assign(s),
            ast::Stmt::MultiAssign(s) => self.lower_multi_assign(s),
            ast::Stmt::If(s) => self.lower_if(s),
            ast::Stmt::Loop(s) => self.lower_loop(s),
            ast::Stmt::ForIn(s) => self.lower_for_in(s),
            ast::Stmt::Break(s) => self.lower_break(s),
            ast::Stmt::Continue(s) => self.lower_continue(s),
            ast::Stmt::Return(s) => self.lower_return(s),
            ast::Stmt::Expr(s) => {
                self.lower_expr_for_effect(&s.expr);
            }
            ast::Stmt::Block(b) => self.lower_block(b),
            ast::Stmt::Throw(s) => self.lower_throw(s),
            ast::Stmt::Try(s) => self.lower_try(s),
            // `unsafe { }`/`commit { }` lower as an ordinary block (roadmap
            // Phase 4e). The transactional journal (design D5/D6) is
            // explicitly deferred — see `openspec/changes/fase-4e-unsafe-pointer-extern/tasks.md`
            // section 8: this slice ships the pointer core and `extern`
            // (D1-D4, D7-D8) without rollback-on-failure, which the checker
            // still enforces at the *context* level (unsafe/commit depth)
            // regardless of whether a write inside is journaled.
            ast::Stmt::Unsafe(s) => self.lower_block(&s.body),
            ast::Stmt::Commit(s) => self.lower_block(&s.body),
        }
    }

    fn lower_let(&mut self, stmt: &ast::LetStmt) {
        // The value is computed before declaring the slot so that
        // `mut x = x` — with an outer `x` — reads the outer one, as the checker
        // resolved it.
        let annotated = stmt.ty.as_ref().map(|a| self.ir_type_from_ref(a));

        // D14: a capturing closure literal written directly as this
        // initializer adopts its own id as the local's type
        // (`Checker::check_let`'s own narrowing) — re-deriving the
        // annotation's id from its written text (`annotated`, above) only
        // finds *a* same-shaped `Fn(...) => R` id, which is ambiguous
        // whenever this exact shape is also the canonical, capture-less one
        // another position shares (`Self::resolve_written_type`'s own doc
        // comment) or another capturing literal entirely. The initializer's
        // own lowered value already carries the right id
        // (`Self::lower_lambda` derives it straight from the checker's
        // `LambdaInfo`, not from this annotation), so it is asked instead.
        let literal_captures = matches!(
            stmt.init.as_ref(),
            Some(ast::Expr::Lambda(l))
                if self
                    .checked
                    .lambdas
                    .get(&l.span)
                    .is_some_and(|i| !i.captures.is_empty())
        );

        let value = stmt.init.as_ref().map(|e| {
            let operand = match annotated {
                Some(expected) if !literal_captures => self.lower_expr_as(e, expected),
                _ => self.lower_expr(e),
            };
            (operand, e.span())
        });

        let ty = match (annotated, value) {
            (Some(ty), _) if !literal_captures => ty,
            // Without an annotation — or with one a capturing literal just
            // overrides — the type is the one the initializer produced.
            // Asking the value rather than the tree is what lets a lambda be
            // inferred: its closure type only exists once lowered.
            (_, Some((operand, _))) => self.type_of_operand(operand),
            (None, None) => unreachable!("without a type there is always an initializer"),
            (Some(ty), None) => ty,
        };

        let slot = self.declare_slot(&stmt.name.name, ty, stmt.name.span);

        if let Some((operand, _)) = value {
            self.emit_effect(InstKind::Store(slot, operand), stmt.span);
        }
    }

    /// `mut first, second: String = a, b;` (roadmap Phase 4d) — the
    /// comma-grouped form of `Self::lower_let`.
    ///
    /// The checker only pairs initializers to names when the arity matches
    /// (`Checker::check_multi_let`); a mismatch is already a reported error,
    /// so this still lowers every initializer for its side effects but pairs
    /// none of them, and gives every binding its type's default value
    /// (`Self::default_value`) — the same fallback the checker's own
    /// `initialized` flag already assumed.
    fn lower_multi_let(&mut self, stmt: &ast::MultiLetStmt) {
        let mut shared_ty = stmt.ty.as_ref().map(|a| self.ir_type_from_ref(a));
        let arity_matches = !stmt.inits.is_empty() && stmt.inits.len() == stmt.names.len();

        let mut inits: Vec<Operand> = Vec::with_capacity(stmt.inits.len());
        if arity_matches {
            for init in &stmt.inits {
                let operand = match shared_ty {
                    Some(expected) => self.lower_expr_as(init, expected),
                    None => self.lower_expr(init),
                };
                if shared_ty.is_none() {
                    shared_ty = Some(self.type_of_operand(operand));
                }
                inits.push(operand);
            }
        } else {
            for init in &stmt.inits {
                self.lower_expr(init);
            }
        }

        let ty = shared_ty.expect("the checker required a determinable shared type");

        for (i, name) in stmt.names.iter().enumerate() {
            let slot = self.declare_slot(&name.name, ty, name.span);
            let value = if arity_matches {
                Some(inits[i])
            } else {
                self.default_value(ty, name.span)
            };
            if let Some(value) = value {
                self.emit_effect(InstKind::Store(slot, value), stmt.span);
            }
        }
    }

    /// The type an assignment target's storage holds, without lowering
    /// anything — shared by `Self::lower_multi_assign`'s two phases so a
    /// destination's expected type is known before any source is lowered.
    fn assign_target_type(&self, target: &ast::AssignTarget) -> IrType {
        match target {
            ast::AssignTarget::Name(name) => self.slot_type(self.lookup_slot(&name.name)),
            ast::AssignTarget::Field(field) => {
                self.field_position(&field.object, &field.name.name).1
            }
        }
    }

    /// Commits an already-computed `value` to one assignment target — the
    /// store-emission path `Self::lower_multi_assign`'s second phase reuses
    /// per destination (design D3). `value` is always a plain operand by the
    /// time this runs (loaded back from a temporary slot), so — unlike
    /// `Self::lower_assign`'s own `Field` case — lowering it can never open a
    /// block partway through the object/value pair, and no spill is needed.
    fn store_assign_target(&mut self, target: &ast::AssignTarget, value: Operand, span: Span) {
        match target {
            ast::AssignTarget::Name(name) => {
                let slot = self.lookup_slot(&name.name);
                self.emit_effect(InstKind::Store(slot, value), span);
            }
            ast::AssignTarget::Field(field) => {
                let object = self.lower_expr(&field.object);
                let (index, _ty) = self.field_position(&field.object, &field.name.name);
                self.emit_effect(
                    InstKind::StoreField {
                        object,
                        index,
                        value,
                    },
                    span,
                );
            }
        }
    }

    /// `left, right = right, left;` (roadmap Phase 4d) — the comma-grouped
    /// form of `Self::lower_assign`.
    ///
    /// Design D3: every source is evaluated into a temporary, left to right,
    /// *before* any destination is written; writes then commit left to
    /// right. This ordering is what makes `left, right = right, left;` swap
    /// correctly instead of the second read observing the first write.
    fn lower_multi_assign(&mut self, stmt: &ast::MultiAssignStmt) {
        let n = stmt.targets.len().min(stmt.values.len());

        let mut temps: Vec<Option<(SlotId, IrType)>> = Vec::with_capacity(n);
        for i in 0..n {
            if let ast::AssignTarget::Name(name) = &stmt.targets[i]
                && name.name == "_"
            {
                self.lower_expr(&stmt.values[i]);
                temps.push(None);
                continue;
            }

            let ty = self.assign_target_type(&stmt.targets[i]);
            let value = self.lower_expr_as(&stmt.values[i], ty);
            let slot = self.declare_slot("<multi_assign_tmp>", ty, stmt.values[i].span());
            self.emit_effect(InstKind::Store(slot, value), stmt.values[i].span());
            temps.push(Some((slot, ty)));
        }

        // Beyond the shorter list's length (an arity mismatch, already
        // reported by the checker), the extra sources are still lowered for
        // any side effect, same as `Checker::check_multi_assign`'s own
        // leftover handling.
        for value in &stmt.values[n..] {
            self.lower_expr(value);
        }

        for (i, temp) in temps.iter().enumerate().take(n) {
            let Some((slot, ty)) = *temp else { continue };
            let value = self.emit(InstKind::Load(slot), ty, stmt.values[i].span());
            self.store_assign_target(&stmt.targets[i], value, stmt.span);
        }
    }

    fn lower_assign(&mut self, stmt: &ast::AssignStmt) {
        match &stmt.target {
            // `_ = expr;`: the value is lowered for its side effects and
            // then dropped — there is no slot named `_` to store it in
            // (`Checker::check_assign`'s own `_` handling never declared
            // one), which is exactly the point (roadmap Phase 4a).
            ast::AssignTarget::Name(name) if name.name == "_" => {
                self.lower_expr(&stmt.value);
            }
            ast::AssignTarget::Name(name) => {
                let slot = self.lookup_slot(&name.name);
                let value = self.lower_expr_as(&stmt.value, self.slot_type(slot));
                self.emit_effect(InstKind::Store(slot, value), stmt.span);
            }
            ast::AssignTarget::Field(field) => {
                // The object is evaluated before the value, which is the order
                // it is written in. When the value contains its own `?.` (or
                // any other block-opening form), lowering it moves `self.current`
                // to a fresh block — so the object computed here does not
                // survive to the `StoreField` below unless it goes through a
                // slot first, the same holder pattern `lower_safe_field` uses
                // for its own receiver (values do not cross blocks, ADR-007).
                let object = self.lower_expr(&field.object);
                let (index, ty) = self.field_position(&field.object, &field.name.name);
                let object_ty = self.type_of(&field.object, field.object.span());
                let held = if self.opens_blocks(&stmt.value) {
                    let holder = self.declare_slot("<assign_object>", object_ty, field.span);
                    self.emit_effect(InstKind::Store(holder, object), stmt.span);
                    Held::Spilled(holder, object_ty)
                } else {
                    Held::Value(object)
                };
                let value = self.lower_expr_as(&stmt.value, ty);
                let object = self.reload(held, field.span);
                self.emit_effect(
                    InstKind::StoreField {
                        object,
                        index,
                        value,
                    },
                    stmt.span,
                );
            }
        }
    }

    /// Lowers `if`/`else` into blocks with a conditional branch.
    ///
    /// ```text
    ///            ┌── condition ──┐
    ///            │   branch      │
    ///            ▼               ▼
    ///          then            else
    ///            │               │
    ///            └──▶ continue ◀─┘
    /// ```
    ///
    /// The continuation block is only created if some branch can reach it: if
    /// both return, creating it would leave an unreachable block with no
    /// terminator.
    fn lower_if(&mut self, stmt: &ast::IfStmt) {
        let condition = self.lower_expr(&stmt.condition);

        let then_block = self.new_block();
        let else_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        self.lower_block(&stmt.then_branch);
        let then_open = !self.is_terminated(self.current);
        let then_end = self.current;

        self.current = else_block;
        match &stmt.else_branch {
            Some(ast::ElseBranch::Block(b)) => self.lower_block(b),
            Some(ast::ElseBranch::If(nested)) => self.lower_if(nested),
            None => {}
        }
        let else_open = !self.is_terminated(self.current);
        let else_end = self.current;

        if !then_open && !else_open {
            // Both branches returned: there is nothing after the conditional.
            return;
        }

        let continue_block = self.new_block();

        if then_open {
            self.current = then_end;
            self.terminate(Terminator::Jump(continue_block));
        }
        if else_open {
            self.current = else_end;
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = continue_block;
    }

    /// Lowers `while`, `loop` and the three-clause `for` into a cycle.
    ///
    /// ```text
    ///     init
    ///      │
    ///      ▼
    ///   ┌ header ──(false)──▶ continue
    ///   │  │(true)
    ///   │  ▼
    ///   │ body ──▶ step ──┐
    ///   └─────────────────┘
    /// ```
    ///
    /// The step is its own block rather than the tail of the body because that
    /// is where `continue` has to jump: skipping it would turn `for` into an
    /// infinite loop the first time someone wrote `continue`.
    fn lower_loop(&mut self, stmt: &ast::LoopStmt) {
        self.scopes.push(HashMap::new());

        if let Some(init) = &stmt.init {
            self.lower_stmt(init);
        }

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        // `do ... while` differs from `while` in exactly this edge: execution
        // enters the body instead of the header, so the condition is first
        // checked after one run. Everything below is shared.
        if stmt.kind.body_runs_first() {
            self.terminate(Terminator::Jump(body_block));
        } else {
            self.terminate(Terminator::Jump(header));
        }

        self.current = header;
        match &stmt.condition {
            Some(condition) => {
                let value = self.lower_expr(condition);
                self.terminate(Terminator::Branch {
                    condition: value,
                    then_block: body_block,
                    else_block: continue_block,
                });
            }
            // `loop` has no exit other than `break`.
            None => self.terminate(Terminator::Jump(body_block)),
        }

        self.loops.push(LoopTargets {
            break_to: continue_block,
            continue_to: step_block,
            try_depth: self.try_stack.len(),
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        if let Some(step) = &stmt.step {
            self.lower_stmt(step);
        }
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    /// Lowers `for x in a..b` into the counter loop it stands for.
    ///
    /// There is no iterator protocol to call into: `ZIRK_LANGUAGE_SPEC.md`
    /// leaves iteration to traits, which are Phase 3. Over a range the loop is
    /// exactly a counter, so it is built directly instead of inventing a
    /// protocol the language does not define yet. Decision D3.
    fn lower_for_in(&mut self, stmt: &ast::ForInStmt) {
        if let ast::Expr::Range(range) = &stmt.iterable {
            return self.lower_for_in_range(stmt, range);
        }
        if self.type_of(&stmt.iterable, stmt.iterable.span()) == IrType::String {
            return self.lower_for_in_string(stmt);
        }
        self.lower_for_in_iterable(stmt);
    }

    /// Lowers `for i in a..b { ... }`.
    fn lower_for_in_range(&mut self, stmt: &ast::ForInStmt, range: &ast::RangeExpr) {
        self.scopes.push(HashMap::new());

        let start = self.lower_expr(&range.start);
        let binding = self.declare_slot(
            &stmt.binding.name,
            IrType::Int(IntWidth::I32),
            stmt.binding.span,
        );
        self.emit_effect(InstKind::Store(binding, start), stmt.span);

        // The end is evaluated once, before the loop: re-evaluating it each
        // iteration would call any function in it repeatedly.
        let end = self.lower_expr(&range.end);
        let limit = self.declare_slot("<range end>", IrType::Int(IntWidth::I32), range.end.span());
        self.emit_effect(InstKind::Store(limit, end), stmt.span);

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let current = self.emit(
            InstKind::Load(binding),
            IrType::Int(IntWidth::I32),
            stmt.span,
        );
        let bound = self.emit(InstKind::Load(limit), IrType::Int(IntWidth::I32), stmt.span);
        let op = if range.inclusive {
            BinaryOp::LtEq
        } else {
            BinaryOp::Lt
        };
        let keep_going = self.emit(
            InstKind::Binary {
                op,
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        self.terminate(Terminator::Branch {
            condition: keep_going,
            then_block: body_block,
            else_block: continue_block,
        });

        self.loops.push(LoopTargets {
            break_to: continue_block,
            continue_to: step_block,
            try_depth: self.try_stack.len(),
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let value = self.emit(
            InstKind::Load(binding),
            IrType::Int(IntWidth::I32),
            stmt.span,
        );
        let one = self.emit(InstKind::ConstInt(1), IrType::Int(IntWidth::I32), stmt.span);
        let next = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: value,
                right: one,
            },
            IrType::Int(IntWidth::I32),
            stmt.span,
        );
        self.emit_effect(InstKind::Store(binding, next), stmt.span);
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    /// Lowers `for c in someString { ... }` (roadmap Phase 3b, task 6.3),
    /// binding each Unicode extended grapheme as a `Char`, in order.
    ///
    /// The same block shape `lower_for_in_range` already uses, with a byte
    /// offset in place of the range's own counter: `GraphemeLenAt` at the
    /// head answers "is there a next grapheme, and how many bytes is it"
    /// (`-1` means done), `GraphemeSlice` in the body builds the `Char`
    /// from the length the head already found (held across the branch the
    /// same way any other value that must survive one is,
    /// `lower_and_hold`), and the step advances the offset by that same
    /// length — never by one, since a byte offset and a grapheme count are
    /// not the same number.
    fn lower_for_in_string(&mut self, stmt: &ast::ForInStmt) {
        self.scopes.push(HashMap::new());

        let string = self.lower_expr(&stmt.iterable);
        let string_slot = self.declare_slot("<string>", IrType::String, stmt.iterable.span());
        self.emit_effect(InstKind::Store(string_slot, string), stmt.span);

        let i64_ty = IrType::Int(IntWidth::I64);
        let offset_slot = self.declare_slot("<offset>", i64_ty, stmt.span);
        let zero = self.const_i64(0, stmt.span);
        self.emit_effect(InstKind::Store(offset_slot, zero), stmt.span);

        let binding = self.declare_slot(&stmt.binding.name, IrType::Char, stmt.binding.span);

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let string_value = self.emit(InstKind::Load(string_slot), IrType::String, stmt.span);
        let offset = self.emit(InstKind::Load(offset_slot), i64_ty, stmt.span);
        let length = self.emit(
            InstKind::GraphemeLenAt {
                string: string_value,
                offset,
            },
            i64_ty,
            stmt.span,
        );
        let minus_one = self.const_i64(-1, stmt.span);
        let keep_going = self.emit(
            InstKind::Binary {
                op: BinaryOp::NotEq,
                left: length,
                right: minus_one,
            },
            IrType::Boolean,
            stmt.span,
        );

        // `length` and `offset` both survive into `body_block`: the header
        // computed them, but values do not cross blocks (ADR-007), so each
        // goes through a slot exactly like `string`/`offset` already do.
        let length_slot = self.declare_slot("<grapheme len>", i64_ty, stmt.span);
        self.emit_effect(InstKind::Store(length_slot, length), stmt.span);

        self.terminate(Terminator::Branch {
            condition: keep_going,
            then_block: body_block,
            else_block: continue_block,
        });

        self.loops.push(LoopTargets {
            break_to: continue_block,
            continue_to: step_block,
            try_depth: self.try_stack.len(),
        });

        self.current = body_block;
        let string_value = self.emit(InstKind::Load(string_slot), IrType::String, stmt.span);
        let offset = self.emit(InstKind::Load(offset_slot), i64_ty, stmt.span);
        let length = self.emit(InstKind::Load(length_slot), i64_ty, stmt.span);
        let grapheme = self.emit(
            InstKind::GraphemeSlice {
                string: string_value,
                offset,
                len: length,
            },
            IrType::Char,
            stmt.span,
        );
        self.emit_effect(InstKind::Store(binding, grapheme), stmt.span);
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let offset = self.emit(InstKind::Load(offset_slot), i64_ty, stmt.span);
        let length = self.emit(InstKind::Load(length_slot), i64_ty, stmt.span);
        let next_offset = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: offset,
                right: length,
            },
            i64_ty,
            stmt.span,
        );
        self.emit_effect(InstKind::Store(offset_slot, next_offset), stmt.span);
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    /// Lowers `for x in it { ... }` over a type's own `Iterable<T>` (roadmap
    /// task 13.5, D8) — `it.iterator()` runs once, then `next()` runs at the
    /// head of every pass, matched against `Iteration<T>`'s two variants the
    /// same way an ordinary `match` on it would (`lower_match`'s own
    /// discriminant-plus-payload pattern): `Item` extracts its associated
    /// value and enters the body, `Done` exits.
    ///
    /// ```text
    ///   it.iterator() ──▶ header: next() ──(Item)──▶ body ──┐
    ///                            │(Done)                    │
    ///                            ▼                          │
    ///                        continue ◀─────────────────────┘
    /// ```
    fn lower_for_in_iterable(&mut self, stmt: &ast::ForInStmt) {
        let iterable_contract = self.native_contract_id("Iterable");
        let iterator_contract = self.native_contract_id("Iterator");

        let &instance = self.checked.for_in_iteration.get(&stmt.iterable.span()).expect(
            "the checker records which `Iteration<T>` a loop over a type's own `Iterable<T>` needs",
        );
        let iteration_id = self.enum_instance_base + instance;
        let item = self.checked.enums[iteration_id as usize]
            .discriminant("Item")
            .expect("the native `Iteration<T>` always has an `Item` variant");
        let item_field = self.module.enums[iteration_id as usize].variants[item as usize][0];
        let element_ty = self.module.enums[iteration_id as usize].fields[item_field as usize].ty;

        // `it.iterator()`, called once — not on every pass, the same reason
        // an ordinary range's own end is evaluated once above.
        let iterable = self.lower_expr(&stmt.iterable);
        let iterator_index = self.checked.contracts[iterable_contract as usize]
            .method("iterator")
            .expect("`Iterable<T>` always declares `iterator`")
            .index as u32;
        let iterator_ty = IrType::Contract(iterator_contract);
        let iterator = self.emit(
            InstKind::CallContract {
                object: iterable,
                contract: iterable_contract,
                index: iterator_index,
                args: Vec::new(),
            },
            iterator_ty,
            stmt.span,
        );
        let iterator_slot = self.declare_slot("<iterator>", iterator_ty, stmt.span);
        self.emit_effect(InstKind::Store(iterator_slot, iterator), stmt.span);

        self.scopes.push(HashMap::new());
        let binding = self.declare_slot(&stmt.binding.name, element_ty, stmt.binding.span);

        let header = self.new_block();
        let body_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));
        self.current = header;

        let next_index = self.checked.contracts[iterator_contract as usize]
            .method("next")
            .expect("`Iterator<T>` always declares `next`")
            .index as u32;
        let held_iterator = self.emit(InstKind::Load(iterator_slot), iterator_ty, stmt.span);
        let iteration_ty = IrType::Enum(iteration_id);
        let iteration = self.emit(
            InstKind::CallContract {
                object: held_iterator,
                contract: iterator_contract,
                index: next_index,
                args: Vec::new(),
            },
            iteration_ty,
            stmt.span,
        );
        let iteration_slot = self.declare_slot("<iteration>", iteration_ty, stmt.span);
        self.emit_effect(InstKind::Store(iteration_slot, iteration), stmt.span);

        let held = self.emit(InstKind::Load(iteration_slot), iteration_ty, stmt.span);
        let discriminant = self.emit(
            InstKind::Discriminant(held),
            IrType::Int(IntWidth::I32),
            stmt.span,
        );
        let expected = self.emit(
            InstKind::ConstInt(item as i32),
            IrType::Int(IntWidth::I32),
            stmt.span,
        );
        let is_item = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: discriminant,
                right: expected,
            },
            IrType::Boolean,
            stmt.span,
        );
        self.terminate(Terminator::Branch {
            condition: is_item,
            then_block: body_block,
            else_block: continue_block,
        });

        self.loops.push(LoopTargets {
            break_to: continue_block,
            continue_to: header,
            try_depth: self.try_stack.len(),
        });

        self.current = body_block;
        let held = self.emit(InstKind::Load(iteration_slot), iteration_ty, stmt.span);
        let value = self.emit(
            InstKind::LoadField {
                object: held,
                index: item_field,
            },
            element_ty,
            stmt.binding.span,
        );
        self.emit_effect(InstKind::Store(binding, value), stmt.binding.span);
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(header));

        self.loops.pop();

        self.current = continue_block;
        self.scopes.pop();
    }

    /// The id of a contract the language itself registers, by name — the
    /// language never lets application code reopen `Iterable`/`Iterator`
    /// (`Self::is_native_contract_name` on the checker side), so each is
    /// registered exactly once, unconditionally, before any program
    /// declaration is checked (task 6.9).
    fn native_contract_id(&self, name: &str) -> u32 {
        self.checked
            .contracts
            .iter()
            .position(|c| c.name == name)
            .expect("the language registers this contract unconditionally") as u32
    }

    fn lower_break(&mut self, _stmt: &ast::JumpStmt) {
        let loop_target = self
            .loops
            .last()
            .expect("a verified program only breaks inside a loop");
        let target = loop_target.break_to;
        self.run_finally_through(loop_target.try_depth);
        self.terminate(Terminator::Jump(target));

        // Statements after a `break` are unreachable, but the block they would
        // land in still needs to exist and be terminated.
        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_continue(&mut self, _stmt: &ast::JumpStmt) {
        let loop_target = self
            .loops
            .last()
            .expect("a verified program only continues inside a loop");
        let target = loop_target.continue_to;
        self.run_finally_through(loop_target.try_depth);
        self.terminate(Terminator::Jump(target));

        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_return(&mut self, stmt: &ast::ReturnStmt) {
        let expected = self.return_type;
        let value = stmt.value.as_ref().map(|e| self.lower_expr_as(e, expected));
        // Spilled ahead of `run_finally_through` below, the same reason
        // `Self::lower_and_hold` spills any operand a later, block-opening
        // expression would otherwise strand (ADR-007): a `finally` on the
        // way out may itself open blocks — a call inside it does,
        // unconditionally, since D11 (`fase-4d-runtimeerror`) — and `value`
        // was computed in the block *before* that, so using it directly in
        // this statement's own `Terminator::Return` below (built in
        // whichever block lowering the `finally` left `self.current` at)
        // would use a value from a block this one is no longer that same
        // block (found via a `return call();` inside a `match ... with`
        // arm whose own resource-close `finally` calls a `Void` method).
        let holder = value.map(|operand| {
            let ty = self.type_of_operand(operand);
            let slot = self.declare_slot("<return_value>", ty, stmt.span);
            self.emit_effect(InstKind::Store(slot, operand), stmt.span);
            (slot, ty)
        });
        // A `return` leaves every enclosing `try`, not just the innermost
        // one (roadmap Phase 4b, design D3) — unlike `break`/`continue`,
        // which only exit the ones nested *inside* the loop they target.
        self.run_finally_through(0);
        let value = holder.map(|(slot, ty)| self.emit(InstKind::Load(slot), ty, stmt.span));
        self.terminate(Terminator::Return(value));
    }

    /// Runs the `finally` of every active `try` frame from `depth` onward,
    /// innermost first — what a `return`/`break`/`continue` leaving one or
    /// more enclosing `try`s owes each of them on the way out (roadmap
    /// Phase 4b, design D3). A plain slice rather than popping
    /// `try_stack`: the frames are still active for whatever this statement
    /// does *not* leave (a `break` only exits as far as its own loop).
    fn run_finally_through(&mut self, depth: usize) {
        let finally_blocks: Vec<Option<ast::Block>> = self.try_stack[depth..]
            .iter()
            .rev()
            .map(|frame| frame.finally.clone())
            .collect();
        for finally in finally_blocks {
            self.lower_finally_block(&finally);
        }
    }

    // --- Expressions ------------------------------------------------------

    fn lower_expr(&mut self, expr: &ast::Expr) -> Operand {
        let span = expr.span();

        match expr {
            ast::Expr::Int(lit) => self.emit(
                InstKind::ConstInt(lit.value as i32),
                IrType::Int(IntWidth::I32),
                span,
            ),
            ast::Expr::Float(lit) => {
                let width = float_literal_width(lit);
                self.emit(
                    InstKind::ConstFloat(width, lit.text.clone()),
                    IrType::Float(width),
                    span,
                )
            }
            ast::Expr::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Expr::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }
            ast::Expr::Char(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstChar(id), IrType::Char, span)
            }

            ast::Expr::Path(ident) => {
                if let Some(slot) = self.try_lookup_slot(&ident.name) {
                    let ty = self.slot_type(slot);
                    self.emit(InstKind::Load(slot), ty, span)
                } else {
                    // A bare function name with no local shadowing it is a
                    // value (`Checker::check_path`, roadmap Phase 4d,
                    // design D12): the canonical, capture-less closure the
                    // `Self::lower` pre-pass already reserved for this
                    // shape, targeting this specific function.
                    let (id, target) = self
                        .named_function_value(&ident.name, ident.span)
                        .expect("a verified program only names declared variables or functions");
                    self.emit(
                        InstKind::MakeClosure {
                            id,
                            captures: Vec::new(),
                            target,
                        },
                        IrType::Closure(id),
                        span,
                    )
                }
            }

            ast::Expr::Unary(e) => {
                let operand = self.lower_expr(&e.operand);
                let (op, ty) = match e.op {
                    // `Neg`/`BitNot` preserve the operand's own width — any
                    // integer width for both, plus any `Float` width for
                    // `Neg` (roadmap Phase 3b) — so the result type is read
                    // from the value just lowered, not assumed to be
                    // `Int32`.
                    ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
                        let op = if e.op == ast::UnaryOp::Neg {
                            UnaryOp::Neg
                        } else {
                            UnaryOp::BitNot
                        };
                        (op, self.type_of_operand(operand))
                    }
                    ast::UnaryOp::Not => (UnaryOp::Not, IrType::Boolean),
                };
                self.emit(InstKind::Unary { op, operand }, ty, span)
            }

            ast::Expr::Binary(e) if e.op == ast::BinaryOp::Coalesce => self.lower_coalesce(e, span),

            // `&&` and `||` must not evaluate their right operand when the
            // left already decides the answer.
            ast::Expr::Binary(e) if matches!(e.op, ast::BinaryOp::And | ast::BinaryOp::Or) => {
                self.lower_short_circuit(e, span)
            }

            // A user type supplies the operator through its reserved method,
            // so the call is what the operator *is* — not a special form the
            // backend would need to know about.
            ast::Expr::Binary(e) if self.operator_method_of(e).is_some() => {
                let (id, name) = self.operator_method_of(e).expect("checked above");
                let class = &self.checked.classes[id as usize];
                let method = class.method(&name).expect("the checker resolved it");
                let symbol = body_symbol(self.checked, method);
                let returns = self.ir_type(method.returns);
                let param = method
                    .params
                    .first()
                    .map(|p| self.ir_type(p.ty))
                    .expect("an operator method takes one operand");

                let held = self.lower_and_hold(&e.left, self.opens_blocks(&e.right));
                let right = self.lower_expr_as(&e.right, param);
                let left = self.reload(held, e.left.span());

                let result = self.emit(
                    InstKind::Call {
                        callee: symbol,
                        args: vec![left, right],
                    },
                    returns,
                    span,
                );

                if e.op == ast::BinaryOp::NotEq {
                    return self.emit(
                        InstKind::Unary {
                            op: UnaryOp::Not,
                            operand: result,
                        },
                        IrType::Boolean,
                        span,
                    );
                }
                result
            }

            ast::Expr::Binary(e)
                if is_string_operator(
                    self.type_of(&e.left, e.left.span()),
                    self.type_of(&e.right, e.right.span()),
                    e.op,
                ) =>
            {
                let held = self.lower_and_hold(&e.left, self.opens_blocks(&e.right));
                let right = self.lower_expr(&e.right);
                let left = self.reload(held, e.left.span());

                if e.op == ast::BinaryOp::Add {
                    self.emit(InstKind::Concat { left, right }, IrType::String, span)
                } else {
                    let (string, count) = if self.type_of(&e.left, e.left.span()) == IrType::String
                    {
                        (left, right)
                    } else {
                        // `3 * "ja"`: the same request, written the other way.
                        (right, left)
                    };
                    // D10: the negative-count check that used to live inside
                    // `zirk_rt_invalid_repeat` (`zirk-runtime/src/string.rs`)
                    // moved here, ahead of the repetition itself.
                    self.checked_repeat(string, count, span)
                }
            }

            ast::Expr::Binary(e) => {
                let held = self.lower_and_hold(&e.left, self.opens_blocks(&e.right));
                let right = self.lower_expr(&e.right);
                let left = self.reload(held, e.left.span());
                let op = binary_op(e.op);

                let left_ty = self.type_of(&e.left, e.left.span());
                let right_ty = self.type_of(&e.right, e.right.span());

                // Mixed integer/`Float` arithmetic implicitly widens the
                // integer operand to the `Float` operand's own width before
                // the operation (`ZIRK_LANGUAGE_SPEC.md` section 3) — the
                // checker already accepted this specific combination
                // (`native_arithmetic`'s mixed arm), so lowering only has to
                // insert the conversion the verifier's `Binary` check then
                // finds both operands already agreeing on. Every other
                // operator the checker allows through here (comparison,
                // bitwise, shift, equality) already requires the same type
                // on both sides, so this never fires for them — except `is`:
                // unlike `==`, the checker deliberately lets `T is T?`
                // through (`Checker::check_binary`'s `Is` arm has no
                // `reject_nullable_comparison`, on purpose — identity is
                // exactly the one place absence is allowed to participate,
                // `ZIRK_LANGUAGE_SPEC.md` §4), the same way `T` widens to
                // `T?` anywhere else it is assigned. `Wrap` here mirrors that
                // widening so both operands reach the IR already agreeing on
                // type (verify.rs's `Binary` check requires it).
                let (left, right, operand_type) = match (left_ty, right_ty) {
                    (IrType::Int(_), IrType::Float(w)) => {
                        let left =
                            self.emit(InstKind::IntToFloat(left), IrType::Float(w), e.left.span());
                        (left, right, IrType::Float(w))
                    }
                    (IrType::Float(w), IrType::Int(_)) => {
                        let right = self.emit(
                            InstKind::IntToFloat(right),
                            IrType::Float(w),
                            e.right.span(),
                        );
                        (left, right, IrType::Float(w))
                    }
                    (base, IrType::Nullable(_))
                        if op == BinaryOp::Identical && !matches!(base, IrType::Nullable(_)) =>
                    {
                        let nb = Nullable::of(base)
                            .expect("`is` only ever reaches a type with identity, and every one has a nullable form");
                        let left = self.emit(
                            InstKind::Wrap {
                                base: nb,
                                value: left,
                            },
                            right_ty,
                            e.left.span(),
                        );
                        (left, right, right_ty)
                    }
                    (IrType::Nullable(_), base)
                        if op == BinaryOp::Identical && !matches!(base, IrType::Nullable(_)) =>
                    {
                        let nb = Nullable::of(base)
                            .expect("`is` only ever reaches a type with identity, and every one has a nullable form");
                        let right = self.emit(
                            InstKind::Wrap {
                                base: nb,
                                value: right,
                            },
                            left_ty,
                            e.right.span(),
                        );
                        (left, right, left_ty)
                    }
                    _ => (left, right, left_ty),
                };

                self.emit_checked_binary(op, left, right, operand_type, span)
            }

            // A recursive lambda calling itself by its own binding name
            // (`ZIRK_LANGUAGE_SPEC.md` section 6, roadmap Phase 4d): an
            // ordinary, direct recursive call to this same lifted function,
            // forwarding its own captures unchanged ahead of the new call's
            // arguments — there is no closure *value* to call through here,
            // since none exists yet at the point this closure literal is
            // still being built (`Self::lower_lambda`'s own doc comment).
            ast::Expr::Call(e) if self.is_recursive_self_call(e) => {
                let (_, target, capture_slots, param_types) = self
                    .recursive_call
                    .clone()
                    .expect("checked by `is_recursive_self_call`");
                let mut args: Vec<Operand> = capture_slots
                    .iter()
                    .map(|slot| self.emit(InstKind::Load(*slot), self.slot_type(*slot), span))
                    .collect();
                args.extend(self.lower_held_args(&e.args, &param_types));
                self.emit(
                    InstKind::Call {
                        callee: target,
                        args,
                    },
                    self.return_type,
                    span,
                )
            }

            // Calling a closure value goes through the value, not a name.
            ast::Expr::Call(e) if self.is_closure_call(e) => {
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                let branching = e.args.iter().any(|a| self.opens_blocks(&a.value));
                let held = self.lower_and_hold(&e.callee, branching);

                let expected = self.module.closures[id as usize].params.clone();
                let returns = self.module.closures[id as usize].returns;
                let args = self.lower_held_args(&e.args, &expected);
                let callee = self.reload(held, e.callee.span());
                self.emit(InstKind::CallClosure { id, callee, args }, returns, span)
            }

            ast::Expr::Call(e) => {
                if self.is_pointer_from_call(e) {
                    return self.lower_pointer_from(&e.args[0].value, span);
                }
                if self.is_pointer_method_call(e) {
                    return self.lower_pointer_method_call(e, span);
                }
                if let Some(target) = self.context_conversion_target(e) {
                    let target_ty = self.ir_type(target);
                    return self.lower_context_tree(target_ty, &e.args[0].value);
                }
                if self.is_fatal_error_call(e) {
                    return self.lower_fatal_error_call(e, span);
                }
                if let Some(operand) = self.lower_super_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_contract_call(e, span) {
                    let ty = self.type_of_operand(operand);
                    return self.lower_throws_check(operand, ty, span);
                }
                if let Some(operand) = self.lower_native_to_string_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_result_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_method_call(e, span) {
                    let ty = self.type_of_operand(operand);
                    return self.lower_throws_check(operand, ty, span);
                }
                if let Some(operand) = self.lower_safe_method_call(e, span) {
                    return operand;
                }
                if let Some((enum_id, variant)) = self.variant_construction(e) {
                    return self.lower_variant_construction(e, enum_id, variant, span);
                }
                if let Some(id) = self.construction_class_id(e) {
                    return if matches!(
                        self.checked.classes[id as usize].kind,
                        ast::ClassKind::Record | ast::ClassKind::ValueClass
                    ) {
                        self.lower_record_construction(e, id, span)
                    } else {
                        self.lower_construction(e, id, span)
                    };
                }
                let name = self.callee_name(e);
                let args = self.lower_args(e);
                let returns = self.signature_return(&name);
                let result = self.emit(InstKind::Call { callee: name, args }, returns, span);
                self.lower_throws_check(result, returns, span)
            }

            ast::Expr::If(e) => self.lower_if_expr(e, span),
            ast::Expr::Field(e) => self.lower_field(e, span),
            ast::Expr::This(_) | ast::Expr::Super(_) => {
                // `super` names where to look, not what to look at: the value
                // is the same instance.
                let slot = self.lookup_slot("this");
                self.emit(InstKind::Load(slot), self.slot_type(slot), span)
            }
            ast::Expr::Ternary(e) => self.lower_ternary(e, span),
            ast::Expr::Increment(e) => self.lower_increment(e, span),
            ast::Expr::Match(e) => self.lower_match(e, span),
            ast::Expr::Variant(e) => {
                let value = self.discriminant(&e.enum_name, &e.variant.name);
                self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I32), span)
            }

            ast::Expr::Println(e) => {
                let operand = self.lower_println_argument(e, span);
                self.emit(InstKind::Println(operand), IrType::Void, span)
            }

            // The checker rejects these before lowering runs; see
            // `zirk_sema` and the tasks still open for this phase.
            ast::Expr::Lambda(e) => self.lower_lambda(e, span),

            ast::Expr::Cast(e) => self.lower_cast(e, span),

            ast::Expr::Interpolated(e) => self.lower_interpolated(e, span),

            // `unsafe { }`/`commit { }` used as a value (roadmap Phase 4e) —
            // same no-journal lowering as the statement form (`Self::lower_stmt`'s
            // own comment on the D5/D6 cut), evaluated for its last
            // expression instead of only its effects.
            ast::Expr::Unsafe(u) => self.lower_block_value(&u.body),
            ast::Expr::Commit(c) => self.lower_block_value(&c.body),

            // `null` has no type of its own: it only appears where a
            // destination supplies one, and `lower_expr_as` handles it there.
            ast::Expr::Null(_) | ast::Expr::Range(_) => {
                unreachable!("lowering received a construct the checker should have rejected")
            }
        }
    }

    /// Lowers `&&` and `||` so the right operand only runs when it matters.
    ///
    /// ```text
    ///   a ──(decides)──▶ result = <that answer>  ──┐
    ///   │(undecided)                               │
    ///   ▼                                          ▼
    ///   result = b ──────────────────────────▶ continue
    /// ```
    ///
    /// `false && f()` and `true || f()` must not call `f`. Evaluating both
    /// operands was the Phase 1 behaviour, noted there as pending precisely
    /// because short-circuiting needs blocks of its own.
    fn lower_short_circuit(&mut self, expr: &ast::BinaryExpr, span: Span) -> Operand {
        let is_and = expr.op == ast::BinaryOp::And;
        let result = self.declare_slot("<logic>", IrType::Boolean, span);

        let left = self.lower_expr(&expr.left);

        let rest_block = self.new_block();
        let decided_block = self.new_block();
        let continue_block = self.new_block();

        // `&&` continues when the left is true; `||` when it is false.
        let (then_block, else_block) = if is_and {
            (rest_block, decided_block)
        } else {
            (decided_block, rest_block)
        };

        self.terminate(Terminator::Branch {
            condition: left,
            then_block,
            else_block,
        });

        // The left operand already decided: `false` for `&&`, `true` for `||`.
        self.current = decided_block;
        let decided = self.emit(InstKind::ConstBool(!is_and), IrType::Boolean, span);
        self.emit_effect(InstKind::Store(result, decided), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = rest_block;
        let right = self.lower_expr(&expr.right);
        self.emit_effect(InstKind::Store(result, right), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), IrType::Boolean, span)
    }

    /// Lowers `a ?? b` into an explicit null check with two blocks.
    ///
    /// ```text
    ///   is_null(a) ──(yes)──▶ evaluate b ──┐
    ///        │(no)                         │
    ///        ▼                             ▼
    ///   unwrap(a) ────────────────────▶ continue
    /// ```
    ///
    /// The fallback lives in its own block so it is only evaluated when the
    /// value is absent: `a ?? expensive()` must not call `expensive` when `a`
    /// holds a value.
    fn lower_coalesce(&mut self, expr: &ast::BinaryExpr, span: Span) -> Operand {
        let left_type = self.type_of(&expr.left, expr.left.span());
        // A literal `null` fallback (`nullable ?? null`) has no type of its
        // own — `type_of` on `Expr::Null` is `unreachable!` on purpose,
        // because everywhere else a destination type supplies it first. Here
        // the only sensible destination is the left side's own nullable
        // type: the fallback keeps the result exactly as nullable as `a`
        // already was, so this stands in for `type_of` instead of calling it.
        let right_type = match &*expr.right {
            ast::Expr::Null(_) => left_type,
            _ => self.type_of(&expr.right, expr.right.span()),
        };
        // The fallback decides: when it may itself be absent, so may the
        // result; otherwise the result always holds a value.
        let result_type = if matches!(right_type, IrType::Nullable(_)) {
            right_type
        } else {
            left_type.unwrapped()
        };
        let result = self.declare_slot("<coalesce>", result_type, span);

        let value = self.lower_expr(&expr.left);
        let test = self.emit(InstKind::IsNull(value), IrType::Boolean, span);

        // The value is needed again in the block that unwraps it, and values do
        // not cross blocks (ADR-007).
        let holder = self.declare_slot("<coalesced>", left_type, span);
        self.emit_effect(InstKind::Store(holder, value), span);

        let absent_block = self.new_block();
        let present_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition: test,
            then_block: absent_block,
            else_block: present_block,
        });

        self.current = absent_block;
        let fallback = self.lower_expr_as(&expr.right, result_type);
        self.emit_effect(InstKind::Store(result, fallback), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = present_block;
        let held = self.emit(InstKind::Load(holder), self.slot_type(holder), span);
        let inner = self.emit(
            InstKind::Unwrap(held),
            self.slot_type(holder).unwrapped(),
            span,
        );
        // The result may still be nullable when the fallback is: widening the
        // unwrapped value back keeps both branches storing the same type.
        let inner = match result_type {
            IrType::Nullable(base) => {
                self.emit(InstKind::Wrap { base, value: inner }, result_type, span)
            }
            _ => inner,
        };
        self.emit_effect(InstKind::Store(result, inner), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), result_type, span)
    }

    /// Lowers a lambda into a module function plus a closure value.
    ///
    /// The body becomes a function whose leading parameters are the captures,
    /// and the value pairs a pointer to it with the captured values themselves.
    /// A capturing closure's value carries every capture inline
    /// (`MakeClosure`), so it needs nothing from the frame that created it
    /// once built — it is already safe to escape that frame (roadmap Phase
    /// 4d, design D12): only *naming* the escaped position (`Fn(...) => R`
    /// in a return type, a field, a typed local) was the gap D9 left, not
    /// this representation.
    fn lower_lambda(&mut self, expr: &ast::LambdaExpr, span: Span) -> Operand {
        let info = self
            .checked
            .lambdas
            .get(&expr.span)
            .expect("the checker records every lambda it accepted");

        // A *recursive* lambda's own binding (`ZIRK_LANGUAGE_SPEC.md`
        // section 6, roadmap Phase 4d) is a capture for the checker's own
        // type-checking purposes, but it is excluded here: no value for it
        // exists yet at the point this literal is still being *built* (its
        // own enclosing `let`/`mut` has not finished storing anything into
        // it — `zirk-ir`'s own `lower_let` only declares that slot *after*
        // lowering this very initializer). A call to it inside this body is
        // lowered as a direct recursive call to this lifted function
        // instead (`Self::lift_lambda_body`'s own `recursive_call`
        // parameter) — see this lambda's own lifted body, below.
        let real_captures: Vec<&Capture> = info
            .captures
            .iter()
            .filter(|c| info.recursive_binding.as_deref() != Some(c.name.as_str()))
            .collect();

        // The types come from the slots the captures live in, not from the
        // checker's types. A captured closure is the reason: the checker
        // identifies a closure type by its own numbering and the IR by the
        // layout it built, and only the slot knows which layout this one is.
        let capture_slots: Vec<SlotId> = real_captures
            .iter()
            .map(|c| self.lookup_slot(&c.name))
            .collect();
        let capture_types: Vec<IrType> = capture_slots
            .iter()
            .map(|slot| self.slot_type(*slot))
            .collect();
        let signature = &self.checked.fn_types[info.fn_type as usize];
        let param_types: Vec<IrType> = signature.params.iter().map(|t| self.ir_type(*t)).collect();
        let returns = self.ir_type(signature.returns);

        // The name reaches the object file as a symbol, so it may only use
        // characters every target's assembler accepts. `#` starts a comment in
        // x86_64's AT&T syntax — with `<lambda>#0` the symbol broke there and
        // nowhere else, since aarch64 comments with `//`.
        //
        // A dot is valid in ELF, Mach-O and COFF symbols, and impossible in a
        // Zirk identifier, so it cannot collide with a user's function. The
        // lambda's own span (file plus source offset) is unique across the
        // whole program, unlike `module.closures.len()` — that no longer
        // tracks "one entry per lambda occurrence" now that a capture-less
        // lambda shares its canonical id with every other value of the same
        // shape (design D12).
        let name = format!("lambda.{}.{}", expr.span.file.0, expr.span.start);
        let id = info.fn_type;

        // A capture-less lambda's id is the canonical, capture-less
        // `ClosureLayout` `Self::lower` already pre-populated for it (D12) —
        // shared with every other value of the same shape, so nothing here
        // overwrites it; only the target this particular `MakeClosure`
        // embeds is specific to this lambda. A capturing lambda's id
        // (design D14) instead names a placeholder `Self::lower` reserved
        // just for this one literal — filled in for real right here, the
        // first (and, by construction, only) time it is lowered. Gated on
        // the checker's own capture list, not `capture_types` above: a
        // *purely* self-recursive lambda (no capture besides itself) still
        // reserved a placeholder id (it is not capture-less at the checker
        // level) that needs its real, non-`Void` shape filled in here too.
        if !info.captures.is_empty() {
            self.module.closures[id as usize] = ClosureLayout {
                captures: capture_types.clone(),
                params: param_types.clone(),
                returns,
            };
        }

        // The captures are read in the enclosing function, where their slots
        // live, before the body is lifted out.
        let captures: Vec<Operand> = capture_slots
            .iter()
            .map(|slot| self.emit(InstKind::Load(*slot), self.slot_type(*slot), span))
            .collect();

        let names: Vec<String> = real_captures
            .iter()
            .map(|c| c.name.clone())
            .chain(expr.params.iter().map(|p| p.name.name.clone()))
            .collect();
        let types: Vec<IrType> = capture_types.into_iter().chain(param_types).collect();

        let recursive_call = info
            .recursive_binding
            .as_deref()
            .map(|source| (source, real_captures.len()));
        let body = self.lift_lambda_body(expr, &name, &names, &types, returns, recursive_call);
        self.lifted.push(body);

        self.emit(
            InstKind::MakeClosure {
                id,
                captures,
                target: name,
            },
            IrType::Closure(id),
            span,
        )
    }

    /// Lowers a lambda body as a function of its own.
    ///
    /// A fresh lowering is used rather than reusing this one: the body has its
    /// own slots and blocks, and sharing them would let a capture resolve to
    /// the enclosing function's slot instead of to the parameter that carries
    /// its copy.
    fn lift_lambda_body(
        &mut self,
        expr: &ast::LambdaExpr,
        name: &str,
        params: &[String],
        types: &[IrType],
        returns: IrType,
        // The source name that refers to this lambda recursively, and how
        // many of `params`' leading entries are real captures to forward
        // unchanged on a self-call (`Self::lower_lambda`'s own
        // `real_captures.len()`, *after* excluding the recursive binding
        // itself from the capture list).
        recursive_binding: Option<(&str, usize)>,
    ) -> Function {
        let mut inner = FunctionLowering::new(
            self.module,
            self.checked,
            self.declarations,
            self.instance_base,
            self.enum_instance_base,
        );
        inner.return_type = returns;

        let entry = inner.new_block();
        inner.current = entry;
        inner.scopes.push(HashMap::new());

        let slots: Vec<SlotId> = params
            .iter()
            .zip(types)
            .map(|(name, ty)| inner.declare_slot(name, *ty, expr.span))
            .collect();

        // Set only now that `slots` (this function's own leading
        // parameters) exists: the forwarded prefix is the real captures,
        // already-declared slots; the rest are this lambda's own ordinary
        // parameter types, fixed regardless of what locals the body itself
        // goes on to declare deeper inside it.
        inner.recursive_call = recursive_binding.map(|(source, forwarded)| {
            (
                source.to_string(),
                name.to_string(),
                slots[..forwarded].to_vec(),
                types[forwarded..].to_vec(),
            )
        });

        match &*expr.body {
            ast::LambdaBody::Expr(e) => {
                let value = inner.lower_expr_as(e, returns);
                inner.terminate(Terminator::Return(Some(value)));
            }
            ast::LambdaBody::Block(b) => {
                inner.lower_block(b);
                inner.terminate(Terminator::Return(None));
            }
        }

        inner.scopes.pop();

        // A lambda may itself contain lambdas.
        let nested = std::mem::take(&mut inner.lifted);
        self.lifted.extend(nested);

        Function {
            name: name.to_string(),
            params: slots,
            return_type: returns,
            gc_roots: gc_roots_of(inner.module, &inner.slots),
            slots: inner.slots,
            blocks: inner.blocks,
            entry,
            span: expr.span,
        }
    }

    /// The discriminant a variant lowers to.
    fn discriminant(&self, enum_name: &ast::Ident, variant: &str) -> i32 {
        let declared = self.declaration_of(&enum_name.name, enum_name.span);
        self.checked
            .enums
            .iter()
            .find(|e| e.name == declared)
            .and_then(|e| e.discriminant(variant))
            .expect("a verified program only names declared variants") as i32
    }

    /// The id a variant pattern's or construction's enum name resolves to.
    fn enum_id_of(&self, enum_name: &ast::Ident) -> u32 {
        let declared = self.declaration_of(&enum_name.name, enum_name.span);
        self.checked
            .enums
            .iter()
            .position(|e| e.name == declared)
            .expect("a verified program only names declared enums") as u32
    }

    /// Lowers a `match` into a chain of comparisons.
    ///
    /// ```text
    ///   test₁ ──(no)──▶ test₂ ──(no)──▶ … ──▶ default
    ///     │(yes)          │(yes)
    ///     ▼               ▼
    ///   arm₁            arm₂ ──▶ continue ◀── …
    /// ```
    ///
    /// A chain rather than a jump table: the backend recognizes the shape and
    /// emits a `switch` when the arms are dense, and building the table here
    /// would duplicate an optimization LLVM already does.
    fn lower_match(&mut self, expr: &ast::MatchExpr, span: Span) -> Operand {
        let scrutinee_type = self.type_of(&expr.scrutinee, expr.scrutinee.span());
        let value = self.lower_expr(&expr.scrutinee);

        // Values do not cross blocks in this IR (ADR-007), and the chain tests
        // the same value in each of them: it goes through a slot, and each
        // block reloads it.
        let scrutinee = self.declare_slot("<scrutinee>", scrutinee_type, span);
        self.emit_effect(InstKind::Store(scrutinee, value), span);

        // Mirrors the checker's own narrowing (`Checker::check_match`): a
        // `null` arm already took the only case in which the scrutinee is
        // absent, so every other arm's own binding is the unwrapped type,
        // not the raw nullable slot.
        let has_null_arm = expr
            .arms
            .iter()
            .any(|arm| matches!(arm.pattern, ast::Pattern::Null(_)));
        let narrowed_type = if has_null_arm {
            scrutinee_type.unwrapped()
        } else {
            scrutinee_type
        };

        // A `match` used as a statement produces nothing, and a slot has no
        // `Void` form to hold it. `arm_value_type` needs the first arm's own
        // bound names in scope before any block for its body exists — a
        // binding pattern's body may read the very name it binds, the same
        // way a variant destructure's may read a field it names — so those
        // are registered here, type-only, with nothing emitted for them:
        // the real bindings for that arm are made again inside the loop
        // below, where there is a body block for a load to belong to.
        self.scopes.push(HashMap::new());
        if let Some(first) = expr.arms.first() {
            let first_type = if matches!(first.pattern, ast::Pattern::Null(_)) {
                scrutinee_type
            } else {
                narrowed_type
            };
            self.declare_pattern_types(&first.pattern, first_type);
        }
        let result_type = self.arm_value_type(expr);
        self.scopes.pop();
        let result =
            (result_type != IrType::Void).then(|| self.declare_slot("<match>", result_type, span));

        let continue_block = self.new_block();
        let mut reachable = false;

        for arm in &expr.arms {
            // An irrefutable pattern takes the value unconditionally, and
            // anything after it is unreachable.
            let body_block = if arm.pattern.is_irrefutable() {
                let block = self.new_block();
                self.terminate(Terminator::Jump(block));
                block
            } else {
                let test = self.lower_pattern_test(
                    &arm.pattern,
                    scrutinee,
                    scrutinee_type,
                    narrowed_type,
                    span,
                );

                let body_block = self.new_block();
                let next_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: test,
                    then_block: body_block,
                    else_block: next_block,
                });
                self.current = next_block;
                body_block
            };

            let resume = self.current;
            self.current = body_block;

            self.scopes.push(HashMap::new());
            // A binding pattern names the scrutinee inside its arm — narrowed
            // to the unwrapped type when a sibling `null` arm already took
            // the absent case (mirrors `Checker::check_match`'s own
            // narrowing).
            if let ast::Pattern::Binding(ident) = &arm.pattern {
                let current = self.load_scrutinee_narrowed(
                    scrutinee,
                    scrutinee_type,
                    narrowed_type,
                    ident.span,
                );
                let slot = self.declare_slot(&ident.name, narrowed_type, ident.span);
                self.emit_effect(InstKind::Store(slot, current), ident.span);
            }
            // A variant pattern destructures its own associated fields, each
            // into the name its sub-pattern binds (roadmap task 11.4) — the
            // checker only accepts a binding or a wildcard here, so a
            // wildcard is the only other shape this sees.
            if let ast::Pattern::Variant(v) = &arm.pattern
                && !v.bindings.is_empty()
            {
                // The discriminant's own order is identical between a
                // generic enum's template and any of its specializations
                // (`specialize_enum` copies variants verbatim), so the
                // *template* id is right for it — but the module-level
                // `EnumLayout` (field indices, concrete field types) is not:
                // a generic template's own layout is an empty placeholder
                // (roadmap task 13.5's specialization pass builds the real
                // one per instantiation), so that id has to come from the
                // scrutinee's own already-specialized `IrType::Enum`
                // instead of re-resolving the pattern's bare written name
                // (roadmap Phase 4a, found via `Result<T,E>`, the first
                // generic enum a program ever matches by hand — `for ... in`
                // over `Iteration<T>` never went through this path).
                let template_id = self.enum_id_of(&v.enum_name);
                let variant = self
                    .checked
                    .enums
                    .get(template_id as usize)
                    .and_then(|e| e.discriminant(&v.variant.name))
                    .expect("the checker resolved this variant");
                // A sibling `null` arm already narrowed a nullable scrutinee
                // (`narrowed_type`) down to the bare enum this pattern
                // destructures.
                let IrType::Enum(module_id) = narrowed_type else {
                    unreachable!("a variant pattern with bindings matches an enum with a payload")
                };
                let indices =
                    self.module.enums[module_id as usize].variants[variant as usize].clone();
                let object =
                    self.load_scrutinee_narrowed(scrutinee, scrutinee_type, narrowed_type, span);
                for (sub_pattern, index) in v.bindings.iter().zip(indices) {
                    let ast::Pattern::Binding(ident) = sub_pattern else {
                        continue;
                    };
                    let field_ty = self.module.enums[module_id as usize].fields[index as usize].ty;
                    let value =
                        self.emit(InstKind::LoadField { object, index }, field_ty, ident.span);
                    let slot = self.declare_slot(&ident.name, field_ty, ident.span);
                    self.emit_effect(InstKind::Store(slot, value), ident.span);
                }
            }

            // `match ... with binding` (roadmap Phase 4c): whichever arm's
            // own pattern binds `binding` owns the acquired `Resource<E>`
            // and must close it on every exit — reuses the `try`/`finally`
            // machinery (design D3, `fase-4b-excepciones/design.md`)
            // wholesale, with no `catch` of its own, so `run_finally_through`
            // already closes it on a `return`/`break`/`continue` written
            // inside the arm, and `lower_pending_exception_dispatch` already
            // closes it on a propagating exception — this only has to build
            // the frame and run it once more on the fall-through path,
            // exactly as `lower_try` does for a real `finally`.
            let resource_finally = expr.with_binding.as_ref().and_then(|binding| {
                Self::pattern_binds(&arm.pattern, &binding.name)
                    .then(|| Self::resource_close_block(&binding.name, arm.span))
            });
            if let Some(finally) = &resource_finally {
                self.try_stack.push(TryFrame {
                    catches: Vec::new(),
                    finally: Some(finally.clone()),
                });
            }

            let value = match &arm.body {
                ast::ArmBody::Expr(e) => self.lower_expr(e),
                ast::ArmBody::Block(b) if result_type == IrType::Void => {
                    self.lower_block(b);
                    self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span)
                }
                ast::ArmBody::Block(b) => self.lower_block_value(b),
            };

            if resource_finally.is_some() {
                self.try_stack.pop();
                if self.block_mut(self.current).terminator.is_none() {
                    self.lower_finally_block(&resource_finally);
                }
            }
            self.scopes.pop();

            if let Some(slot) = result {
                self.emit_effect(InstKind::Store(slot, value), span);
            }
            self.terminate(Terminator::Jump(continue_block));
            reachable = true;

            self.current = resume;
            if arm.pattern.is_irrefutable() {
                break;
            }
        }

        // The checker proved the arms exhaustive, so falling off the chain
        // cannot happen at run time — but the block still needs a terminator
        // for the IR to be well formed.
        if !reachable || !self.is_terminated(self.current) {
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = continue_block;

        match result {
            Some(slot) => self.emit(InstKind::Load(slot), result_type, span),
            // Nothing reads the result of a statement `match`; a placeholder
            // keeps the signature of `lower_expr` total.
            None => self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span),
        }
    }

    /// Loads the scrutinee, unwrapping it when `narrowed_type` says a
    /// sibling `null` arm already ruled out absence — the same
    /// `InstKind::Unwrap` `lower_coalesce`/`lower_safe_field` use once they
    /// too have proven a nullable value present.
    fn load_scrutinee_narrowed(
        &mut self,
        scrutinee: SlotId,
        scrutinee_type: IrType,
        narrowed_type: IrType,
        span: Span,
    ) -> Operand {
        let value = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
        if narrowed_type == scrutinee_type {
            value
        } else {
            self.emit(InstKind::Unwrap(value), narrowed_type, span)
        }
    }

    /// The comparison one pattern stands for.
    fn lower_pattern_test(
        &mut self,
        pattern: &ast::Pattern,
        scrutinee: SlotId,
        scrutinee_type: IrType,
        narrowed_type: IrType,
        span: Span,
    ) -> Operand {
        let expected = match pattern {
            ast::Pattern::Int(lit) => self.emit(
                InstKind::ConstInt(lit.value as i32),
                IrType::Int(IntWidth::I32),
                span,
            ),
            ast::Pattern::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Pattern::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }
            ast::Pattern::Variant(v) => {
                let value = self.discriminant(&v.enum_name, &v.variant.name);
                self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I32), span)
            }
            // `null` tests absence rather than a value, so it is the one
            // pattern that does not compare against anything — and the one
            // test that must read the raw, still-nullable slot.
            ast::Pattern::Null(_) => {
                let value = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                return self.emit(InstKind::IsNull(value), IrType::Boolean, span);
            }
            ast::Pattern::Wildcard(_) | ast::Pattern::Binding(_) => {
                unreachable!("an irrefutable pattern is not tested this way")
            }
        };

        // Every other pattern tests a value that is known present (either
        // the scrutinee was never nullable, or a sibling `null` arm already
        // took that case), so it compares against the unwrapped value.
        let left = self.load_scrutinee_narrowed(scrutinee, scrutinee_type, narrowed_type, span);
        // An algebraic enum's own value is discriminant plus payload, not
        // the discriminant alone (roadmap task 11.3) — a variant pattern
        // still tests only the discriminant, so it is read out first.
        let left = if let IrType::Enum(_) = narrowed_type {
            self.emit(
                InstKind::Discriminant(left),
                IrType::Int(IntWidth::I32),
                span,
            )
        } else {
            left
        };
        self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left,
                right: expected,
            },
            IrType::Boolean,
            span,
        )
    }

    /// Registers a pattern's own bound names against the current scope,
    /// without emitting anything — see the call site in [`Self::lower_match`].
    fn declare_pattern_types(&mut self, pattern: &ast::Pattern, scrutinee_type: IrType) {
        match pattern {
            ast::Pattern::Binding(ident) => {
                self.declare_slot(&ident.name, scrutinee_type, ident.span);
            }
            ast::Pattern::Variant(v) if !v.bindings.is_empty() => {
                let template_id = self.enum_id_of(&v.enum_name);
                let Some(variant) = self
                    .checked
                    .enums
                    .get(template_id as usize)
                    .and_then(|e| e.discriminant(&v.variant.name))
                else {
                    return;
                };
                let IrType::Enum(module_id) = scrutinee_type else {
                    return;
                };
                let indices =
                    self.module.enums[module_id as usize].variants[variant as usize].clone();
                for (sub_pattern, index) in v.bindings.iter().zip(indices) {
                    if let ast::Pattern::Binding(ident) = sub_pattern {
                        let field_ty =
                            self.module.enums[module_id as usize].fields[index as usize].ty;
                        self.declare_slot(&ident.name, field_ty, ident.span);
                    }
                }
            }
            _ => {}
        }
    }

    /// The type the arms of a `match` produce.
    fn arm_value_type(&self, expr: &ast::MatchExpr) -> IrType {
        // The first arm decides: the checker already proved every arm agrees,
        // so looking further would only confirm what it verified.
        let Some(first) = expr.arms.first() else {
            return IrType::Void;
        };

        match &first.body {
            ast::ArmBody::Expr(e) => self.type_of(e, e.span()),
            ast::ArmBody::Block(b) if matches!(b.statements.last(), Some(ast::Stmt::Expr(_))) => {
                self.block_value_type(b)
            }
            // A block that does not end in an expression produces nothing, and
            // that makes the whole `match` a statement.
            ast::ArmBody::Block(_) => IrType::Void,
        }
    }

    /// Lowers an `if` used as a value.
    ///
    /// The branches write into one slot and the continuation reads it. With
    /// locals as slots and no SSA of our own (ADR-007), that is the whole of
    /// it: LLVM promotes the slot to a register and inserts the phi node.
    fn lower_if_expr(&mut self, stmt: &ast::IfStmt, span: Span) -> Operand {
        // Same idea as `lower_ternary`: `Never` (a branch ending in
        // `fatalError(...)`) contributes nothing at the join (roadmap Phase
        // 4a).
        let then_ty = self.block_value_type(&stmt.then_branch);
        let else_ty = match &stmt.else_branch {
            Some(ast::ElseBranch::Block(b)) => self.block_value_type(b),
            Some(ast::ElseBranch::If(nested)) => self.block_value_type(&nested.then_branch),
            None => unreachable!("a verified `if` expression always has an `else`"),
        };
        let ty = if then_ty == IrType::Never {
            else_ty
        } else {
            then_ty
        };
        let result = self.declare_slot("<if>", ty, span);

        let condition = self.lower_expr(&stmt.condition);
        let then_block = self.new_block();
        let else_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        if then_ty == IrType::Never {
            let _ = self.lower_block_value(&stmt.then_branch);
            self.terminate(Terminator::Unreachable);
        } else {
            let value = self.lower_block_value(&stmt.then_branch);
            self.emit_effect(InstKind::Store(result, value), span);
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = else_block;
        if else_ty == IrType::Never {
            let _ = match &stmt.else_branch {
                Some(ast::ElseBranch::Block(b)) => self.lower_block_value(b),
                Some(ast::ElseBranch::If(nested)) => self.lower_if_expr(nested, span),
                None => unreachable!("a verified `if` expression always has an `else`"),
            };
            self.terminate(Terminator::Unreachable);
        } else {
            let value = match &stmt.else_branch {
                Some(ast::ElseBranch::Block(b)) => self.lower_block_value(b),
                Some(ast::ElseBranch::If(nested)) => self.lower_if_expr(nested, span),
                None => unreachable!("a verified `if` expression always has an `else`"),
            };
            self.emit_effect(InstKind::Store(result, value), span);
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = continue_block;
        self.emit(InstKind::Load(result), ty, span)
    }

    /// Lowers `super(...)` and `super.method(...)`.
    ///
    /// Both are direct calls: `super` names a body statically, which is the
    /// whole point of writing it instead of letting dispatch decide.
    fn lower_super_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let base = self.enclosing_base()?;

        // `super.method(...)`
        if let ast::Expr::Field(field) = &*call.callee
            && matches!(&*field.object, ast::Expr::Super(_))
        {
            let class = &self.checked.classes[base as usize];
            let method = class.method(&field.name.name)?;
            let name = body_symbol(self.checked, method);
            let returns = self.ir_type(method.returns);
            let params: Vec<IrType> = method.params.iter().map(|p| self.ir_type(p.ty)).collect();

            let this = self.lookup_slot("this");
            let receiver = self.emit(InstKind::Load(this), self.slot_type(this), span);
            let mut args = vec![receiver];
            for (arg, ty) in call.args.iter().zip(params) {
                args.push(self.lower_expr_as(&arg.value, ty));
            }
            return Some(self.emit(InstKind::Call { callee: name, args }, returns, span));
        }

        if !matches!(&*call.callee, ast::Expr::Super(_)) {
            return None;
        }

        // `super(...)`: the base's constructor, on this same instance.
        let index = self.constructor_index(base, call.args.len());
        let params: Vec<IrType> = self.checked.classes[base as usize].constructors[index]
            .iter()
            .map(|p| self.ir_type(p.ty))
            .collect();

        let this = self.lookup_slot("this");
        let receiver = self.emit(InstKind::Load(this), self.slot_type(this), span);
        let mut args = vec![receiver];
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        let name = self.checked.classes[base as usize].name.clone();
        self.emit_effect(
            InstKind::Call {
                callee: constructor_symbol(&name, index),
                args,
            },
            span,
        );

        // `super(...)` produces nothing: it is run for its effect on `this`.
        Some(self.emit(InstKind::ConstBool(false), IrType::Boolean, span))
    }

    /// The base of the class whose body is being lowered.
    fn enclosing_base(&self) -> Option<u32> {
        let slot = self.scopes.iter().rev().find_map(|s| s.get("this"))?;
        let IrType::Object(id) = self.slot_type(*slot) else {
            return None;
        };
        self.checked.classes.get(id as usize)?.base
    }

    /// The reserved method an operator resolves to, if its left operand is a
    /// class that supplies one.
    fn operator_method_of(&self, expr: &ast::BinaryExpr) -> Option<(u32, String)> {
        let IrType::Object(id) = self.type_of(&expr.left, expr.left.span()) else {
            return None;
        };
        let name = operator_method(expr.op)?;
        self.checked.classes[id as usize]
            .method(name)
            .map(|_| (id, name.to_string()))
    }

    /// The contract method a call invokes, if the receiver is a contract.
    fn contract_method_of(&self, call: &ast::CallExpr) -> Option<&zirk_sema::ContractMethod> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        // Otherwise this asks for the type of a value that is not one: the
        // object names an enum, not a variable (task 11.3's variant
        // construction is the same shape a method call is, up to here).
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        let IrType::Contract(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };
        self.checked.contracts[id as usize].method(&field.name.name)
    }

    /// The method a call invokes, if it is a method call.
    fn method_of(&self, call: &ast::CallExpr) -> Option<&zirk_sema::MethodInfo> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        // A record or value class's own method is reached the same way an
        // ordinary class's is: `IrType::Value` only changes how the
        // receiver is represented (inline, no header), not that it resolves
        // to a `MethodInfo` the same table an object's does.
        let id = match self.type_of(&field.object, field.object.span()) {
            IrType::Object(id) | IrType::Value(id) => id,
            _ => return None,
        };
        self.checked.classes[id as usize].method(&field.name.name)
    }

    /// The module object id of the compiler-known `Throwable` (roadmap
    /// Phase 4b) — the static type every pending exception is taken as,
    /// since the runtime slot holding it is type-erased.
    fn throwable_object_id(&self) -> u32 {
        let native = self
            .checked
            .native_exceptions
            .expect("a program that can throw registered the exception hierarchy");
        let IrType::Object(id) = self.ir_type(Type::of(Base::Class(native.throwable))) else {
            unreachable!("`Throwable` is an ordinary class")
        };
        id
    }

    /// Whether `id` is one of the four native failure classes
    /// (`fase-4d-runtimeerror`, D9) — `DivisionByZeroError`,
    /// `InvalidShiftError`, `InvalidRepeatError` or `FloatNanError`. Used by
    /// [`Self::lower_construction`]'s own special case for a program's
    /// explicit `Type("reason")` of one of these, the same way
    /// [`Self::lower_construction`] already special-cases `StackTrace`.
    fn is_native_failure_class(&self, id: u32) -> bool {
        self.checked.native_exceptions.is_some_and(|n| {
            id == n.division_by_zero
                || id == n.invalid_shift
                || id == n.invalid_repeat
                || id == n.float_nan
        })
    }

    /// Builds one of the four native failure objects by hand (D8/D10): an
    /// `Alloc` plus a `StoreField` of a fixed message into `reason` (field
    /// `0`, the class's only one) — the same shape a program's own
    /// `Type("reason")` takes through [`Self::lower_construction`]'s special
    /// case, except this one never goes through an `ast::CallExpr`: a
    /// compiler-detected failure has no source expression to lower an
    /// argument from.
    fn build_native_failure(&mut self, class_id: u32, message: &str, span: Span) -> Operand {
        let object = self.emit(InstKind::Alloc(class_id), IrType::Object(class_id), span);
        let text = self.module.intern_string(message);
        let value = self.emit(InstKind::ConstString(text), IrType::String, span);
        self.emit_effect(
            InstKind::StoreField {
                object,
                index: 0,
                value,
            },
            span,
        );
        object
    }

    /// Builds and throws one of the four native failures, then dispatches to
    /// whatever active `catch` covers it — or re-propagates, having run
    /// every `finally` on the way (D10). The same two steps
    /// [`Self::lower_throw`] takes for an explicit `throw expr;`, just with
    /// the exception built by [`Self::build_native_failure`] instead of
    /// lowered from a source expression.
    fn throw_native_failure(&mut self, class_id: u32, message: &str, span: Span) {
        let exception = self.build_native_failure(class_id, message, span);
        self.emit_effect(InstKind::Throw(exception), span);
        self.lower_pending_exception_dispatch(span);
    }

    /// Division-by-zero guard (D10): before `Div`/`Rem` over integers runs,
    /// compares the divisor to zero on a branch of its own, throwing
    /// `DivisionByZeroError` on the failing side — `checked_division`'s own
    /// `trap_if` (`zirk-codegen-llvm/src/emit.rs`), moved up from codegen to
    /// here so a `try`/`catch` around the expression can intercept it. The
    /// one overflow case a signed division can still hit (`Int32::MIN /
    /// -1`) is untouched: `OverflowError` is explicitly out of this pass's
    /// scope (see `proposal.md`), so `checked_division` still guards it with
    /// `trap_if` exactly as before.
    /// Spills an already-lowered value into a fresh slot and reads it back
    /// immediately — a no-op in the block it runs in, but what makes the
    /// *value* usable again once [`Self::current`] has moved to a different
    /// block: an IR value never crosses a block boundary on its own (ADR-007
    /// D7 of `fase-1`), so anything a guard's own branch needs on its
    /// "nothing failed" side has to travel through a slot the same way
    /// [`Self::lower_and_hold`]'s `Held::Spilled` already does for a
    /// held operand, just without that helper's own conditional ("only if a
    /// later expression opens blocks") — a guard's branch is unconditional
    /// once it exists.
    fn spill(&mut self, operand: Operand, ty: IrType, span: Span) -> SlotId {
        let slot = self.declare_slot("<guard>", ty, span);
        self.emit_effect(InstKind::Store(slot, operand), span);
        slot
    }

    /// An integer constant at an arbitrary width (a guard's own `0` or bit
    /// width, which may need to compare against an operand narrower or
    /// wider than `Int32`) — the same `ConstInt` declares `Int32`, widen
    /// with `IntCast`" shape [`Self::const_i64`] already uses, generalized
    /// to any destination width: the verifier requires every `ConstInt`
    /// itself to declare `Int32` (a literal always types `Int32`), so a
    /// different destination goes through an explicit `IntCast` from one.
    fn const_int_at(&mut self, value: i32, ty: IrType, span: Span) -> Operand {
        let narrow = self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I32), span);
        if ty == IrType::Int(IntWidth::I32) {
            narrow
        } else {
            self.emit(InstKind::IntCast(narrow), ty, span)
        }
    }

    /// Division/remainder guard (D10): before `Div`/`Rem` over integers
    /// runs, compares the divisor to zero on a branch of its own, throwing
    /// `DivisionByZeroError` on the failing side and performing the real
    /// division/remainder on the other — `checked_division`'s own `trap_if`
    /// (`zirk-codegen-llvm/src/emit.rs`), moved up from codegen to here so a
    /// `try`/`catch` around the expression can intercept it. Both operands
    /// are spilled ahead of the branch ([`Self::spill`]) since the actual
    /// operation is emitted on the far side of it, in a different block. The
    /// one overflow case a signed division can still hit (`Int32::MIN /
    /// -1`) is untouched: `OverflowError` is explicitly out of this pass's
    /// scope (see `proposal.md`), so `checked_division` still guards it with
    /// `trap_if` exactly as before.
    fn checked_int_division(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        operand_type: IrType,
        span: Span,
    ) -> Operand {
        let left_slot = self.spill(left, operand_type, span);
        let right_slot = self.spill(right, operand_type, span);

        let right_reloaded = self.emit(InstKind::Load(right_slot), operand_type, span);
        let zero = self.const_int_at(0, operand_type, span);
        let is_zero = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: right_reloaded,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_zero,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with integer division registered the exception hierarchy");
        self.throw_native_failure(native.division_by_zero, "division by zero", span);

        self.current = cont;
        let left = self.emit(InstKind::Load(left_slot), operand_type, span);
        let right = self.emit(InstKind::Load(right_slot), operand_type, span);
        let result_type = op.result_type(operand_type);
        self.emit(InstKind::Binary { op, left, right }, result_type, span)
    }

    /// Invalid-shift guard (D10): before `Shl`/`Shr` runs, the same range
    /// check `checked_shift` used to make at the LLVM level (negative, or at
    /// least the shifted operand's own bit width), now as an IR `Branch`
    /// that throws `InvalidShiftError` on the failing side and performs the
    /// real shift on the other. Both operands are spilled ahead of the
    /// branch ([`Self::spill`]), the same reason
    /// [`Self::checked_int_division`] does.
    fn checked_shift(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        operand_type: IrType,
        span: Span,
    ) -> Operand {
        let IrType::Int(width) = operand_type else {
            unreachable!("the checker only allows Shl/Shr over an integer operand")
        };
        let IrType::Int(amount_width) = self.type_of_operand(right) else {
            unreachable!("a shift amount is always an integer")
        };
        let amount_ty = IrType::Int(amount_width);

        let left_slot = self.spill(left, operand_type, span);
        let right_slot = self.spill(right, amount_ty, span);

        let amount = self.emit(InstKind::Load(right_slot), amount_ty, span);
        let bits = self.const_int_at(width.bits() as i32, amount_ty, span);

        // An unsigned amount is never negative by construction — the same
        // narrowing `checked_shift`'s own `right_signed` branch made at the
        // codegen level.
        let invalid = if amount_width.signed() {
            let zero = self.const_int_at(0, amount_ty, span);
            let is_negative = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Lt,
                    left: amount,
                    right: zero,
                },
                IrType::Boolean,
                span,
            );
            let is_too_wide = self.emit(
                InstKind::Binary {
                    op: BinaryOp::GtEq,
                    left: amount,
                    right: bits,
                },
                IrType::Boolean,
                span,
            );
            self.emit(
                InstKind::Binary {
                    op: BinaryOp::Or,
                    left: is_negative,
                    right: is_too_wide,
                },
                IrType::Boolean,
                span,
            )
        } else {
            self.emit(
                InstKind::Binary {
                    op: BinaryOp::GtEq,
                    left: amount,
                    right: bits,
                },
                IrType::Boolean,
                span,
            )
        };

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: invalid,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with a shift registered the exception hierarchy");
        self.throw_native_failure(
            native.invalid_shift,
            "a shift amount must be non-negative and less than the operand's bit width",
            span,
        );

        self.current = cont;
        let left = self.emit(InstKind::Load(left_slot), operand_type, span);
        let right = self.emit(InstKind::Load(right_slot), amount_ty, span);
        self.emit(InstKind::Binary { op, left, right }, operand_type, span)
    }

    /// Invalid-repeat guard (D10): before `String * Int32` runs, checks the
    /// count is non-negative — the one check that used to live inside the
    /// runtime itself (`zirk_rt_invalid_repeat`'s own negative-count branch
    /// in `zirk-runtime/src/string.rs`), now moved ahead of the call so a
    /// `try`/`catch` can intercept it, throwing `InvalidRepeatError` on the
    /// failing side and performing the real repetition on the other. Both
    /// operands are spilled ahead of the branch ([`Self::spill`]), the same
    /// reason [`Self::checked_int_division`] does. The runtime keeps its own
    /// other check (an overflowing byte size): that guards `OverflowError`'s
    /// own territory, out of this pass's scope.
    fn checked_repeat(&mut self, string: Operand, count: Operand, span: Span) -> Operand {
        let string_slot = self.spill(string, IrType::String, span);
        let count_slot = self.spill(count, IrType::Int(IntWidth::I32), span);

        let count_reloaded =
            self.emit(InstKind::Load(count_slot), IrType::Int(IntWidth::I32), span);
        let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
        let is_negative = self.emit(
            InstKind::Binary {
                op: BinaryOp::Lt,
                left: count_reloaded,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_negative,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with a string repetition registered the exception hierarchy");
        self.throw_native_failure(
            native.invalid_repeat,
            "a string can only be repeated a non-negative number of times",
            span,
        );

        self.current = cont;
        let string = self.emit(InstKind::Load(string_slot), IrType::String, span);
        let count = self.emit(InstKind::Load(count_slot), IrType::Int(IntWidth::I32), span);
        self.emit(InstKind::Repeat { string, count }, IrType::String, span)
    }

    /// `NaN` guard (D10): after a `Float` arithmetic operation runs (unlike
    /// the other three, this checks the *result*, not an operand — a `Float`
    /// division's own zero divisor is a valid infinity, never a check target
    /// on its own), compares it against itself — the IEEE 754 property that
    /// only `NaN` fails — and throws `FloatNanError` on a hit. The result is
    /// spilled before the branch ([`Self::spill`]) and reloaded on the
    /// non-failing side, since it is the value this guard itself returns.
    fn guard_nan(&mut self, result: Operand, ty: IrType, span: Span) -> Operand {
        let slot = self.spill(result, ty, span);
        let reloaded = self.emit(InstKind::Load(slot), ty, span);
        // `x == x` is `false` exactly when `x` is `NaN` — the IEEE 754
        // property `check_not_nan` used to test directly through LLVM's own
        // *unordered* `UNO` predicate. `BinaryOp::NotEq` is not its
        // negation here: `ZIRK_LANGUAGE_SPEC.md`'s own `!=` over `Float`
        // lowers to LLVM's *ordered* `ONE` (`emit_float_binary`'s own
        // table), which — like `==`'s `OEQ` — is `false` whenever either
        // operand is `NaN`, so `x != x` is `false` for a `NaN` `x` too, the
        // opposite of what a guard needs. `!(x == x)` sidesteps the
        // difference entirely: `Eq`'s own `OEQ` already answers "is `x`
        // exactly itself", and negating that is exactly "is `x` `NaN`".
        let is_equal = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: reloaded,
                right: reloaded,
            },
            IrType::Boolean,
            span,
        );
        let is_nan = self.emit(
            InstKind::Unary {
                op: UnaryOp::Not,
                operand: is_equal,
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_nan,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with Float arithmetic registered the exception hierarchy");
        self.throw_native_failure(
            native.float_nan,
            "Float operation produced an indeterminate result (NaN)",
            span,
        );

        self.current = cont;
        self.emit(InstKind::Load(slot), ty, span)
    }

    /// Emits a binary operation, guarded by whichever of the four native
    /// checks D10 moved into `zirk-ir` applies to it: division/remainder and
    /// shift are guarded before the operation runs (the operand decides the
    /// failure, so [`Self::checked_int_division`]/[`Self::checked_shift`]
    /// perform the operation themselves, on the far side of their own
    /// branch), `Float` arithmetic is guarded after (the result does, so
    /// [`Self::guard_nan`] wraps an already-emitted one). Every other
    /// operator (comparison, bitwise, logical, `is`) has no native failure
    /// to guard and reaches straight through to [`Self::emit`].
    fn emit_checked_binary(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        operand_type: IrType,
        span: Span,
    ) -> Operand {
        if let (BinaryOp::Div | BinaryOp::Rem, IrType::Int(_)) = (op, operand_type) {
            return self.checked_int_division(op, left, right, operand_type, span);
        }
        if let (BinaryOp::Shl | BinaryOp::Shr, IrType::Int(_)) = (op, operand_type) {
            return self.checked_shift(op, left, right, operand_type, span);
        }

        let result_type = op.result_type(operand_type);
        let result = self.emit(InstKind::Binary { op, left, right }, result_type, span);

        if matches!(operand_type, IrType::Float(_))
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            return self.guard_nan(result, result_type, span);
        }

        result
    }

    /// `throw expr;` / `throw;` (roadmap Phase 4b, design D1): records the
    /// value as the pending exception and returns immediately from the
    /// current function with a placeholder — the callee side of the same
    /// mechanism [`Self::lower_throws_check`] is the caller side of.
    fn lower_throw(&mut self, stmt: &ast::ThrowStmt) {
        let exception = match &stmt.value {
            Some(expr) => self.lower_expr(expr),
            // A bare rethrow reads the innermost `catch`'s own binding — the
            // checker already confirmed one is active
            // (`Checker::check_throw`'s own `catch_type` gate).
            None => {
                let (slot, ty) = self
                    .current_catch
                    .expect("the checker confirmed a bare `throw;` is inside a `catch`");
                self.emit(InstKind::Load(slot), ty, stmt.span)
            }
        };
        self.emit_effect(InstKind::Throw(exception), stmt.span);
        // Dispatches to a local `catch` that covers it, or — finding
        // none — re-propagates by returning early from the current
        // function; the same resolution a throwing call's own post-call
        // check reaches (`Self::lower_throws_check`), since a `throw`
        // statement is just that check with the "was something pending"
        // test already known to be true.
        self.lower_pending_exception_dispatch(stmt.span);
    }

    /// After *every* call (roadmap Phase 4b, design D1; unconditional since
    /// `fase-4d-runtimeerror`'s own D11 — see below): tests
    /// `zirk_rt_has_pending_exception()`, and on a hit dispatches to the
    /// nearest enclosing `catch` that covers the actual (runtime-tested)
    /// exception type, or — having run every `finally` it passes through on
    /// the way — re-propagates by returning early from the current
    /// function. `result`/`ty` are the call's own already-lowered value and
    /// type, held across the branch through a slot (values do not cross
    /// blocks, ADR-007) and reloaded once the check confirms nothing was
    /// pending.
    ///
    /// Runs after every call, not only one whose static target declares a
    /// non-empty `throws` (D11, `fase-4d-runtimeerror`'s design doc): once a
    /// native check inside the callee (division by zero, an invalid shift, a
    /// negative repeat count, `Float` producing `NaN`) can itself throw a
    /// `RuntimeError` that no `throws` clause ever names — by design,
    /// `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` §3 — a callee's
    /// declared `throws` alone is no longer sound evidence that nothing
    /// pending needs checking here. The cost (one
    /// `zirk_rt_has_pending_exception` plus a branch per call site) is the
    /// same trade-off `fase-4b-excepciones`'s own D6 already flagged and
    /// accepted if this ever happened.
    fn lower_throws_check(&mut self, result: Operand, ty: IrType, span: Span) -> Operand {
        let holder = (ty != IrType::Void).then(|| {
            let slot = self.declare_slot("<call_result>", ty, span);
            self.emit_effect(InstKind::Store(slot, result), span);
            slot
        });

        let has_pending = self.emit(InstKind::HasPendingException, IrType::Boolean, span);
        let handle_block = self.new_block();
        let continue_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: has_pending,
            then_block: handle_block,
            else_block: continue_block,
        });

        self.current = handle_block;
        self.lower_pending_exception_dispatch(span);

        self.current = continue_block;
        match holder {
            Some(slot) => self.emit(InstKind::Load(slot), ty, span),
            // `result` (a `Void` value, carrying no information of its own)
            // was defined in the block before this function's own branch —
            // reusing it here directly, in `continue_block`, would cross
            // that boundary (ADR-007) the moment a caller actually threads
            // it onward instead of discarding it, as
            // `Self::lower_variant_construction` does for `Result.Ok(a_void_call())`
            // (found via `FakeFile::close`'s own `return Result.Ok(this.nothing())`).
            // A fresh placeholder in `continue_block` is exactly as
            // meaningless as `result` was — nothing ever reads a `Void`
            // value — so there is nothing to actually carry across, only a
            // valid one to produce here.
            None => self.emit(InstKind::Undefined, IrType::Void, span),
        }
    }

    /// The handling half of [`Self::lower_throws_check`]: takes the pending
    /// exception, tests it against each active `catch` from innermost to
    /// outermost, jumps into the first match, and otherwise re-propagates —
    /// running each frame's own `finally` on the way past it, the same as a
    /// normal exit from that `try` would (design D2).
    fn lower_pending_exception_dispatch(&mut self, span: Span) {
        let throwable_id = self.throwable_object_id();
        let throwable_ty = IrType::Object(throwable_id);
        let taken = self.emit(InstKind::TakePendingException, throwable_ty, span);
        let holder = self.declare_slot("<exception>", throwable_ty, span);
        self.emit_effect(InstKind::Store(holder, taken), span);

        // A snapshot, walked from innermost to outermost — and, crucially,
        // what `self.try_stack` itself is temporarily narrowed to while each
        // frame's own `finally` is lowered below. Without the narrowing, a
        // call inside that `finally` (D11, `fase-4d-runtimeerror`: *every*
        // call now runs this same dispatch after it, not only a `throws`-
        // declared one — a `Resource<E>`'s own `close()` call, synthesized
        // by `Self::resource_close_block`, is exactly such a call and
        // declares no `throws` of its own) would still see this same frame
        // — not yet actually popped from the live `self.try_stack` at this
        // point — as active, re-enter it, and lower its `finally` again,
        // forever: found via `resources_match_with_closes_on_thrown_exception`
        // recursing until the generated program's own stack overflowed.
        // Real exception semantics agree with the narrowing regardless: a
        // `finally` runs in the context of whatever is *outer* to its own
        // `try`, never wrapped by that `try` itself.
        let frames = self.try_stack.clone();
        let full_try_stack = std::mem::take(&mut self.try_stack);
        for (index, frame) in frames.iter().enumerate().rev() {
            for catch in &frame.catches {
                let exception = self.emit(InstKind::Load(holder), throwable_ty, span);
                let matches = self.emit(
                    InstKind::IsInstance {
                        object: exception,
                        target_class: catch.class,
                    },
                    IrType::Boolean,
                    span,
                );
                let matched_block = self.new_block();
                let next_test_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: matches,
                    then_block: matched_block,
                    else_block: next_test_block,
                });

                self.current = matched_block;
                let exception = self.emit(InstKind::Load(holder), throwable_ty, span);
                let narrowed = self.emit(
                    InstKind::Retype(exception),
                    IrType::Object(catch.class),
                    span,
                );
                self.emit_effect(InstKind::Store(catch.slot, narrowed), span);
                self.terminate(Terminator::Jump(catch.handler));

                self.current = next_test_block;
            }
            // None of this frame's own catches covers it: its `finally`
            // still runs before the search continues further out, exactly
            // as it would on any other exit from this `try` — with only the
            // frames outer to this one active for the duration (see above).
            self.try_stack = full_try_stack[..index].to_vec();
            self.lower_finally_block(&frame.finally);
        }
        self.try_stack = full_try_stack;

        // Escaped every active `try`: put the exception back as pending and
        // re-propagate from the current function, the same early-return an
        // uncaught `throw` itself takes.
        let exception = self.emit(InstKind::Load(holder), throwable_ty, span);
        self.emit_effect(InstKind::Throw(exception), span);
        let placeholder = self
            .default_value(self.return_type, span)
            .unwrap_or_else(|| self.emit(InstKind::Undefined, self.return_type, span));
        self.terminate(Terminator::Return(
            (self.return_type != IrType::Void).then_some(placeholder),
        ));
    }

    /// Lowers a `finally` block's statements in place, if there is one — no
    /// termination of its own (roadmap Phase 4b): the caller decides what
    /// follows.
    ///
    /// Only reached from the "falls through" and "propagates past this
    /// `try`" exits ([`Self::lower_try`]/[`Self::lower_pending_exception_dispatch`]):
    /// a `return`/`break`/`continue` written *inside* the `try` body itself
    /// still jumps out through the ordinary `lower_return`/`lower_break`/
    /// `lower_continue` paths, which do not consult `try_stack` — narrower
    /// than design D2's full rule (every exit path runs `finally`), and
    /// documented as such (`fase-4b-excepciones/design.md`'s own risk list).
    /// Whether `pattern` — bare or inside a variant's `(...)` — binds `name`
    /// (roadmap Phase 4c), the same shape [`Self::declare_pattern_types`]
    /// already walks for an ordinary arm.
    fn pattern_binds(pattern: &ast::Pattern, name: &str) -> bool {
        match pattern {
            ast::Pattern::Binding(ident) => ident.name == name,
            ast::Pattern::Variant(v) => v.bindings.iter().any(|p| Self::pattern_binds(p, name)),
            _ => false,
        }
    }

    /// Synthesizes `binding.close();` as a one-statement block (roadmap
    /// Phase 4c) — a `Resource<E>`'s `close(): Result<Void,E>` called for
    /// its own side effect and never for its returned `Result`, the same
    /// simplification this pass's own scope narrowing accepts: a close
    /// failure is not merged into `ResourceFailure<BodyError,CloseError>`
    /// (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 4), only
    /// discarded. Lowered through the ordinary method-call path
    /// (`Self::lower_expr_for_effect`'s `method_of` arm), which resolves
    /// `binding` by its own slot in `self.scopes` — no checker-side table is
    /// consulted, since none was built for this synthetic node.
    fn resource_close_block(name: &str, span: Span) -> ast::Block {
        let object = ast::Expr::Path(ast::Ident::new(name.to_string(), span));
        let callee = ast::Expr::Field(ast::FieldExpr {
            object: Box::new(object),
            name: ast::Ident::new("close".to_string(), span),
            safe: false,
            span,
        });
        let call = ast::Expr::Call(ast::CallExpr {
            callee: Box::new(callee),
            args: Vec::new(),
            span,
        });
        ast::Block {
            statements: vec![ast::Stmt::Expr(ast::ExprStmt { expr: call, span })],
            span,
        }
    }

    fn lower_finally_block(&mut self, finally: &Option<ast::Block>) {
        if let Some(block) = finally {
            self.lower_block(block);
        }
    }

    /// Lowers `try { } catch Type(name) { } ... finally { }` (roadmap Phase
    /// 4b). Each `catch`'s handler block and binding slot are built before
    /// the body is lowered, so a throwing call anywhere inside it — however
    /// deeply nested — can jump straight into one
    /// (`Self::lower_pending_exception_dispatch`).
    fn lower_try(&mut self, stmt: &ast::TryStmt) {
        let continue_block = self.new_block();

        let catches: Vec<CatchFrame> = stmt
            .catches
            .iter()
            .map(|catch| {
                let class = self
                    .class_id(&catch.ty.name)
                    .expect("the checker resolved this catch type to a class");
                let handler = self.new_block();
                let slot = self.declare_slot(
                    &catch.binding.name,
                    IrType::Object(class),
                    catch.binding.span,
                );
                CatchFrame {
                    class,
                    handler,
                    slot,
                    name: catch.binding.name.clone(),
                }
            })
            .collect();

        self.try_stack.push(TryFrame {
            catches: catches.clone(),
            finally: stmt.finally.clone(),
        });
        self.lower_block(&stmt.body);
        self.try_stack.pop();
        if self.block_mut(self.current).terminator.is_none() {
            self.lower_finally_block(&stmt.finally);
            self.terminate(Terminator::Jump(continue_block));
        }

        for frame in &catches {
            self.current = frame.handler;
            self.scopes.push(HashMap::new());
            self.scopes
                .last_mut()
                .expect("just pushed")
                .insert(frame.name.clone(), frame.slot);
            let clause = stmt
                .catches
                .iter()
                .find(|c| c.binding.name == frame.name)
                .expect("built from the same list");
            let previous_catch = self
                .current_catch
                .replace((frame.slot, IrType::Object(frame.class)));
            self.lower_block(&clause.body);
            self.current_catch = previous_catch;
            self.scopes.pop();
            if self.block_mut(self.current).terminator.is_none() {
                self.lower_finally_block(&stmt.finally);
                self.terminate(Terminator::Jump(continue_block));
            }
        }

        self.current = continue_block;
    }

    /// Lowers `value.method(...)` where the receiver is reached by contract.
    fn lower_contract_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        let IrType::Contract(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };

        let contract = &self.checked.contracts[id as usize];
        let method = contract.method(&field.name.name)?;
        let index = method.index as u32;
        let returns = self.ir_type(method.returns);
        let params: Vec<IrType> = method.params.iter().map(|p| self.ir_type(p.ty)).collect();

        let receiver = self.lower_expr(&field.object);
        let mut args = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        Some(self.emit(
            InstKind::CallContract {
                object: receiver,
                contract: id,
                index,
                args,
            },
            returns,
            span,
        ))
    }

    /// Lowers `object.method(...)` into a direct call with the receiver first.
    ///
    /// Direct because no method is redefinable yet: without inheritance every
    /// call has exactly one target, and paying an indirection for a generality
    /// the program cannot use would be paying for nothing (D3).
    /// Lowers `myInt.to_string()` — the explicit spelling of the same
    /// conversion `println`/interpolation reach implicitly (roadmap Phase
    /// 3b, task 8's own follow-up) — for a native scalar only. A class,
    /// record or value class's own `to_string()` method already goes
    /// through the ordinary method-call path (`lower_method_call`, tried
    /// right after this one), which is why this returns `None` for
    /// `Object`/`Value`/`Contract` receivers rather than handling them too.
    fn lower_native_to_string_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        if !self.is_native_to_string_call(call) {
            return None;
        }
        let ast::Expr::Field(field) = &*call.callee else {
            unreachable!("checked by is_native_to_string_call")
        };
        let receiver_ty = self.type_of(&field.object, field.object.span());
        let receiver = self.lower_expr(&field.object);
        Some(self.lower_to_string(receiver, receiver_ty, span))
    }

    fn lower_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        // A record or value class's own method call is the same shape as an
        // ordinary object's — see `Self::method_of`. `extends` is rejected
        // for either kind, so no subclass can ever redefine one of its
        // methods: `virtual_index` below always comes out `None`, which is
        // what keeps this call direct rather than through a table a value
        // has no header to carry.
        let id = match self.type_of(&field.object, field.object.span()) {
            IrType::Object(id) | IrType::Value(id) => id,
            _ => return None,
        };

        let class = &self.checked.classes[id as usize];
        let method = class.method(&field.name.name)?;
        // The body lives where it was declared, which is not always the class
        // being called through: an inherited method keeps its owner, and an
        // adopted trait default belongs to the trait.
        let name = body_symbol(self.checked, method);
        let returns = self.ir_type(method.returns);
        let params: Vec<IrType> = method.params.iter().map(|p| self.ir_type(p.ty)).collect();

        let virtual_index = method.overridden.then_some(method.index as u32);

        let receiver = self.lower_expr(&field.object);
        let mut args = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        // A method some subclass redefines has no statically known target, so
        // it goes through the object's own table. Every other call is direct.
        Some(match virtual_index {
            Some(index) => self.emit(
                InstKind::CallVirtual {
                    object: receiver,
                    index,
                    args,
                },
                returns,
                span,
            ),
            None => {
                let mut all = vec![receiver];
                all.extend(args);
                self.emit(
                    InstKind::Call {
                        callee: name,
                        args: all,
                    },
                    returns,
                    span,
                )
            }
        })
    }

    /// The layout id of a class, if the name is one.
    fn class_id(&self, name: &str) -> Option<u32> {
        self.module
            .objects
            .iter()
            .position(|o| o.name == name)
            .map(|i| i as u32)
    }

    /// The layout id a construction call builds.
    ///
    /// The target type of a deep contextual conversion call, if this is one
    /// (`ZIRK_LANGUAGE_SPEC.md` section 3, roadmap Phase 3b task 7):
    /// `Float(3 / 4)`, `String("x=" + 42)`. A native scalar name can never
    /// collide with a declared class (type names are reserved), so this is
    /// checked ahead of `construction_class_id` — but a local binding could
    /// still shadow the name (nothing stops `mut Float = 5;`), which is why
    /// this checks the scopes the same way `is_closure_call` does before
    /// trusting the name.
    fn context_conversion_target(&self, call: &ast::CallExpr) -> Option<Type> {
        let ast::Expr::Path(callee) = &*call.callee else {
            return None;
        };
        if self
            .scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(&callee.name))
        {
            return None;
        }
        let target = Type::from_name(&callee.name)?;
        matches!(target.base, Base::Int(_) | Base::Float(_) | Base::String).then_some(target)
    }

    /// Whether `call` is `fatalError(...)` (roadmap Phase 4a) — checked the
    /// same way `context_conversion_target` protects `Float`/`String`: a
    /// local binding could shadow the name (`mut fatalError = 5;`), so the
    /// name alone is not enough.
    fn is_fatal_error_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Path(callee) = &*call.callee else {
            return false;
        };
        callee.name == "fatalError"
            && !self
                .scopes
                .iter()
                .rev()
                .any(|scope| scope.contains_key(&callee.name))
    }

    /// Lowers `fatalError(message)`: reports `message` and terminates the
    /// process, never returning normally (`InstKind::FatalError`'s own doc
    /// comment). The current block ends in `Terminator::Unreachable`
    /// immediately after — nothing following it in source can ever run —
    /// and a fresh block picks up from there purely so this function can
    /// still return an `Operand` the way every other expression lowering
    /// does; nothing that matters ever reads it (a diverging `if`/ternary
    /// branch skips the `Store` entirely instead, see `lower_if_expr`).
    fn lower_fatal_error_call(&mut self, call: &ast::CallExpr, span: Span) -> Operand {
        let message = self.lower_expr_as(&call.args[0].value, IrType::String);
        self.lower_fatal_error(message, span)
    }

    /// The operand-level core of [`Self::lower_fatal_error_call`], reused by
    /// `Result::unwrap`/`unwrap_error` (roadmap Phase 4a) for the message
    /// they build themselves rather than take from a program-written
    /// argument.
    fn lower_fatal_error(&mut self, message: Operand, span: Span) -> Operand {
        let result = self.emit(InstKind::FatalError(message), IrType::Never, span);
        self.terminate(Terminator::Unreachable);
        let unreachable = self.new_block();
        self.current = unreachable;
        result
    }

    /// Whether lowering this expression opens blocks of its own.
    ///
    /// Only these constructs move the insertion point, and that is what
    /// forces an earlier value to travel through a slot. A method, needing
    /// `self`: a call opens blocks not only when one of its arguments does,
    /// but also when it is itself one of `Result<T,E>`'s branching methods
    /// (`ok_or_null`, `get_or`, `unwrap`, … — `Self::result_method`) —
    /// unlike `to_string()` or a construction, those lower to a branch of
    /// their own (roadmap Phase 4a).
    ///
    /// Every ordinary call opens blocks too, unconditionally, since
    /// `fase-4d-runtimeerror`'s D11: [`Self::lower_throws_check`] now runs
    /// after *every* call (not only one whose static target declares
    /// `throws`), and it always branches on
    /// `zirk_rt_has_pending_exception()`. Before D11 this arm only opened
    /// blocks through a nested `Result` method or a branching operand — a
    /// plain call was "leaf-shaped" as far as this function was concerned,
    /// and no longer is: a `1 + length(n.next)` recursive call now moves the
    /// insertion point exactly as much as an `if` would, so `1` (the left
    /// operand, evaluated first) has to be held in a slot across it the same
    /// way. Marking every call this way is a deliberate over-approximation —
    /// a construction or a closure call never actually branches — but
    /// telling those apart here would just re-derive the same resolution
    /// `Self::lower_expr`'s own call arm already does, for a correctness
    /// question this coarser answer already gets right.
    fn opens_blocks(&self, expr: &ast::Expr) -> bool {
        match expr {
            // A ternary opens blocks for the same reason an `if` does: only
            // one of its branches runs.
            ast::Expr::If(_) | ast::Expr::Match(_) | ast::Expr::Ternary(_) => true,
            ast::Expr::Binary(e) => {
                matches!(
                    e.op,
                    ast::BinaryOp::Coalesce | ast::BinaryOp::And | ast::BinaryOp::Or
                ) || self.opens_blocks(&e.left)
                    || self.opens_blocks(&e.right)
            }
            ast::Expr::Unary(e) => self.opens_blocks(&e.operand),
            // `?.` itself lowers to the same absent/present/continue split as
            // an `if` (`Self::lower_safe_field`/`lower_safe_method_call`), so
            // it has to be recognized here too — missing this let a `?.`
            // buried inside a call's argument or a field chain strand an
            // earlier operand across the branch it opens (found via a
            // recursive traversal combining a call with a `?.` read of the
            // same discriminant in one arm).
            ast::Expr::Field(e) => e.safe || self.opens_blocks(&e.object),
            ast::Expr::Call(_) => true,
            ast::Expr::Println(e) => self.opens_blocks(&e.arg),
            ast::Expr::Interpolated(e) => e.parts.iter().any(|p| match p {
                ast::InterpolatedPart::Expr(inner) => self.opens_blocks(inner),
                ast::InterpolatedPart::Literal(_) => false,
            }),
            _ => false,
        }
    }

    /// Which of `Result<T,E>`'s seven in-scope methods `call` is, if any
    /// (`Checker::check_result_method_call`'s IR-side counterpart —
    /// `get_or_else` and the four generic combinators are out of scope for
    /// this pass, `fase-4a-errores/proposal.md`). Returns the method name
    /// plus the receiver's own resolved `T`/`E`.
    fn result_method(&self, call: &ast::CallExpr) -> Option<(&'static str, IrType, IrType)> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if field.safe {
            return None;
        }
        let name = match field.name.name.as_str() {
            "is_ok" => "is_ok",
            "is_error" => "is_error",
            "ok_or_null" => "ok_or_null",
            "error_or_null" => "error_or_null",
            "unwrap" => "unwrap",
            "unwrap_error" => "unwrap_error",
            "get_or" => "get_or",
            _ => return None,
        };
        let IrType::Enum(module_id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };
        if module_id < self.enum_instance_base {
            return None;
        }
        let instance = &self.checked.enum_instances[(module_id - self.enum_instance_base) as usize];
        if Some(instance.enum_id) != self.checked.native_result {
            return None;
        }
        Some((
            name,
            self.ir_type(instance.args[0]),
            self.ir_type(instance.args[1]),
        ))
    }

    /// Lowers a call to one of `Result<T,E>`'s seven in-scope methods.
    fn lower_result_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let (name, t, e) = self.result_method(call)?;
        let ast::Expr::Field(field) = &*call.callee else {
            unreachable!("checked by `result_method`")
        };

        let receiver_ty = self.type_of(&field.object, field.object.span());
        let IrType::Enum(module_id) = receiver_ty else {
            unreachable!("checked by `result_method`")
        };
        let receiver = self.lower_expr(&field.object);
        let holder = self.declare_slot("<result>", receiver_ty, span);
        self.emit_effect(InstKind::Store(holder, receiver), span);
        let object = self.emit(InstKind::Load(holder), receiver_ty, span);
        let discriminant = self.emit(
            InstKind::Discriminant(object),
            IrType::Int(IntWidth::I32),
            span,
        );

        // `Ok` is declared before `Error` (`register_native_result_enum`),
        // so its discriminant and field index are always 0 and its variant
        // 1 — the same order `specialize_enum` preserves.
        let ok_field = self.module.enums[module_id as usize].variants[0][0];
        let error_field = self.module.enums[module_id as usize].variants[1][0];

        Some(match name {
            "is_ok" | "is_error" => {
                let target = self.emit(
                    InstKind::ConstInt(if name == "is_ok" { 0 } else { 1 }),
                    IrType::Int(IntWidth::I32),
                    span,
                );
                self.emit(
                    InstKind::Binary {
                        op: BinaryOp::Eq,
                        left: discriminant,
                        right: target,
                    },
                    IrType::Boolean,
                    span,
                )
            }
            "ok_or_null" => {
                self.lower_result_or_null(holder, receiver_ty, discriminant, ok_field, t, span)
            }
            "error_or_null" => {
                self.lower_result_or_null(holder, receiver_ty, discriminant, error_field, e, span)
            }
            "unwrap" => self.lower_result_unwrap(
                holder,
                receiver_ty,
                discriminant,
                ok_field,
                t,
                "Error",
                span,
            ),
            "unwrap_error" => self.lower_result_unwrap(
                holder,
                receiver_ty,
                discriminant,
                error_field,
                e,
                "Ok",
                span,
            ),
            "get_or" => {
                let default = self.lower_expr_as(&call.args[0].value, t);
                self.lower_result_get_or(
                    holder,
                    receiver_ty,
                    discriminant,
                    ok_field,
                    t,
                    default,
                    span,
                )
            }
            _ => unreachable!("checked by `result_method`"),
        })
    }

    /// `result.ok_or_null()`/`.error_or_null()`: the named field, wrapped
    /// nullable, when the discriminant names that side; `null` otherwise.
    #[allow(clippy::too_many_arguments)]
    fn lower_result_or_null(
        &mut self,
        holder: SlotId,
        object_ty: IrType,
        discriminant: Operand,
        field: u32,
        payload: IrType,
        span: Span,
    ) -> Operand {
        let base = Nullable::of(payload)
            .expect("`Result`'s own T/E can never be a closure type — no syntax names one (D9)");
        let result_ty = IrType::Nullable(base);
        let result = self.declare_slot("<result_or_null>", result_ty, span);
        let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
        let matches = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: discriminant,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        // `field` already picks out the right side (`ok_field` when this is
        // `ok_or_null`, `error_field` when it is `error_or_null`), so the
        // same discriminant-vs-`Ok` test the caller derives its own branch
        // order from is reused, and only one of `field`/`discriminant`
        // pairs is ever actually the one this compares — see the two call
        // sites in `lower_result_method_call`.
        let present_block = self.new_block();
        let absent_block = self.new_block();
        let continue_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: matches,
            then_block: if field == 0 {
                present_block
            } else {
                absent_block
            },
            else_block: if field == 0 {
                absent_block
            } else {
                present_block
            },
        });

        self.current = present_block;
        let object = self.emit(InstKind::Load(holder), object_ty, span);
        let value = self.emit(
            InstKind::LoadField {
                object,
                index: field,
            },
            payload,
            span,
        );
        let value = match payload {
            IrType::Nullable(_) => value,
            _ => self.emit(InstKind::Wrap { base, value }, result_ty, span),
        };
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = absent_block;
        let absent = self.emit(InstKind::NullValue(base), result_ty, span);
        self.emit_effect(InstKind::Store(result, absent), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), result_ty, span)
    }

    /// `result.get_or(default_value)`: the `Ok` payload, or `default_value`
    /// when the receiver is `Error`.
    #[allow(clippy::too_many_arguments)]
    fn lower_result_get_or(
        &mut self,
        holder: SlotId,
        object_ty: IrType,
        discriminant: Operand,
        ok_field: u32,
        t: IrType,
        default: Operand,
        span: Span,
    ) -> Operand {
        let result = self.declare_slot("<result_get_or>", t, span);
        // `default` was computed in the block active before this branch —
        // values do not cross blocks (ADR-007) — so it travels through a
        // slot to reach `error_block` the same way `holder`'s own payload
        // does.
        let default_holder = self.declare_slot("<result_default>", t, span);
        self.emit_effect(InstKind::Store(default_holder, default), span);
        let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
        let is_ok = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: discriminant,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let ok_block = self.new_block();
        let error_block = self.new_block();
        let continue_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_ok,
            then_block: ok_block,
            else_block: error_block,
        });

        self.current = ok_block;
        let object = self.emit(InstKind::Load(holder), object_ty, span);
        let value = self.emit(
            InstKind::LoadField {
                object,
                index: ok_field,
            },
            t,
            span,
        );
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = error_block;
        let default = self.emit(InstKind::Load(default_holder), t, span);
        self.emit_effect(InstKind::Store(result, default), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), t, span)
    }

    /// `result.unwrap()`/`.unwrap_error()`: the named field when the
    /// discriminant names that side; a `fatalError` naming the *other*
    /// variant (`wrong_variant`) otherwise — the assertion the doc comment
    /// of `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 2 describes.
    /// The message is a static string rather than one built from the
    /// payload's own text form: `T`/`E` are not guaranteed `is_printable`
    /// (a class with no `to_string()`, say), and there is no source-location
    /// plumbing yet for any compiler-generated failure (`zirk_rt_overflow`
    /// and its siblings have the same gap) — both are narrower than this
    /// pass needs to close.
    #[allow(clippy::too_many_arguments)]
    fn lower_result_unwrap(
        &mut self,
        holder: SlotId,
        object_ty: IrType,
        discriminant: Operand,
        field: u32,
        payload: IrType,
        wrong_variant: &str,
        span: Span,
    ) -> Operand {
        let result = self.declare_slot("<result_unwrap>", payload, span);
        let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
        // `field == 0` names the `Ok` side (`unwrap`); the other
        // (`unwrap_error`) wants the discriminant to be `Error` instead.
        let op = if field == 0 {
            BinaryOp::Eq
        } else {
            BinaryOp::NotEq
        };
        let matches = self.emit(
            InstKind::Binary {
                op,
                left: discriminant,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let present_block = self.new_block();
        let wrong_block = self.new_block();
        let continue_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: matches,
            then_block: present_block,
            else_block: wrong_block,
        });

        self.current = present_block;
        let object = self.emit(InstKind::Load(holder), object_ty, span);
        let value = self.emit(
            InstKind::LoadField {
                object,
                index: field,
            },
            payload,
            span,
        );
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = wrong_block;
        let text = format!(
            "called `unwrap{}()` on a `Result.{wrong_variant}` value",
            if field == 0 { "" } else { "_error" }
        );
        let id = self.module.intern_string(&text);
        let message = self.emit(InstKind::ConstString(id), IrType::String, span);
        // `Never`, like a diverging `if`/ternary branch (`lower_if_expr`):
        // nothing is stored and no jump to `continue_block` is made — the
        // dangling block `lower_fatal_error` leaves `self.current` on is
        // simply closed off here instead.
        let _ = self.lower_fatal_error(message, span);
        self.terminate(Terminator::Unreachable);

        self.current = continue_block;
        self.emit(InstKind::Load(result), payload, span)
    }

    /// Lowers the operand tree of a deep contextual conversion (task 7),
    /// converting at each leaf rather than the whole result after — mirrors
    /// `zirk-sema/checker.rs`'s `check_context_tree`, which already proved
    /// this tree is well-formed for `target`.
    ///
    /// Two operands of a compatible operator are held across each other the
    /// same way any other binary operand is (`lower_and_hold`): a later one
    /// that opens blocks would otherwise strand the earlier value across a
    /// block boundary the IR forbids (ADR-007).
    fn lower_context_tree(&mut self, target: IrType, expr: &ast::Expr) -> Operand {
        let span = expr.span();
        let numeric = matches!(target, IrType::Int(_) | IrType::Float(_));
        let is_string = target == IrType::String;

        let compatible_op = |op: ast::BinaryOp| {
            (numeric
                && matches!(
                    op,
                    ast::BinaryOp::Add
                        | ast::BinaryOp::Sub
                        | ast::BinaryOp::Mul
                        | ast::BinaryOp::Div
                        | ast::BinaryOp::Rem
                ))
                || (is_string && op == ast::BinaryOp::Add)
        };

        match expr {
            ast::Expr::Binary(b) if compatible_op(b.op) => {
                let held = self.lower_context_hold(target, &b.left, self.opens_blocks(&b.right));
                let right = self.lower_context_tree(target, &b.right);
                let left = self.reload(held, b.left.span());

                if is_string {
                    self.emit(InstKind::Concat { left, right }, IrType::String, span)
                } else {
                    let op = binary_op(b.op);
                    self.emit_checked_binary(op, left, right, target, span)
                }
            }
            ast::Expr::Unary(u) if numeric && u.op == ast::UnaryOp::Neg => {
                let operand = self.lower_context_tree(target, &u.operand);
                self.emit(
                    InstKind::Unary {
                        op: UnaryOp::Neg,
                        operand,
                    },
                    target,
                    span,
                )
            }
            _ => self.lower_context_leaf(target, expr, span),
        }
    }

    /// [`Self::lower_and_hold`], for a value produced by `lower_context_tree`
    /// rather than `lower_expr`.
    fn lower_context_hold(&mut self, target: IrType, expr: &ast::Expr, will_branch: bool) -> Held {
        let value = self.lower_context_tree(target, expr);
        if !will_branch {
            return Held::Value(value);
        }
        let slot = self.declare_slot("<context>", target, expr.span());
        self.emit_effect(InstKind::Store(slot, value), expr.span());
        Held::Spilled(slot, target)
    }

    /// A leaf of a contextual conversion tree: lowered with its own type,
    /// then converted into `target` — a numeric leaf reaches any other
    /// numeric target unchecked (the same as `as`), and any printable value
    /// reaches `String` through `to_string()` (`Self::lower_to_string`).
    fn lower_context_leaf(&mut self, target: IrType, expr: &ast::Expr, span: Span) -> Operand {
        let operand = self.lower_expr(expr);
        let actual = self.type_of_operand(operand);

        if actual == target {
            return operand;
        }
        match (actual, target) {
            (IrType::Int(_), IrType::Int(_)) => self.emit(InstKind::IntCast(operand), target, span),
            (IrType::Float(_), IrType::Float(_)) => {
                self.emit(InstKind::FloatCast(operand), target, span)
            }
            (IrType::Int(_), IrType::Float(_)) => {
                self.emit(InstKind::IntToFloat(operand), target, span)
            }
            (IrType::Float(_), IrType::Int(_)) => {
                self.emit(InstKind::FloatToInt(operand), target, span)
            }
            (_, IrType::String) => self.lower_to_string(operand, actual, span),
            _ => unreachable!("the checker validated this conversion"),
        }
    }

    /// A generic class's own bare name (`Box`) cannot tell `Box<Int32>` from
    /// `Box<String>` apart the way `class_id` resolves an ordinary class —
    /// the checker records which instantiation each call site inferred
    /// (`Self::checked`'s `generic_constructions`, roadmap task 11.1), and
    /// that takes priority whenever this call's span is one.
    fn construction_class_id(&self, call: &ast::CallExpr) -> Option<u32> {
        if let Some(&instance) = self.checked.generic_constructions.get(&call.span) {
            return Some(self.instance_base + instance);
        }
        self.class_id(&self.callee_name(call))
    }

    /// Lowers `User(...)` into an allocation plus a call to its constructor.
    ///
    /// The two steps are what separates identity from initialization: the
    /// object exists — and has its address, which is its identity — before its
    /// constructor runs on it.
    fn lower_construction(&mut self, call: &ast::CallExpr, id: u32, span: Span) -> Operand {
        let object = self.emit(InstKind::Alloc(id), IrType::Object(id), span);

        // `StackTrace()` (roadmap Phase 4b): the compiler-injected class has
        // no `program.classes` entry, so there is no real constructor
        // symbol to call — nor any field to initialize (`register_native_exception_hierarchy`'s
        // own doc comment). The allocation alone is the whole of it, the
        // same "inject the tables directly, nothing to lower into a body"
        // shape `Result`'s own construction needs none of, since it is an
        // enum variant rather than a class at all.
        if self
            .checked
            .native_exceptions
            .is_some_and(|n| n.stack_trace == id)
        {
            return object;
        }

        // One of the four native failure classes (`fase-4d-runtimeerror`,
        // D8/D9): concrete and nameable, so a program may write
        // `DivisionByZeroError("custom reason")` exactly like any other
        // `implements RuntimeError` class — nothing rejects it — but there is
        // still no `program.classes` entry and so no real `construct`
        // symbol to call. Its single field (`reason`, always index `0`) is
        // written directly instead, the same shape
        // `Self::build_native_failure` uses when the compiler itself throws
        // one of these from a failed native check.
        if self.is_native_failure_class(id) {
            let value = self.lower_expr_as(&call.args[0].value, IrType::String);
            self.emit_effect(
                InstKind::StoreField {
                    object,
                    index: 0,
                    value,
                },
                span,
            );
            return object;
        }

        let index = self.constructor_index(id, call.args.len());
        let params: Vec<IrType> = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.constructors.get(index))
            .map(|p| p.iter().map(|param| self.ir_type(param.ty)).collect())
            .expect("the checker resolved the constructor");

        let mut args = vec![object];
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        let name = self.module.objects[id as usize].name.clone();
        self.emit_effect(
            InstKind::Call {
                callee: constructor_symbol(&name, index),
                args,
            },
            span,
        );

        object
    }

    /// Lowers `Point(x: 1, y: 2)`: a record or value class's implicit
    /// construction (roadmap task 11.5), packaging every field's value in
    /// declaration order rather than obtaining storage and calling into it
    /// — there is no `construct` to call, and nothing here allocates.
    ///
    /// Arguments are always named (the checker requires it), and match a
    /// field the same way regardless of the order they were written in; an
    /// omitted field — one the checker confirmed has a default — takes it.
    fn lower_record_construction(&mut self, call: &ast::CallExpr, id: u32, span: Span) -> Operand {
        let field_names: Vec<String> = self.module.values[id as usize]
            .fields
            .iter()
            .map(|f| f.name.clone())
            .collect();
        let field_types: Vec<IrType> = self.module.values[id as usize]
            .fields
            .iter()
            .map(|f| f.ty)
            .collect();

        let mut given: Vec<Option<Operand>> = vec![None; field_names.len()];
        for arg in &call.args {
            let name = &arg
                .name
                .as_ref()
                .expect("the checker requires named arguments for a record")
                .name;
            let index = field_names
                .iter()
                .position(|n| n == name)
                .expect("the checker resolved the field against this layout");
            given[index] = Some(self.lower_expr_as(&arg.value, field_types[index]));
        }

        let fields: Vec<Operand> = given
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                value.unwrap_or_else(|| {
                    self.default_value(field_types[index], span)
                        .expect("the checker required a default for an omitted field")
                })
            })
            .collect();

        self.emit(
            InstKind::BuildValue { class: id, fields },
            IrType::Value(id),
            span,
        )
    }

    /// The enum and discriminant a call constructs, if it names a variant
    /// with associated data (`Shape.Circle(radius: 5)`) — the checker marks
    /// such a call's callee span the same way it marks a bare variant
    /// reference's (`self.checked.variant_accesses`), so this is the call
    /// equivalent of that check.
    fn variant_construction(&self, call: &ast::CallExpr) -> Option<(u32, u32)> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        let ast::Expr::Path(enum_name) = &*field.object else {
            return None;
        };
        // A generic enum's variant construction resolves against the
        // specific instantiation the checker inferred from this call's own
        // arguments (roadmap task 13.5, `CheckedProgram::variant_constructions`)
        // — the enum construction equivalent of `Self::construction_class_id`
        // consulting `generic_constructions` first. Every other enum's bare
        // name is unambiguous, the same way a non-generic class's is.
        let enum_id = match self.checked.variant_constructions.get(&call.span) {
            Some(&instance) => self.enum_instance_base + instance,
            None => {
                let declared = self.declaration_of(&enum_name.name, enum_name.span);
                self.checked.enums.iter().position(|e| e.name == declared)? as u32
            }
        };
        let variant = self.checked.enums[enum_id as usize].discriminant(&field.name.name)?;
        Some((enum_id, variant))
    }

    /// Lowers `Shape.Circle(radius: 5)`: an algebraic enum's variant
    /// construction (roadmap task 11.3), packaging the discriminant plus
    /// this variant's own associated fields — the checker requires every
    /// variant with data to have some (`Self::variant_construction` only
    /// returns one that does), so there is nothing to default the way a
    /// record's omitted field can be.
    ///
    /// Arguments may be positional or named, the same as an ordinary call
    /// (unlike a record, which the checker requires named-only): a bare
    /// position matches the associated field at that position, a named one
    /// matches by name regardless of where it was written.
    fn lower_variant_construction(
        &mut self,
        call: &ast::CallExpr,
        enum_id: u32,
        variant: u32,
        span: Span,
    ) -> Operand {
        let associated =
            &self.checked.enums[enum_id as usize].variants[variant as usize].associated;
        let field_names: Vec<String> = associated.iter().map(|f| f.name.clone()).collect();
        let field_types: Vec<IrType> = associated.iter().map(|f| self.ir_type(f.ty)).collect();

        let mut given: Vec<Option<Operand>> = vec![None; field_names.len()];
        for (position, arg) in call.args.iter().enumerate() {
            let index = match &arg.name {
                Some(name) => field_names
                    .iter()
                    .position(|n| n == &name.name)
                    .expect("the checker resolved the field against this variant"),
                None => position,
            };
            given[index] = Some(self.lower_expr_as(&arg.value, field_types[index]));
        }

        let fields: Vec<Operand> = given
            .into_iter()
            .map(|value| value.expect("the checker required every associated field"))
            .collect();

        self.emit(
            InstKind::BuildEnum {
                enum_id,
                variant,
                fields,
            },
            IrType::Enum(enum_id),
            span,
        )
    }

    /// Lowers `value as Target` (roadmap task 11.6).
    ///
    /// The identity shape — same base on both sides, whatever kind it is —
    /// needs no runtime check at all: the value already is the target, so
    /// it passes through unchanged. The checker's `cast_is_directly_lowerable`
    /// only accepts that shape or a class/contract value checked against a
    /// class's descriptor, so a target that is not itself already the
    /// operand's type is always `IrType::Object` here.
    fn lower_cast(&mut self, expr: &ast::CastExpr, span: Span) -> Operand {
        let target_ty = self.ir_type_from_ref(&expr.target);
        let value = self.lower_expr(&expr.expr);
        let actual_ty = self.type_of_operand(value);

        if actual_ty == target_ty {
            return value;
        }

        if let (IrType::Int(_), IrType::Int(_)) = (actual_ty, target_ty) {
            return self.emit(InstKind::IntCast(value), target_ty, span);
        }
        if let (IrType::Float(_), IrType::Float(_)) = (actual_ty, target_ty) {
            return self.emit(InstKind::FloatCast(value), target_ty, span);
        }
        if let (IrType::Int(_), IrType::Float(_)) = (actual_ty, target_ty) {
            return self.emit(InstKind::IntToFloat(value), target_ty, span);
        }
        if let (IrType::Float(_), IrType::Int(_)) = (actual_ty, target_ty) {
            return self.emit(InstKind::FloatToInt(value), target_ty, span);
        }
        // `ptr as Pointer<U>` — `.cast<U>()`'s spelling (design D8, tasks.md
        // 6.2's decision note): an LLVM pointer bitcast, unchecked.
        if let (IrType::Pointer(_), IrType::Pointer(_)) = (actual_ty, target_ty) {
            return self.emit(InstKind::PointerCast(value), target_ty, span);
        }

        let IrType::Object(target_class) = target_ty else {
            unreachable!(
                "the checker only lowers a cast whose target is a class, once identity is ruled out"
            )
        };
        self.emit(
            InstKind::CheckedCast {
                object: value,
                target_class,
            },
            target_ty,
            span,
        )
    }

    /// Which `construct` a call of this arity resolves to.
    fn constructor_index(&self, id: u32, arity: usize) -> usize {
        self.checked.classes[id as usize]
            .constructors
            .iter()
            .position(|params| {
                let required = params
                    .iter()
                    .filter(|p| !p.optional && !p.variadic && !p.has_default)
                    .count();
                let variadic = params.iter().any(|p| p.variadic);
                arity >= required && (variadic || arity <= params.len())
            })
            .expect("the checker resolved the constructor")
    }

    /// Lowers `a.b`, which is a variant access or a field read.
    ///
    /// The checker already decided which one it is and recorded the answer, so
    /// this reads the decision rather than repeating it with the same tables.
    fn lower_field(&mut self, expr: &ast::FieldExpr, span: Span) -> Operand {
        if expr.name.name == "is_null"
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Pointer(_)
            )
        {
            let pointer = self.lower_expr(&expr.object);
            return self.emit(InstKind::PointerIsNull(pointer), IrType::Boolean, span);
        }

        if self.checked.variant_accesses.contains(&expr.span) {
            let ast::Expr::Path(enum_name) = &*expr.object else {
                unreachable!("a variant access names its enum")
            };
            let enum_id = self.enum_id_of(enum_name);
            let discriminant = self.discriminant(enum_name, &expr.name.name);
            // A bare access only ever names a variant with no associated
            // data (the checker rejects any other shape) — but the enum it
            // belongs to may still carry a payload elsewhere (`Shape.Empty`
            // alongside `Shape.Circle(radius: Int32)`), which is what
            // decides its representation: the discriminant alone, or the
            // same `discriminant`-plus-payload shape every other value of
            // that enum has (with nothing to fill in for this variant's own
            // fields). This is the standalone counterpart of the
            // destination-driven build `lower_expr_as` does for a generic
            // enum, where a bare id is not the specialized one needed.
            return if enum_has_payload(self.checked, enum_id) {
                self.emit(
                    InstKind::BuildEnum {
                        enum_id,
                        variant: discriminant as u32,
                        fields: Vec::new(),
                    },
                    IrType::Enum(enum_id),
                    span,
                )
            } else {
                self.emit(
                    InstKind::ConstInt(discriminant),
                    IrType::Int(IntWidth::I32),
                    span,
                )
            };
        }

        if expr.safe {
            return self.lower_safe_field(expr, span);
        }

        let object = self.lower_expr(&expr.object);
        let (index, ty) = self.field_position(&expr.object, &expr.name.name);
        self.emit(InstKind::LoadField { object, index }, ty, span)
    }

    /// Lowers `object?.field`: a null check with two blocks, the same shape
    /// `??` uses (D7). The absent branch answers null; the present branch
    /// unwraps the receiver, reads the field, and wraps the result — a
    /// non-nullable field read through `?.` still comes back nullable.
    fn lower_safe_field(&mut self, expr: &ast::FieldExpr, span: Span) -> Operand {
        let (index, field_ty) = self.field_position(&expr.object, &expr.name.name);
        let result_type = match field_ty {
            IrType::Nullable(_) => field_ty,
            _ => IrType::Nullable(
                Nullable::of(field_ty).expect("a field reachable through `?.` has a nullable form"),
            ),
        };

        let object_ty = self.type_of(&expr.object, expr.object.span());
        // `Checker::reject_redundant_safe` rejects `?.` on a non-nullable
        // receiver unconditionally (including a narrowing-derived one), so
        // a checked program never reaches lowering with `object_ty` other
        // than `Nullable` here.
        debug_assert!(matches!(object_ty, IrType::Nullable(_)));

        let result = self.declare_slot("<safe_field>", result_type, span);

        let receiver = self.lower_expr(&expr.object);
        let test = self.emit(InstKind::IsNull(receiver), IrType::Boolean, span);

        // The receiver is needed again in the block that unwraps it, and
        // values do not cross blocks (ADR-007).
        let holder = self.declare_slot("<safe_receiver>", object_ty, span);
        self.emit_effect(InstKind::Store(holder, receiver), span);

        let absent_block = self.new_block();
        let present_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition: test,
            then_block: absent_block,
            else_block: present_block,
        });

        self.current = absent_block;
        let IrType::Nullable(base) = result_type else {
            unreachable!("computed above")
        };
        let absent = self.emit(InstKind::NullValue(base), result_type, span);
        self.emit_effect(InstKind::Store(result, absent), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = present_block;
        let held = self.emit(InstKind::Load(holder), object_ty, span);
        let unwrapped = self.emit(InstKind::Unwrap(held), object_ty.unwrapped(), span);
        let value = self.emit(
            InstKind::LoadField {
                object: unwrapped,
                index,
            },
            field_ty,
            span,
        );
        let value = if matches!(field_ty, IrType::Nullable(_)) {
            value
        } else {
            self.emit(InstKind::Wrap { base, value }, result_type, span)
        };
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), result_type, span)
    }

    /// Which body a safe method call (`u?.greet()`) reaches, once the
    /// receiver turns out to be present — an object (direct or through its
    /// own table) or a contract. Shared between [`Self::lower_safe_method_call`]
    /// and `type_of`, so a call's type is known before it is lowered.
    fn safe_method_call_info(
        &self,
        call: &ast::CallExpr,
    ) -> Option<(IrType, Vec<IrType>, SafeDispatch, IrType)> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !field.safe || self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        let IrType::Nullable(inner) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };

        match inner.inner() {
            IrType::Object(id) => {
                let class = &self.checked.classes[id as usize];
                let method = class.method(&field.name.name)?;
                let name = body_symbol(self.checked, method);
                let returns = self.ir_type(method.returns);
                let params = method.params.iter().map(|p| self.ir_type(p.ty)).collect();
                let dispatch = SafeDispatch::Object {
                    name,
                    virtual_index: method.overridden.then_some(method.index as u32),
                };
                Some((returns, params, dispatch, inner.inner()))
            }
            IrType::Contract(id) => {
                let contract = &self.checked.contracts[id as usize];
                let method = contract.method(&field.name.name)?;
                let returns = self.ir_type(method.returns);
                let params = method.params.iter().map(|p| self.ir_type(p.ty)).collect();
                let dispatch = SafeDispatch::Contract {
                    contract: id,
                    index: method.index as u32,
                };
                Some((returns, params, dispatch, inner.inner()))
            }
            // A generic parameter's constraint has no concrete body to call
            // yet — gated at the checker (`Self::check_call`'s `?.` branch).
            _ => None,
        }
    }

    /// Lowers `u?.greet()`: the same absent/present split
    /// [`Self::lower_safe_field`] uses for a field, with the call itself —
    /// direct, through the object's own table, or through a contract's —
    /// inside the present block (roadmap task 10.8).
    fn lower_safe_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let (returns, params, dispatch, unwrapped_ty) = self.safe_method_call_info(call)?;
        let ast::Expr::Field(field) = &*call.callee else {
            unreachable!("checked by `safe_method_call_info`")
        };

        // `Void` has no nullable form (the checker types `objeto?.algo()` as
        // plain `Void` for exactly that reason — `Self::check_call`'s `?.`
        // branch), so there is nothing to store or merge: the call either
        // runs or does not, and neither path leaves a value behind. `result`
        // stays `None` for it, the same way `Self::lower_match` skips a slot
        // for a `Void` arm value.
        let result_type = match returns {
            IrType::Void => IrType::Void,
            IrType::Nullable(_) => returns,
            _ => IrType::Nullable(
                Nullable::of(returns).expect("a method reachable through `?.` has a nullable form"),
            ),
        };
        let object_ty = self.type_of(&field.object, field.object.span());
        // `safe_method_call_info` already bails out (returning `None`, via
        // the `?` above) unless `object_ty` is `Nullable`, and
        // `Checker::reject_redundant_safe` rejects `?.` on a non-nullable
        // receiver unconditionally — so a checked program never reaches
        // this point with anything but a nullable `object_ty`.
        debug_assert!(matches!(object_ty, IrType::Nullable(_)));

        // `Void` has no nullable form (`result_type` stays `Void` for it,
        // above) — there is nothing to store or merge, the same way
        // `Self::lower_match` skips a slot for a `Void` arm value.
        let result = (result_type != IrType::Void)
            .then(|| self.declare_slot("<safe_call>", result_type, span));

        let receiver = self.lower_expr(&field.object);
        let test = self.emit(InstKind::IsNull(receiver), IrType::Boolean, span);

        let holder = self.declare_slot("<safe_receiver>", object_ty, span);
        self.emit_effect(InstKind::Store(holder, receiver), span);

        let absent_block = self.new_block();
        let present_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition: test,
            then_block: absent_block,
            else_block: present_block,
        });

        self.current = absent_block;
        if let Some(result) = result {
            let IrType::Nullable(base) = result_type else {
                unreachable!("computed above")
            };
            let absent = self.emit(InstKind::NullValue(base), result_type, span);
            self.emit_effect(InstKind::Store(result, absent), span);
        }
        self.terminate(Terminator::Jump(continue_block));

        self.current = present_block;
        let held = self.emit(InstKind::Load(holder), object_ty, span);
        let unwrapped = self.emit(InstKind::Unwrap(held), unwrapped_ty, span);

        // Arguments are lowered here, inside the present block: they are
        // only ever evaluated once the receiver is known to exist.
        let mut args = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        let value = match dispatch {
            SafeDispatch::Object {
                name,
                virtual_index,
            } => match virtual_index {
                Some(index) => self.emit(
                    InstKind::CallVirtual {
                        object: unwrapped,
                        index,
                        args,
                    },
                    returns,
                    span,
                ),
                None => {
                    let mut all = vec![unwrapped];
                    all.extend(args);
                    self.emit(
                        InstKind::Call {
                            callee: name,
                            args: all,
                        },
                        returns,
                        span,
                    )
                }
            },
            SafeDispatch::Contract { contract, index } => self.emit(
                InstKind::CallContract {
                    object: unwrapped,
                    contract,
                    index,
                    args,
                },
                returns,
                span,
            ),
        };
        if let Some(result) = result {
            let IrType::Nullable(base) = result_type else {
                unreachable!("computed above")
            };
            let value = if matches!(returns, IrType::Nullable(_)) {
                value
            } else {
                self.emit(InstKind::Wrap { base, value }, result_type, span)
            };
            self.emit_effect(InstKind::Store(result, value), span);
        }
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        Some(match result {
            Some(result) => self.emit(InstKind::Load(result), result_type, span),
            // Nothing reads the result of a `Void` safe call; a placeholder
            // keeps the signature of `Self::lower_expr` total, the same
            // trick `Self::lower_match` uses for a `Void` arm value.
            None => self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span),
        })
    }

    /// The type a member access produces.
    fn field_type_of(&self, expr: &ast::FieldExpr) -> IrType {
        // `.is_null` (roadmap Phase 4e) — the one `Pointer<T>` operation
        // that needs no `unsafe` (`Checker::member_type`'s own matching
        // branch).
        if expr.name.name == "is_null"
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Pointer(_)
            )
        {
            return IrType::Boolean;
        }
        if self.checked.variant_accesses.contains(&expr.span) {
            let ast::Expr::Path(enum_name) = &*expr.object else {
                unreachable!("a variant access names its enum")
            };
            let enum_id = self.enum_id_of(enum_name);
            // A variant without associated data is exactly its discriminant
            // — unless the enum it belongs to carries a payload elsewhere,
            // in which case every value of it shares that representation
            // (see `Self::lower_field`).
            return if enum_has_payload(self.checked, enum_id) {
                IrType::Enum(enum_id)
            } else {
                IrType::Int(IntWidth::I32)
            };
        }
        let field_ty = self.field_position(&expr.object, &expr.name.name).1;
        if !expr.safe || matches!(field_ty, IrType::Nullable(_)) {
            return field_ty;
        }
        // `?.` makes the result nullable even when the field itself is not
        // (D7) — the same widening `Self::lower_safe_field` performs.
        IrType::Nullable(
            Nullable::of(field_ty).expect("a field reachable through `?.` has a nullable form"),
        )
    }

    /// Where a field sits in its object, and what type it holds.
    ///
    /// Unwraps a nullable object type first: `?.`'s own lowering already
    /// established that the receiver is not null before this runs, the same
    /// way `??`'s does for its right-hand side.
    /// Whether `e` is `Pointer.from(place)` (roadmap Phase 4e, design D1) —
    /// the compiler-built-in static call `Checker::check_pointer_from`
    /// already recognized the same way.
    fn is_pointer_from_call(&self, e: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*e.callee else {
            return false;
        };
        let ast::Expr::Path(base) = &*field.object else {
            return false;
        };
        base.name == "Pointer"
            && field.name.name == "from"
            && self.try_lookup_slot(&base.name).is_none()
    }

    /// Whether `e` is `.read()`/`.write(v)`/`.offset(n)`/`.offset_bytes(n)`
    /// on a `Pointer<T>` receiver (design D8). `.is_null` is not here — it
    /// is a member read (`Self::field_type_of`/`Self::lower_field`), never a
    /// call.
    fn is_pointer_method_call(&self, e: &ast::CallExpr) -> bool {
        if self.is_pointer_from_call(e) {
            return false;
        }
        let ast::Expr::Field(field) = &*e.callee else {
            return false;
        };
        if !matches!(
            field.name.name.as_str(),
            "read" | "write" | "offset" | "offset_bytes"
        ) {
            return false;
        }
        matches!(
            self.type_of(&field.object, field.object.span()),
            IrType::Pointer(_)
        )
    }

    /// `Pointer.from(place)` (design D8): the address already computed for
    /// `place`'s own storage — no allocation, just exposing it.
    fn lower_pointer_from(&mut self, place: &ast::Expr, span: Span) -> Operand {
        let pointee = self.type_of(place, place.span());
        let id =
            self.module
                .pointer_types
                .iter()
                .position(|&t| t == pointee)
                .expect("the checker interned every Pointer<T> it type-checked") as u32;
        let ty = IrType::Pointer(id);

        match place {
            ast::Expr::Path(ident) => {
                let slot = self.lookup_slot(&ident.name);
                self.emit(InstKind::PointerFromSlot(slot), ty, span)
            }
            ast::Expr::Field(field) => {
                let (index, _) = self.field_position(&field.object, &field.name.name);
                let object = self.lower_expr(&field.object);
                self.emit(InstKind::PointerFromField { object, index }, ty, span)
            }
            _ => unreachable!("the checker only accepts an addressable place"),
        }
    }

    /// `.read()`/`.write(v)`/`.offset(n)`/`.offset_bytes(n)` (design D8).
    fn lower_pointer_method_call(&mut self, e: &ast::CallExpr, span: Span) -> Operand {
        let ast::Expr::Field(field) = &*e.callee else {
            unreachable!("checked by `Self::is_pointer_method_call`")
        };
        let pointer = self.lower_expr(&field.object);
        let IrType::Pointer(id) = self.type_of_operand(pointer) else {
            unreachable!("checked by `Self::is_pointer_method_call`")
        };

        match field.name.name.as_str() {
            "read" => {
                let t = self.module.pointer_types[id as usize];
                self.emit(InstKind::PointerRead(pointer), t, span)
            }
            "write" => {
                let t = self.module.pointer_types[id as usize];
                let value = self.lower_expr_as(&e.args[0].value, t);
                self.emit(
                    InstKind::PointerWrite { pointer, value },
                    IrType::Void,
                    span,
                )
            }
            "offset" => {
                let amount = self.lower_expr_as(&e.args[0].value, IrType::Int(IntWidth::I32));
                self.emit(
                    InstKind::PointerOffset { pointer, amount },
                    IrType::Pointer(id),
                    span,
                )
            }
            "offset_bytes" => {
                let amount = self.lower_expr_as(&e.args[0].value, IrType::Int(IntWidth::I32));
                self.emit(
                    InstKind::PointerOffsetBytes { pointer, amount },
                    IrType::Pointer(id),
                    span,
                )
            }
            _ => unreachable!("checked by `Self::is_pointer_method_call`"),
        }
    }

    fn field_position(&self, object: &ast::Expr, name: &str) -> (u32, IrType) {
        match self.type_of(object, object.span()).unwrapped() {
            IrType::Object(id) => {
                let layout = &self.module.objects[id as usize];
                let index = layout
                    .field_index(name)
                    .expect("the checker resolved the field against this layout");
                (index as u32, layout.fields[index].ty)
            }
            // A record or value class reads the same way (roadmap task
            // 11.5): `LoadField` does not care whether its operand is a
            // pointer or an inline value, only the LLVM backend does.
            IrType::Value(id) => {
                let layout = &self.module.values[id as usize];
                let index = layout
                    .field_index(name)
                    .expect("the checker resolved the field against this layout");
                (index as u32, layout.fields[index].ty)
            }
            _ => unreachable!("a verified field access reads an object or a value"),
        }
    }

    /// Lowers `cond ? a : b`.
    ///
    /// The same shape as the `if` expression — two blocks writing one slot —
    /// because it means the same thing. What it does not share is the branches
    /// being blocks: here they are expressions, so there is nothing to scope.
    fn lower_ternary(&mut self, expr: &ast::TernaryExpr, span: Span) -> Operand {
        // `Never` contributes nothing at the join (`Type::unify`'s own
        // treatment of it, roadmap Phase 4a) — `cond ? 5 : fatalError(...)`
        // is `Int32`, not `Never`, so the result's own type comes from
        // whichever branch is not the diverging one.
        let true_ty = self.type_of(&expr.when_true, expr.when_true.span());
        let false_ty = self.type_of(&expr.when_false, expr.when_false.span());
        let ty = if true_ty == IrType::Never {
            false_ty
        } else {
            true_ty
        };
        let result = self.declare_slot("<ternary>", ty, span);

        let condition = self.lower_expr(&expr.condition);
        let then_block = self.new_block();
        let else_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        });

        // Only the selected branch runs: the operand of the other one is never
        // evaluated, which is what makes a ternary usable as a guard. A
        // branch whose own type is `Never` diverges instead of producing a
        // value — `fatalError`'s own lowering already ends its block in
        // `Terminator::Unreachable`, so there is nothing to store and no
        // join to reach from here.
        self.current = then_block;
        if true_ty == IrType::Never {
            let _ = self.lower_expr(&expr.when_true);
            self.terminate(Terminator::Unreachable);
        } else {
            let value = self.lower_expr(&expr.when_true);
            self.emit_effect(InstKind::Store(result, value), span);
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = else_block;
        if false_ty == IrType::Never {
            let _ = self.lower_expr(&expr.when_false);
            self.terminate(Terminator::Unreachable);
        } else {
            let value = self.lower_expr(&expr.when_false);
            self.emit_effect(InstKind::Store(result, value), span);
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = continue_block;
        self.emit(InstKind::Load(result), ty, span)
    }

    /// Lowers `i++`, `++i`, `i--` and `--i` used as a value.
    ///
    /// Both forms perform the same update; they differ only in which value the
    /// expression yields, and that is decided by whether the previous value is
    /// read before or after the store.
    fn lower_increment(&mut self, expr: &ast::IncrementExpr, span: Span) -> Operand {
        let slot = self.lookup_slot(expr.target.name());
        let ty = self.slot_type(slot);

        let previous = self.emit(InstKind::Load(slot), ty, span);
        let one = self.emit(InstKind::ConstInt(1), IrType::Int(IntWidth::I32), span);
        let updated = self.emit(
            InstKind::Binary {
                op: binary_op(expr.op.as_binary()),
                left: previous,
                right: one,
            },
            ty,
            span,
        );
        self.emit_effect(InstKind::Store(slot, updated), span);

        match expr.fix {
            ast::IncrementFix::Prefix => updated,
            ast::IncrementFix::Postfix => previous,
        }
    }

    /// Lowers a block used as a value: its statements, then its last
    /// expression.
    fn lower_block_value(&mut self, block: &ast::Block) -> Operand {
        self.scopes.push(HashMap::new());

        let (last, rest) = block
            .statements
            .split_last()
            .expect("a verified block used as a value is not empty");

        for stmt in rest {
            self.lower_stmt(stmt);
        }

        let value = match last {
            ast::Stmt::Expr(e) => self.lower_expr(&e.expr),
            // Same reason as in the checker: an `if` is parsed as a statement
            // wherever it appears, and the position decides what it is.
            ast::Stmt::If(nested) => self.lower_if_expr(nested, nested.span),
            ast::Stmt::Unsafe(nested) => self.lower_block_value(&nested.body),
            ast::Stmt::Commit(nested) => self.lower_block_value(&nested.body),
            _ => unreachable!("a verified block used as a value ends in an expression"),
        };

        self.scopes.pop();
        value
    }

    /// The type a block used as a value produces.
    fn block_value_type(&self, block: &ast::Block) -> IrType {
        match block.statements.last() {
            Some(ast::Stmt::Expr(e)) => self.type_of(&e.expr, e.expr.span()),
            Some(ast::Stmt::If(nested)) => self.block_value_type(&nested.then_branch),
            Some(ast::Stmt::Unsafe(nested)) => self.block_value_type(&nested.body),
            Some(ast::Stmt::Commit(nested)) => self.block_value_type(&nested.body),
            _ => unreachable!("a verified block used as a value ends in an expression"),
        }
    }

    /// Lowers an expression whose value is discarded.
    ///
    /// `Println` and `Void` calls produce no value, so emitting a result for
    /// them would leave a value nobody reads.
    fn lower_expr_for_effect(&mut self, expr: &ast::Expr) {
        match expr {
            // `.write(v)` as a bare statement (roadmap Phase 4e) — checked
            // ahead of the later "void call by name" arm below, whose own
            // guard calls `Self::callee_name` unconditionally and panics on
            // a `Field` callee (`.write` is never a plain-name call).
            ast::Expr::Call(e) if self.is_pointer_method_call(e) => {
                self.lower_pointer_method_call(e, expr.span());
            }
            ast::Expr::Println(e) => {
                let operand = self.lower_println_argument(e, expr.span());
                self.emit_effect(InstKind::Println(operand), expr.span());
            }
            // `super(...)` and a method call know their own target, so they
            // need none of the name resolution the arm below does.
            ast::Expr::Call(e)
                if matches!(&*e.callee, ast::Expr::Super(_))
                    || matches!(&*e.callee, ast::Expr::Field(f) if matches!(&*f.object, ast::Expr::Super(_))) =>
            {
                self.lower_super_call(e, expr.span());
            }
            ast::Expr::Call(e) if self.contract_method_of(e).is_some() => {
                let span = expr.span();
                if let Some(result) = self.lower_contract_call(e, span) {
                    let ty = self.type_of_operand(result);
                    self.lower_throws_check(result, ty, span);
                }
            }
            ast::Expr::Call(e) if self.method_of(e).is_some() => {
                let span = expr.span();
                if let Some(result) = self.lower_method_call(e, span) {
                    let ty = self.type_of_operand(result);
                    self.lower_throws_check(result, ty, span);
                }
            }
            // `u?.greet();` as a bare statement still needs the absent/present
            // split — only the value it produces goes unused.
            ast::Expr::Call(e) if self.safe_method_call_info(e).is_some() => {
                self.lower_safe_method_call(e, expr.span());
            }
            // A closure call goes through the value and has its own arm in
            // `lower_expr`; only a direct call is special-cased here.
            ast::Expr::Call(e)
                if !self.is_closure_call(e) && {
                    let name = self.callee_name(e);
                    self.signature_return(&name)
                } == IrType::Void =>
            {
                let span = expr.span();
                let name = self.callee_name(e).to_string();
                let args = self.lower_args(e);
                let result = self.emit(InstKind::Call { callee: name, args }, IrType::Void, span);
                self.lower_throws_check(result, IrType::Void, span);
            }
            other => {
                self.lower_expr(other);
            }
        }
    }

    /// Converts an already-lowered value to `String` via `to_string()`
    /// (`ZIRK_STDLIB_SPEC.md` section 3, roadmap Phase 3b task 8) — the
    /// shared conversion `println`'s argument and each `{expr}` of an
    /// interpolation both go through.
    ///
    /// A native scalar goes through `InstKind::ToString`, which codegen
    /// dispatches by the operand's own recorded width (`emit.rs`). A
    /// class/record/value class with its own `to_string()` method is called
    /// directly instead — the exact shape any other method call is
    /// (`lower_method_call`), because it needs a real symbol/table lookup
    /// `InstKind::ToString` alone cannot express; the checker already
    /// confirmed the method exists with the right signature
    /// (`Checker::is_printable`), so this only has to find it again, not
    /// re-validate it.
    fn lower_to_string(&mut self, operand: Operand, ty: IrType, span: Span) -> Operand {
        if ty == IrType::String {
            return operand;
        }

        if let IrType::Object(id) | IrType::Value(id) = ty
            && let Some(method) = self.checked.classes[id as usize].method("to_string")
        {
            let name = body_symbol(self.checked, method);
            let virtual_index = method.overridden.then_some(method.index as u32);
            return match virtual_index {
                Some(index) => self.emit(
                    InstKind::CallVirtual {
                        object: operand,
                        index,
                        args: Vec::new(),
                    },
                    IrType::String,
                    span,
                ),
                None => self.emit(
                    InstKind::Call {
                        callee: name,
                        args: vec![operand],
                    },
                    IrType::String,
                    span,
                ),
            };
        }

        // A value reached through a contract that declares `to_string()`:
        // which body runs is not statically known — the whole point of a
        // contract — so it goes through the object's own dispatch table for
        // it, exactly like any other contract method call
        // (`lower_contract_call`).
        if let IrType::Contract(id) = ty
            && let Some(method) = self.checked.contracts[id as usize].method("to_string")
        {
            let index = method.index as u32;
            return self.emit(
                InstKind::CallContract {
                    object: operand,
                    contract: id,
                    index,
                    args: Vec::new(),
                },
                IrType::String,
                span,
            );
        }

        self.emit(InstKind::ToString(operand), IrType::String, span)
    }

    /// Lowers the argument of a `println`, converting it via `to_string()`
    /// when it is not already a `String`.
    ///
    /// `ZIRK_STDLIB_SPEC.md` section 3: every printable value goes through
    /// `to_string()`. Without this the runtime would read an `Int32` as if it
    /// were a pointer.
    fn lower_println_argument(&mut self, expr: &ast::PrintlnExpr, span: Span) -> Operand {
        let operand = self.lower_expr(&expr.arg);
        let ty = self.type_of(&expr.arg, expr.arg.span());
        self.lower_to_string(operand, ty, span)
    }

    /// Lowers `"text {expr} text"` into a chain of `Concat`, converting each
    /// `{expr}` the same way `println`'s own argument is (roadmap Phase 3b).
    ///
    /// A part after an `{expr}` that opens blocks (an `if`, `match` or
    /// ternary inside the interpolation) would otherwise strand every piece
    /// already computed — values do not cross blocks (ADR-007) — so each
    /// piece is held in a slot when a later one branches, the same pattern
    /// [`Self::lower_held_args`] uses for a call's own arguments.
    fn lower_interpolated(&mut self, expr: &ast::InterpolatedStrExpr, span: Span) -> Operand {
        let mut held: Vec<Held> = Vec::with_capacity(expr.parts.len());
        for (position, part) in expr.parts.iter().enumerate() {
            let branches_later = expr.parts[position + 1..]
                .iter()
                .any(|p| matches!(p, ast::InterpolatedPart::Expr(e) if self.opens_blocks(e)));

            let value = match part {
                ast::InterpolatedPart::Literal(text) => {
                    let id = self.module.intern_string(text);
                    self.emit(InstKind::ConstString(id), IrType::String, span)
                }
                ast::InterpolatedPart::Expr(inner) => {
                    let operand = self.lower_expr(inner);
                    let ty = self.type_of(inner, inner.span());
                    self.lower_to_string(operand, ty, span)
                }
            };

            held.push(if branches_later {
                let slot = self.declare_slot("<interp>", IrType::String, span);
                self.emit_effect(InstKind::Store(slot, value), span);
                Held::Spilled(slot, IrType::String)
            } else {
                Held::Value(value)
            });
        }

        let mut pieces = Vec::with_capacity(held.len());
        for h in held {
            pieces.push(self.reload(h, span));
        }

        let mut iter = pieces.into_iter();
        // An interpolation always has at least one `StrPart` — the lexer
        // never produces an empty one, a plain `""` lexes as `TokenKind::Str`
        // instead — but an empty fallback keeps this total rather than
        // leaning on that invariant holding forever.
        let Some(mut acc) = iter.next() else {
            let id = self.module.intern_string("");
            return self.emit(InstKind::ConstString(id), IrType::String, span);
        };
        for piece in iter {
            acc = self.emit(
                InstKind::Concat {
                    left: acc,
                    right: piece,
                },
                IrType::String,
                span,
            );
        }
        acc
    }

    /// Lowers the arguments of a call into the parameters they fill.
    ///
    /// Named arguments are placed by name and omitted ones take their default,
    /// so the call reaching the IR always has the declared arity. That is
    /// decision D4: neither the IR nor LLVM ever sees a named or missing
    /// argument.
    fn lower_args(&mut self, call: &ast::CallExpr) -> Vec<Operand> {
        let name = self.callee_name(call);

        // `extern "C" fn` (roadmap Phase 4e, `ADR-015`): a single-item
        // declaration with no optional/default/variadic/named parameters —
        // every argument is positional, so this needs none of the
        // held-argument reordering below.
        if let Some(signature) = self.checked.externs.get(&name).cloned() {
            let expected: Vec<IrType> = signature
                .params
                .iter()
                .map(|&ty| self.ir_type(ty))
                .collect();
            return self.lower_held_args(&call.args, &expected);
        }

        let signature = self
            .checked
            .functions
            .get(&name)
            .expect("a verified program only calls declared functions");
        let params: Vec<(String, IrType)> = signature
            .params
            .iter()
            .map(|p| (p.name.clone(), self.ir_type(p.ty)))
            .collect();

        let mut slots: Vec<Option<Held>> = (0..params.len()).map(|_| None).collect();
        let mut next = 0usize;

        for (position, arg) in call.args.iter().enumerate() {
            let index = match &arg.name {
                Some(named) => params
                    .iter()
                    .position(|(name, _)| *name == named.name)
                    .expect("the checker resolved every named argument"),
                None => {
                    while next < slots.len() && slots[next].is_some() {
                        next += 1;
                    }
                    let index = next;
                    next += 1;
                    index
                }
            };
            // A later argument that opens blocks would strand the ones already
            // computed, so each is held until every argument is lowered.
            let branches_later = call.args[position + 1..]
                .iter()
                .any(|a| self.opens_blocks(&a.value));
            slots[index] =
                Some(self.lower_and_hold_as(&arg.value, params[index].1, branches_later));
        }

        let declaration = self.declarations.get(name.as_str()).copied();

        slots
            .into_iter()
            .enumerate()
            .map(|(index, filled)| match filled {
                Some(held) => self.reload(held, call.span),
                None => self.lower_default(declaration, index, params[index].1, call.span),
            })
            .collect()
    }

    /// The value a parameter takes when the call omits it.
    fn lower_default(
        &mut self,
        declaration: Option<&ast::FnDecl>,
        index: usize,
        ty: IrType,
        span: Span,
    ) -> Operand {
        let param = declaration
            .and_then(|d| d.params.get(index))
            .expect("the checker rejects a call missing a parameter with no default");

        match &param.default {
            Some(expr) => self.lower_expr_as(expr, ty),
            // An optional parameter with no default is absent, and absence is
            // `null`: the checker made its type nullable for exactly this.
            None => {
                let IrType::Nullable(base) = ty else {
                    unreachable!("an omitted parameter without a default is nullable")
                };
                self.emit(InstKind::NullValue(base), ty, span)
            }
        }
    }

    /// Whether a call goes through a closure value rather than a name.
    /// Whether `call` is `myScalar.to_string()` — the explicit spelling of
    /// the conversion `println`/interpolation reach implicitly (roadmap
    /// Phase 3b, task 8's own follow-up). Excludes `Object`/`Value`/
    /// `Contract` receivers, which already resolve through the ordinary
    /// method-call path (a class/record's own declared `to_string()`).
    fn is_native_to_string_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        field.name.name == "to_string"
            && !self.checked.variant_accesses.contains(&field.span)
            && !matches!(
                self.type_of(&field.object, field.object.span()),
                IrType::Object(_) | IrType::Value(_) | IrType::Contract(_)
            )
    }

    /// Whether `call` is a recursive lambda calling itself by its own
    /// binding name (`ZIRK_LANGUAGE_SPEC.md` section 6, roadmap Phase 4d)
    /// — see `Self::recursive_call`'s own doc comment.
    fn is_recursive_self_call(&self, call: &ast::CallExpr) -> bool {
        let Some((source, ..)) = &self.recursive_call else {
            return false;
        };
        matches!(&*call.callee, ast::Expr::Path(ident) if &ident.name == source)
    }

    fn is_closure_call(&self, call: &ast::CallExpr) -> bool {
        // A method or `super` call is not a closure call, and asking for the
        // type of its callee would ask for the type of a method — which is not
        // a value. Neither is `myScalar.to_string()` — asking for the type of
        // its own callee (`field_type_of`) would try to resolve `to_string`
        // as a *field*, which panics for anything that is not an
        // `Object`/`Value` (a native scalar has no fields at all).
        // Neither is `Pointer.from(place)`/a `Pointer<T>` method (roadmap
        // Phase 4e) — checked first and unconditionally, before any of the
        // calls below: `Pointer.from(...)`'s own callee object is
        // `Expr::Path("Pointer")`, which names no variable or function, so
        // `Self::type_of` would panic trying to resolve it as either
        // (exactly the failure mode this whole guard exists to avoid for
        // `to_string`/method calls, per the comment above).
        if self.is_pointer_from_call(call) || self.is_pointer_method_call(call) {
            return false;
        }

        if self.method_of(call).is_some()
            || self.contract_method_of(call).is_some()
            || self.safe_method_call_info(call).is_some()
            || self.variant_construction(call).is_some()
            || self.is_native_to_string_call(call)
            || self.result_method(call).is_some()
            || matches!(&*call.callee, ast::Expr::Super(_))
        {
            return false;
        }

        match &*call.callee {
            ast::Expr::Path(ident) => self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(&ident.name))
                .is_some_and(|slot| matches!(self.slot_type(*slot), IrType::Closure(_))),
            other => matches!(self.type_of(other, other.span()), IrType::Closure(_)),
        }
    }

    fn signature_return(&self, name: &str) -> IrType {
        if let Some(signature) = self.checked.functions.get(name) {
            return self.ir_type(signature.returns);
        }
        // `extern "C" fn` (roadmap Phase 4e, design D7) — a call to one
        // reaches the same `Call`-lowering path as an ordinary function,
        // named by its own real symbol; only its declaration differs.
        self.checked
            .externs
            .get(name)
            .map(|s| self.ir_type(s.returns))
            .expect("a verified program only calls declared functions")
    }

    /// Derives the type of an expression from the shape of the tree.
    ///
    /// Total on a verified program. It is derivation, not checking: everything
    /// the checker would have rejected never reaches this point.
    fn type_of(&self, expr: &ast::Expr, _span: Span) -> IrType {
        match expr {
            ast::Expr::Int(_) => IrType::Int(IntWidth::I32),
            ast::Expr::Float(lit) => IrType::Float(float_literal_width(lit)),
            ast::Expr::Bool(_) => IrType::Boolean,
            ast::Expr::Str(_) => IrType::String,
            ast::Expr::Char(_) => IrType::Char,
            ast::Expr::Path(ident) => match self.try_lookup_slot(&ident.name) {
                Some(slot) => self.slot_type(slot),
                None => {
                    let (id, _) = self
                        .named_function_value(&ident.name, ident.span)
                        .expect("a verified program only names declared variables or functions");
                    IrType::Closure(id)
                }
            },
            // `Neg`/`BitNot` preserve the operand's own width (any integer
            // width for both, plus any `Float` width for `Neg` — roadmap
            // Phase 3b); only `Not` has a type of its own regardless of the
            // operand.
            ast::Expr::Unary(e) => match e.op {
                ast::UnaryOp::Neg | ast::UnaryOp::BitNot => self.type_of(&e.operand, e.span),
                ast::UnaryOp::Not => IrType::Boolean,
            },
            // `??` yields its operands' shared type, not a boolean or an
            // arithmetic result: it is the one binary operator that is not an
            // operator in the IR at all.
            ast::Expr::Binary(e) if e.op == ast::BinaryOp::Coalesce => {
                let left = self.type_of(&e.left, e.left.span());
                // A literal `null` fallback has no type of its own — see
                // `Self::lower_coalesce`'s matching special case — so the
                // left side's own (still nullable) type stands in for it
                // instead of calling `type_of` on `Expr::Null`.
                let right = match &*e.right {
                    ast::Expr::Null(_) => left,
                    _ => self.type_of(&e.right, e.right.span()),
                };
                if matches!(right, IrType::Nullable(_)) {
                    right
                } else {
                    left.unwrapped()
                }
            }
            ast::Expr::Binary(e)
                if matches!(e.op, ast::BinaryOp::Eq | ast::BinaryOp::NotEq)
                    && self.operator_method_of(e).is_some() =>
            {
                IrType::Boolean
            }
            ast::Expr::Binary(e) if self.operator_method_of(e).is_some() => {
                let (id, name) = self.operator_method_of(e).expect("checked above");
                self.ir_type(
                    self.checked.classes[id as usize]
                        .method(&name)
                        .expect("the checker resolved it")
                        .returns,
                )
            }
            ast::Expr::Binary(e)
                if is_string_operator(
                    self.type_of(&e.left, e.left.span()),
                    self.type_of(&e.right, e.right.span()),
                    e.op,
                ) =>
            {
                IrType::String
            }
            ast::Expr::Binary(e) => {
                binary_op(e.op).result_type(self.type_of(&e.left, e.left.span()))
            }
            ast::Expr::Call(e) if self.is_recursive_self_call(e) => self.return_type,
            ast::Expr::Call(e) if self.is_closure_call(e) => {
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                self.module.closures[id as usize].returns
            }
            ast::Expr::Call(e) if matches!(&*e.callee, ast::Expr::Super(_)) => IrType::Void,
            ast::Expr::Call(e) if self.context_conversion_target(e).is_some() => {
                self.ir_type(self.context_conversion_target(e).expect("checked above"))
            }
            ast::Expr::Call(e) if self.is_fatal_error_call(e) => IrType::Never,
            ast::Expr::Call(e) if self.is_pointer_from_call(e) => {
                let place = &e.args[0].value;
                let pointee = self.type_of(place, place.span());
                let id = self
                    .module
                    .pointer_types
                    .iter()
                    .position(|&t| t == pointee)
                    .expect("the checker interned every Pointer<T> it type-checked")
                    as u32;
                IrType::Pointer(id)
            }
            ast::Expr::Call(e) if self.is_pointer_method_call(e) => {
                let ast::Expr::Field(field) = &*e.callee else {
                    unreachable!("checked by `Self::is_pointer_method_call`")
                };
                let IrType::Pointer(id) = self.type_of(&field.object, field.object.span()) else {
                    unreachable!("checked by `Self::is_pointer_method_call`")
                };
                match field.name.name.as_str() {
                    "read" => self.module.pointer_types[id as usize],
                    "write" => IrType::Void,
                    "offset" | "offset_bytes" => IrType::Pointer(id),
                    _ => unreachable!("checked by `Self::is_pointer_method_call`"),
                }
            }
            ast::Expr::Call(e) => {
                if let Some(method) = self.contract_method_of(e) {
                    return self.ir_type(method.returns);
                }
                if let Some(method) = self.method_of(e) {
                    return self.ir_type(method.returns);
                }
                if let Some((returns, ..)) = self.safe_method_call_info(e) {
                    // `Void` stays `Void` through `?.` (`Self::lower_safe_method_call`'s
                    // doc comment) — there is no nullable form of "no value" to widen to.
                    return match returns {
                        IrType::Void | IrType::Nullable(_) => returns,
                        _ => IrType::Nullable(
                            Nullable::of(returns)
                                .expect("a method reachable through `?.` has a nullable form"),
                        ),
                    };
                }
                if let Some((enum_id, _)) = self.variant_construction(e) {
                    return self.ir_type(Type::of(Base::Enum(enum_id)));
                }
                if self.is_native_to_string_call(e) {
                    return IrType::String;
                }
                if let Some((name, t, e_ty)) = self.result_method(e) {
                    return match name {
                        "is_ok" | "is_error" => IrType::Boolean,
                        "ok_or_null" => IrType::Nullable(
                            Nullable::of(t).expect("`Result`'s T/E cannot be a closure type (D9)"),
                        ),
                        "error_or_null" => IrType::Nullable(
                            Nullable::of(e_ty)
                                .expect("`Result`'s T/E cannot be a closure type (D9)"),
                        ),
                        "unwrap" | "get_or" => t,
                        "unwrap_error" => e_ty,
                        _ => unreachable!("checked by `result_method`"),
                    };
                }
                match self.construction_class_id(e) {
                    Some(id) => self.ir_type(Type::of(Base::Class(id))),
                    None => self.signature_return(&self.callee_name(e)),
                }
            }
            // `Never` contributes nothing at the join (roadmap Phase 4a) —
            // see `lower_if_expr`/`lower_ternary`'s own matching logic.
            ast::Expr::If(e) => {
                let then_ty = self.block_value_type(&e.then_branch);
                if then_ty != IrType::Never {
                    then_ty
                } else {
                    match &e.else_branch {
                        Some(ast::ElseBranch::Block(b)) => self.block_value_type(b),
                        Some(ast::ElseBranch::If(nested)) => {
                            self.block_value_type(&nested.then_branch)
                        }
                        None => unreachable!("a verified `if` expression always has an `else`"),
                    }
                }
            }
            ast::Expr::Field(e) => self.field_type_of(e),
            ast::Expr::This(_) | ast::Expr::Super(_) => self.slot_type(self.lookup_slot("this")),
            ast::Expr::Ternary(e) => {
                let true_ty = self.type_of(&e.when_true, e.when_true.span());
                if true_ty != IrType::Never {
                    true_ty
                } else {
                    self.type_of(&e.when_false, e.when_false.span())
                }
            }
            // Both forms yield the type of the operand they update.
            ast::Expr::Increment(e) => self.slot_type(self.lookup_slot(e.target.name())),
            ast::Expr::Match(e) => self.arm_value_type(e),
            ast::Expr::Variant(_) => IrType::Int(IntWidth::I32),
            ast::Expr::Println(_) => IrType::Void,
            // A lambda's type is the closure layout it produced, which only
            // exists once it has been lowered: the caller asks the value.
            ast::Expr::Lambda(_) | ast::Expr::Null(_) | ast::Expr::Range(_) => {
                unreachable!("the type of this expression comes from the value it produced")
            }
            ast::Expr::Cast(e) => self.ir_type_from_ref(&e.target),
            ast::Expr::Interpolated(_) => IrType::String,
            ast::Expr::Unsafe(u) => self.block_value_type(&u.body),
            ast::Expr::Commit(c) => self.block_value_type(&c.body),
        }
    }

    /// The type of an already emitted value.
    fn type_of_operand(&self, operand: Operand) -> IrType {
        self.value_types
            .get(&operand.0)
            .copied()
            .expect("every emitted value records its type")
    }
}

/// The reserved method an operator resolves to on a user type.
fn operator_method(op: ast::BinaryOp) -> Option<&'static str> {
    use ast::BinaryOp::*;
    Some(match op {
        Add => "_add",
        Sub => "_subtract",
        Mul => "_multiply",
        Div => "_divide",
        Rem => "_remainder",
        // `!=` is `==` negated: one method answers both, so a type cannot
        // define them inconsistently.
        Eq | NotEq => "_equals",
        _ => return None,
    })
}

/// Whether an operator on these operands is one of `String`'s.
fn is_string_operator(left: IrType, right: IrType, op: ast::BinaryOp) -> bool {
    matches!(
        (left, right, op),
        (IrType::String, IrType::String, ast::BinaryOp::Add)
            | (
                IrType::String,
                IrType::Int(IntWidth::I32),
                ast::BinaryOp::Mul
            )
            | (
                IrType::Int(IntWidth::I32),
                IrType::String,
                ast::BinaryOp::Mul
            )
    )
}

fn binary_op(op: ast::BinaryOp) -> BinaryOp {
    use ast::BinaryOp as A;
    match op {
        A::Add => BinaryOp::Add,
        A::Sub => BinaryOp::Sub,
        A::Mul => BinaryOp::Mul,
        A::Div => BinaryOp::Div,
        A::Rem => BinaryOp::Rem,
        A::Eq => BinaryOp::Eq,
        A::NotEq => BinaryOp::NotEq,
        A::Lt => BinaryOp::Lt,
        A::LtEq => BinaryOp::LtEq,
        A::Gt => BinaryOp::Gt,
        A::GtEq => BinaryOp::GtEq,
        A::And => BinaryOp::And,
        A::Or => BinaryOp::Or,
        A::Is => BinaryOp::Identical,
        A::BitAnd => BinaryOp::BitAnd,
        A::BitOr => BinaryOp::BitOr,
        A::BitXor => BinaryOp::BitXor,
        A::Shl => BinaryOp::Shl,
        A::Shr => BinaryOp::Shr,
        // `??` is expanded by the lowering into a null check with two blocks,
        // so it never reaches the IR as an operator.
        A::Coalesce => unreachable!("`??` is lowered into branches, not an operator"),
    }
}

/// How a safe method call (`u?.greet()`) reaches its body, once the
/// receiver is known to be present.
enum SafeDispatch {
    /// Through the object's own class: direct if nobody overrides it,
    /// through its table (`virtual_index`) if some subclass does.
    Object {
        name: String,
        virtual_index: Option<u32>,
    },
    /// Through a contract's table.
    Contract { contract: u32, index: u32 },
}

/// Where a value waits while a later expression is lowered.
enum Held {
    /// Still in its block, because nothing moved the insertion point.
    Value(Operand),
    /// Parked in a slot, because the later expression opened blocks.
    Spilled(SlotId, IrType),
}

/// The name of a directly called function.
///
/// Calling a closure value goes through a different instruction, and the
/// checker rejects anything else, so a verified program only reaches here with
/// a plain name.
fn callee_ident(call: &ast::CallExpr) -> &ast::Ident {
    match &*call.callee {
        ast::Expr::Path(ident) => ident,
        _ => unreachable!("a verified program calls a name or a closure value"),
    }
}
