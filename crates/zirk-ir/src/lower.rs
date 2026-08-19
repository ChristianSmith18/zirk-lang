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
    AssociatedFieldInfo, Base, CheckedProgram, ClassType, EnumType, EnumVariantInfo, FieldInfo,
    MethodInfo, ParamInfo, Type,
};

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
            let mut ancestors = vec![id];
            let mut current = class.base;
            while let Some(base_id) = current {
                ancestors.push(base_id);
                current = checked.classes[base_id as usize].base;
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
                // subclass start with its base's.
                methods: {
                    let mut table = vec![String::new(); class.methods.len()];
                    for method in &class.methods {
                        table[method.index] = body_symbol(checked, method);
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

    // Default values are written in the declaration but evaluated at the call
    // site, so the lowering needs the declarations while lowering the calls.
    let declarations: HashMap<&str, &ast::FnDecl> = program
        .functions
        .iter()
        .map(|f| (f.name.name.as_str(), f))
        .collect();

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
        Base::Int32 => IrType::Int32,
        Base::Boolean => IrType::Boolean,
        Base::String => IrType::String,
        // A traditional enum — none of its variants carry data — is exactly
        // its discriminant. One with at least one algebraic variant gets a
        // representation of its own (roadmap task 11.3).
        Base::Enum(id) if enum_has_payload(checked, id) => IrType::Enum(id),
        Base::Enum(_) => IrType::Int32,
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
                IrType::Int32
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
        Base::Unknown
        | Base::Null
        | Base::Function(_)
        | Base::Range
        | Base::Param(_)
        | Base::Union(_) => {
            unreachable!("lowering received a construct the checker should have rejected")
        }
    };

    if !ty.nullable {
        return base;
    }

    IrType::Nullable(Nullable::of(base).expect("the checker rejects `Void?`"))
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
}

/// The two blocks a loop exposes to the jumps inside it.
struct LoopTargets {
    break_to: BlockId,
    continue_to: BlockId,
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
        Operand(result)
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

    fn lookup_slot(&self, name: &str) -> SlotId {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
            .expect("a verified program only names declared variables")
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
        let declared = self.declaration_of(&reference.name, reference.span);
        if let Some(ty) = Type::from_name(&reference.name) {
            return ty;
        }
        if let Some(id) = self.checked.enums.iter().position(|e| e.name == declared) {
            return Type::of(Base::Enum(id as u32));
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
            let ty = expected.get(position).copied().unwrap_or(IrType::Int32);
            let branches_later = args[position + 1..]
                .iter()
                .any(|later| opens_blocks(&later.value));
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
            // first (mirrors the non-nullable branch below), since `Wrap`
            // itself only adds the present flag, not the widening.
            let value = if actual != base.inner()
                && matches!(actual, IrType::Object(_) | IrType::Contract(_))
                && matches!(base, Nullable::Object(_) | Nullable::Contract(_))
            {
                self.emit(InstKind::Retype(value), base.inner(), expr.span())
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

        value
    }

    // --- Function ---------------------------------------------------------

    fn run(mut self, function: &ast::FnDecl) -> (Function, Vec<Function>) {
        self.return_type = self.ir_type_from_ref(&function.return_type);

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
        let lowered = Function {
            name: function.name.name.clone(),
            params,
            return_type: self.return_type,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: function.span,
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
            IrType::Int32 => self.emit(InstKind::ConstInt(0), ty, span),
            IrType::Boolean => self.emit(InstKind::ConstBool(false), ty, span),
            IrType::String => {
                let id = self.module.intern_string("");
                self.emit(InstKind::ConstString(id), ty, span)
            }
            // Absence is exactly what a nullable type defaults to.
            IrType::Nullable(base) => self.emit(InstKind::NullValue(base), ty, span),
            IrType::Void
            | IrType::Closure(_)
            | IrType::Object(_)
            | IrType::Contract(_)
            | IrType::Value(_)
            | IrType::Enum(_) => {
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
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: method.span,
        };

        (lowered, lifted)
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
            ast::Stmt::Assign(s) => self.lower_assign(s),
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
        }
    }

    fn lower_let(&mut self, stmt: &ast::LetStmt) {
        // The value is computed before declaring the slot so that
        // `mut x = x` — with an outer `x` — reads the outer one, as the checker
        // resolved it.
        let annotated = stmt.ty.as_ref().map(|a| self.ir_type_from_ref(a));

        let value = stmt.init.as_ref().map(|e| {
            let operand = match annotated {
                Some(expected) => self.lower_expr_as(e, expected),
                None => self.lower_expr(e),
            };
            (operand, e.span())
        });

        let ty = match (annotated, value) {
            (Some(ty), _) => ty,
            // Without an annotation the type is the one the initializer
            // produced. Asking the value rather than the tree is what lets a
            // lambda be inferred: its closure type only exists once lowered.
            (None, Some((operand, _))) => self.type_of_operand(operand),
            (None, None) => unreachable!("without a type there is always an initializer"),
        };

        let slot = self.declare_slot(&stmt.name.name, ty, stmt.name.span);

        if let Some((operand, _)) = value {
            self.emit_effect(InstKind::Store(slot, operand), stmt.span);
        }
    }

    fn lower_assign(&mut self, stmt: &ast::AssignStmt) {
        match &stmt.target {
            ast::AssignTarget::Name(name) => {
                let slot = self.lookup_slot(&name.name);
                let value = self.lower_expr_as(&stmt.value, self.slot_type(slot));
                self.emit_effect(InstKind::Store(slot, value), stmt.span);
            }
            ast::AssignTarget::Field(field) => {
                // The object is evaluated before the value, which is the order
                // it is written in.
                let object = self.lower_expr(&field.object);
                let (index, ty) = self.field_position(&field.object, &field.name.name);
                let value = self.lower_expr_as(&stmt.value, ty);
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
        let ast::Expr::Range(range) = &stmt.iterable else {
            return self.lower_for_in_iterable(stmt);
        };

        self.scopes.push(HashMap::new());

        let start = self.lower_expr(&range.start);
        let binding = self.declare_slot(&stmt.binding.name, IrType::Int32, stmt.binding.span);
        self.emit_effect(InstKind::Store(binding, start), stmt.span);

        // The end is evaluated once, before the loop: re-evaluating it each
        // iteration would call any function in it repeatedly.
        let end = self.lower_expr(&range.end);
        let limit = self.declare_slot("<range end>", IrType::Int32, range.end.span());
        self.emit_effect(InstKind::Store(limit, end), stmt.span);

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let current = self.emit(InstKind::Load(binding), IrType::Int32, stmt.span);
        let bound = self.emit(InstKind::Load(limit), IrType::Int32, stmt.span);
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
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let value = self.emit(InstKind::Load(binding), IrType::Int32, stmt.span);
        let one = self.emit(InstKind::ConstInt(1), IrType::Int32, stmt.span);
        let next = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: value,
                right: one,
            },
            IrType::Int32,
            stmt.span,
        );
        self.emit_effect(InstKind::Store(binding, next), stmt.span);
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
        let discriminant = self.emit(InstKind::Discriminant(held), IrType::Int32, stmt.span);
        let expected = self.emit(InstKind::ConstInt(item as i32), IrType::Int32, stmt.span);
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
        let target = self
            .loops
            .last()
            .expect("a verified program only breaks inside a loop")
            .break_to;
        self.terminate(Terminator::Jump(target));

        // Statements after a `break` are unreachable, but the block they would
        // land in still needs to exist and be terminated.
        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_continue(&mut self, _stmt: &ast::JumpStmt) {
        let target = self
            .loops
            .last()
            .expect("a verified program only continues inside a loop")
            .continue_to;
        self.terminate(Terminator::Jump(target));

        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_return(&mut self, stmt: &ast::ReturnStmt) {
        let expected = self.return_type;
        let value = stmt.value.as_ref().map(|e| self.lower_expr_as(e, expected));
        self.terminate(Terminator::Return(value));
    }

    // --- Expressions ------------------------------------------------------

    fn lower_expr(&mut self, expr: &ast::Expr) -> Operand {
        let span = expr.span();

        match expr {
            ast::Expr::Int(lit) => {
                self.emit(InstKind::ConstInt(lit.value as i32), IrType::Int32, span)
            }
            ast::Expr::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Expr::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }

            ast::Expr::Path(ident) => {
                let slot = self.lookup_slot(&ident.name);
                let ty = self.slot_type(slot);
                self.emit(InstKind::Load(slot), ty, span)
            }

            ast::Expr::Unary(e) => {
                let operand = self.lower_expr(&e.operand);
                let (op, ty) = match e.op {
                    ast::UnaryOp::Neg => (UnaryOp::Neg, IrType::Int32),
                    ast::UnaryOp::Not => (UnaryOp::Not, IrType::Boolean),
                    ast::UnaryOp::BitNot => (UnaryOp::BitNot, IrType::Int32),
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

                let held = self.lower_and_hold(&e.left, opens_blocks(&e.right));
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
                let held = self.lower_and_hold(&e.left, opens_blocks(&e.right));
                let right = self.lower_expr(&e.right);
                let left = self.reload(held, e.left.span());

                let kind = if e.op == ast::BinaryOp::Add {
                    InstKind::Concat { left, right }
                } else if self.type_of(&e.left, e.left.span()) == IrType::String {
                    InstKind::Repeat {
                        string: left,
                        count: right,
                    }
                } else {
                    // `3 * "ja"`: the same request, written the other way.
                    InstKind::Repeat {
                        string: right,
                        count: left,
                    }
                };
                self.emit(kind, IrType::String, span)
            }

            ast::Expr::Binary(e) => {
                let held = self.lower_and_hold(&e.left, opens_blocks(&e.right));
                let right = self.lower_expr(&e.right);
                let left = self.reload(held, e.left.span());
                let op = binary_op(e.op);
                let operand_type = self.type_of(&e.left, e.left.span());
                self.emit(
                    InstKind::Binary { op, left, right },
                    op.result_type(operand_type),
                    span,
                )
            }

            // Calling a closure value goes through the value, not a name.
            ast::Expr::Call(e) if self.is_closure_call(e) => {
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                let branching = e.args.iter().any(|a| opens_blocks(&a.value));
                let held = self.lower_and_hold(&e.callee, branching);

                let expected = self.module.closures[id as usize].params.clone();
                let returns = self.module.closures[id as usize].returns;
                let args = self.lower_held_args(&e.args, &expected);
                let callee = self.reload(held, e.callee.span());
                self.emit(InstKind::CallClosure { id, callee, args }, returns, span)
            }

            ast::Expr::Call(e) => {
                if let Some(operand) = self.lower_super_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_contract_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_method_call(e, span) {
                    return operand;
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
                self.emit(InstKind::Call { callee: name, args }, returns, span)
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
                self.emit(InstKind::ConstInt(value), IrType::Int32, span)
            }

            ast::Expr::Println(e) => {
                let operand = self.lower_println_argument(e, span);
                self.emit(InstKind::Println(operand), IrType::Void, span)
            }

            // The checker rejects these before lowering runs; see
            // `zirk_sema` and the tasks still open for this phase.
            ast::Expr::Lambda(e) => self.lower_lambda(e, span),

            ast::Expr::Cast(e) => self.lower_cast(e, span),

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
        let right_type = self.type_of(&expr.right, expr.right.span());
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
    /// Nothing is allocated: a closure cannot escape in this phase, so the
    /// value lives wherever its slot does. Decision D10.
    fn lower_lambda(&mut self, expr: &ast::LambdaExpr, span: Span) -> Operand {
        let info = self
            .checked
            .lambdas
            .get(&expr.span)
            .expect("the checker records every lambda it accepted");

        // The types come from the slots the captures live in, not from the
        // checker's types. A captured closure is the reason: the checker
        // identifies a closure type by its own numbering and the IR by the
        // layout it built, and only the slot knows which layout this one is.
        let capture_slots: Vec<SlotId> = info
            .captures
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
        // Zirk identifier, so it cannot collide with a user's function.
        let name = format!("lambda.{}", self.module.closures.len());
        let id = self.module.closures.len() as u32;
        self.module.closures.push(ClosureLayout {
            function: name.clone(),
            captures: capture_types.clone(),
            params: param_types.clone(),
            returns,
        });

        // The captures are read in the enclosing function, where their slots
        // live, before the body is lifted out.
        let captures: Vec<Operand> = capture_slots
            .iter()
            .map(|slot| self.emit(InstKind::Load(*slot), self.slot_type(*slot), span))
            .collect();

        let names: Vec<String> = info
            .captures
            .iter()
            .map(|c| c.name.clone())
            .chain(expr.params.iter().map(|p| p.name.name.clone()))
            .collect();
        let types: Vec<IrType> = capture_types.into_iter().chain(param_types).collect();

        let body = self.lift_lambda_body(expr, &name, &names, &types, returns);
        self.lifted.push(body);

        self.emit(
            InstKind::MakeClosure { id, captures },
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
            self.declare_pattern_types(&first.pattern, scrutinee_type);
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
                let test = self.lower_pattern_test(&arm.pattern, scrutinee, scrutinee_type, span);

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
            // A binding pattern names the scrutinee inside its arm.
            if let ast::Pattern::Binding(ident) = &arm.pattern {
                let current = self.emit(InstKind::Load(scrutinee), scrutinee_type, ident.span);
                let slot = self.declare_slot(&ident.name, scrutinee_type, ident.span);
                self.emit_effect(InstKind::Store(slot, current), ident.span);
            }
            // A variant pattern destructures its own associated fields, each
            // into the name its sub-pattern binds (roadmap task 11.4) — the
            // checker only accepts a binding or a wildcard here, so a
            // wildcard is the only other shape this sees.
            if let ast::Pattern::Variant(v) = &arm.pattern
                && !v.bindings.is_empty()
            {
                let enum_id = self.enum_id_of(&v.enum_name);
                let variant = self
                    .checked
                    .enums
                    .get(enum_id as usize)
                    .and_then(|e| e.discriminant(&v.variant.name))
                    .expect("the checker resolved this variant");
                let indices =
                    self.module.enums[enum_id as usize].variants[variant as usize].clone();
                let object = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                for (sub_pattern, index) in v.bindings.iter().zip(indices) {
                    let ast::Pattern::Binding(ident) = sub_pattern else {
                        continue;
                    };
                    let field_ty = self.module.enums[enum_id as usize].fields[index as usize].ty;
                    let value =
                        self.emit(InstKind::LoadField { object, index }, field_ty, ident.span);
                    let slot = self.declare_slot(&ident.name, field_ty, ident.span);
                    self.emit_effect(InstKind::Store(slot, value), ident.span);
                }
            }

            let value = match &arm.body {
                ast::ArmBody::Expr(e) => self.lower_expr(e),
                ast::ArmBody::Block(b) if result_type == IrType::Void => {
                    self.lower_block(b);
                    self.emit(InstKind::ConstInt(0), IrType::Int32, span)
                }
                ast::ArmBody::Block(b) => self.lower_block_value(b),
            };
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
            None => self.emit(InstKind::ConstInt(0), IrType::Int32, span),
        }
    }

    /// The comparison one pattern stands for.
    fn lower_pattern_test(
        &mut self,
        pattern: &ast::Pattern,
        scrutinee: SlotId,
        scrutinee_type: IrType,
        span: Span,
    ) -> Operand {
        let expected = match pattern {
            ast::Pattern::Int(lit) => {
                self.emit(InstKind::ConstInt(lit.value as i32), IrType::Int32, span)
            }
            ast::Pattern::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Pattern::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }
            ast::Pattern::Variant(v) => {
                let value = self.discriminant(&v.enum_name, &v.variant.name);
                self.emit(InstKind::ConstInt(value), IrType::Int32, span)
            }
            // `null` tests absence rather than a value, so it is the one
            // pattern that does not compare against anything.
            ast::Pattern::Null(_) => {
                let value = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                return self.emit(InstKind::IsNull(value), IrType::Boolean, span);
            }
            ast::Pattern::Wildcard(_) | ast::Pattern::Binding(_) => {
                unreachable!("an irrefutable pattern is not tested this way")
            }
        };

        let left = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
        // An algebraic enum's own value is discriminant plus payload, not
        // the discriminant alone (roadmap task 11.3) — a variant pattern
        // still tests only the discriminant, so it is read out first.
        let left = if let IrType::Enum(_) = scrutinee_type {
            self.emit(InstKind::Discriminant(left), IrType::Int32, span)
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
                let enum_id = self.enum_id_of(&v.enum_name);
                let Some(variant) = self
                    .checked
                    .enums
                    .get(enum_id as usize)
                    .and_then(|e| e.discriminant(&v.variant.name))
                else {
                    return;
                };
                let indices =
                    self.module.enums[enum_id as usize].variants[variant as usize].clone();
                for (sub_pattern, index) in v.bindings.iter().zip(indices) {
                    if let ast::Pattern::Binding(ident) = sub_pattern {
                        let field_ty =
                            self.module.enums[enum_id as usize].fields[index as usize].ty;
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
        let ty = self.block_value_type(&stmt.then_branch);
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
        let value = self.lower_block_value(&stmt.then_branch);
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = else_block;
        let value = match &stmt.else_branch {
            Some(ast::ElseBranch::Block(b)) => self.lower_block_value(b),
            Some(ast::ElseBranch::If(nested)) => self.lower_if_expr(nested, span),
            None => unreachable!("a verified `if` expression always has an `else`"),
        };
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

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
                self.emit(InstKind::ConstInt(discriminant), IrType::Int32, span)
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
        let result = self.declare_slot("<safe_field>", result_type, span);

        let object_ty = self.type_of(&expr.object, expr.object.span());
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

        let result_type = match returns {
            IrType::Nullable(_) => returns,
            _ => IrType::Nullable(
                Nullable::of(returns).expect("a method reachable through `?.` has a nullable form"),
            ),
        };
        let result = self.declare_slot("<safe_call>", result_type, span);

        let object_ty = self.type_of(&field.object, field.object.span());
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
        let IrType::Nullable(base) = result_type else {
            unreachable!("computed above")
        };
        let absent = self.emit(InstKind::NullValue(base), result_type, span);
        self.emit_effect(InstKind::Store(result, absent), span);
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
        let value = if matches!(returns, IrType::Nullable(_)) {
            value
        } else {
            self.emit(InstKind::Wrap { base, value }, result_type, span)
        };
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        Some(self.emit(InstKind::Load(result), result_type, span))
    }

    /// The type a member access produces.
    fn field_type_of(&self, expr: &ast::FieldExpr) -> IrType {
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
                IrType::Int32
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
        let ty = self.type_of(&expr.when_true, expr.when_true.span());
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
        // evaluated, which is what makes a ternary usable as a guard.
        self.current = then_block;
        let value = self.lower_expr(&expr.when_true);
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = else_block;
        let value = self.lower_expr(&expr.when_false);
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

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
        let one = self.emit(InstKind::ConstInt(1), IrType::Int32, span);
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
            _ => unreachable!("a verified block used as a value ends in an expression"),
        }
    }

    /// Lowers an expression whose value is discarded.
    ///
    /// `Println` and `Void` calls produce no value, so emitting a result for
    /// them would leave a value nobody reads.
    fn lower_expr_for_effect(&mut self, expr: &ast::Expr) {
        match expr {
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
                self.lower_contract_call(e, expr.span());
            }
            ast::Expr::Call(e) if self.method_of(e).is_some() => {
                self.lower_method_call(e, expr.span());
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
                let name = self.callee_name(e).to_string();
                let args = self.lower_args(e);
                self.emit_effect(InstKind::Call { callee: name, args }, expr.span());
            }
            other => {
                self.lower_expr(other);
            }
        }
    }

    /// Lowers the argument of a `println`, converting it when it is not a
    /// `String`.
    ///
    /// `ZIRK_STDLIB_SPEC.md` section 3: every printable value goes through
    /// `to_string()`. Without this the runtime would read an `Int32` as if it
    /// were a pointer.
    fn lower_println_argument(&mut self, expr: &ast::PrintlnExpr, span: Span) -> Operand {
        let operand = self.lower_expr(&expr.arg);
        let ty = self.type_of(&expr.arg, expr.arg.span());

        if ty == IrType::String {
            return operand;
        }

        self.emit(InstKind::ToString(operand), IrType::String, span)
    }

    /// Lowers the arguments of a call into the parameters they fill.
    ///
    /// Named arguments are placed by name and omitted ones take their default,
    /// so the call reaching the IR always has the declared arity. That is
    /// decision D4: neither the IR nor LLVM ever sees a named or missing
    /// argument.
    fn lower_args(&mut self, call: &ast::CallExpr) -> Vec<Operand> {
        let name = self.callee_name(call);
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
                .any(|a| opens_blocks(&a.value));
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
    fn is_closure_call(&self, call: &ast::CallExpr) -> bool {
        // A method or `super` call is not a closure call, and asking for the
        // type of its callee would ask for the type of a method — which is not
        // a value.
        if self.method_of(call).is_some()
            || self.contract_method_of(call).is_some()
            || self.safe_method_call_info(call).is_some()
            || self.variant_construction(call).is_some()
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
        self.checked
            .functions
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
            ast::Expr::Int(_) => IrType::Int32,
            ast::Expr::Bool(_) => IrType::Boolean,
            ast::Expr::Str(_) => IrType::String,
            ast::Expr::Path(ident) => self.slot_type(self.lookup_slot(&ident.name)),
            ast::Expr::Unary(e) => match e.op {
                ast::UnaryOp::Neg => IrType::Int32,
                ast::UnaryOp::Not => IrType::Boolean,
                ast::UnaryOp::BitNot => IrType::Int32,
            },
            // `??` yields its operands' shared type, not a boolean or an
            // arithmetic result: it is the one binary operator that is not an
            // operator in the IR at all.
            ast::Expr::Binary(e) if e.op == ast::BinaryOp::Coalesce => {
                let left = self.type_of(&e.left, e.left.span()).unwrapped();
                let right = self.type_of(&e.right, e.right.span());
                if matches!(right, IrType::Nullable(_)) {
                    right
                } else {
                    left
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
            ast::Expr::Call(e) if self.is_closure_call(e) => {
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                self.module.closures[id as usize].returns
            }
            ast::Expr::Call(e) if matches!(&*e.callee, ast::Expr::Super(_)) => IrType::Void,
            ast::Expr::Call(e) => {
                if let Some(method) = self.contract_method_of(e) {
                    return self.ir_type(method.returns);
                }
                if let Some(method) = self.method_of(e) {
                    return self.ir_type(method.returns);
                }
                if let Some((returns, ..)) = self.safe_method_call_info(e) {
                    return match returns {
                        IrType::Nullable(_) => returns,
                        _ => IrType::Nullable(
                            Nullable::of(returns)
                                .expect("a method reachable through `?.` has a nullable form"),
                        ),
                    };
                }
                if let Some((enum_id, _)) = self.variant_construction(e) {
                    return self.ir_type(Type::of(Base::Enum(enum_id)));
                }
                match self.construction_class_id(e) {
                    Some(id) => self.ir_type(Type::of(Base::Class(id))),
                    None => self.signature_return(&self.callee_name(e)),
                }
            }
            ast::Expr::If(e) => self.block_value_type(&e.then_branch),
            ast::Expr::Field(e) => self.field_type_of(e),
            ast::Expr::This(_) | ast::Expr::Super(_) => self.slot_type(self.lookup_slot("this")),
            ast::Expr::Ternary(e) => self.type_of(&e.when_true, e.when_true.span()),
            // Both forms yield the type of the operand they update.
            ast::Expr::Increment(e) => self.slot_type(self.lookup_slot(e.target.name())),
            ast::Expr::Match(e) => self.arm_value_type(e),
            ast::Expr::Variant(_) => IrType::Int32,
            ast::Expr::Println(_) => IrType::Void,
            // A lambda's type is the closure layout it produced, which only
            // exists once it has been lowered: the caller asks the value.
            ast::Expr::Lambda(_) | ast::Expr::Null(_) | ast::Expr::Range(_) => {
                unreachable!("the type of this expression comes from the value it produced")
            }
            ast::Expr::Cast(e) => self.ir_type_from_ref(&e.target),
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
            | (IrType::String, IrType::Int32, ast::BinaryOp::Mul)
            | (IrType::Int32, IrType::String, ast::BinaryOp::Mul)
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

/// Whether lowering this expression opens blocks of its own.
///
/// Only these constructs move the insertion point, and that is what forces an
/// earlier value to travel through a slot.
fn opens_blocks(expr: &ast::Expr) -> bool {
    match expr {
        // A ternary opens blocks for the same reason an `if` does: only one of
        // its branches runs.
        ast::Expr::If(_) | ast::Expr::Match(_) | ast::Expr::Ternary(_) => true,
        ast::Expr::Binary(e) => {
            matches!(
                e.op,
                ast::BinaryOp::Coalesce | ast::BinaryOp::And | ast::BinaryOp::Or
            ) || opens_blocks(&e.left)
                || opens_blocks(&e.right)
        }
        ast::Expr::Unary(e) => opens_blocks(&e.operand),
        ast::Expr::Call(e) => e.args.iter().any(|a| opens_blocks(&a.value)),
        ast::Expr::Println(e) => opens_blocks(&e.arg),
        _ => false,
    }
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
