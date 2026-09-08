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
    ParamInfo, TupleType, Type, TypeNames, VariantMapping, describe,
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
    // `String.chars()`/`bytes()`/`codepoints()` and `split_whitespace()`/`lines()`
    // need their concrete `List<T>` id in extern signatures even when a small
    // program only uses one of them; pre-populate the table so every id is
    // stable and known.
    for element in [
        Type::of(Base::String),
        Type::of(Base::Char),
        Type::of(Base::Int(SemaIntWidth::U8)),
        Type::of(Base::Int(SemaIntWidth::U32)),
    ] {
        if !checked.list_types.contains(&element) {
            checked.list_types.push(element);
        }
    }
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
    // A record lives in `values`, not `objects` — its
    // `ObjectLayout` entry here is an empty placeholder that is never read
    // (`IrType::Value`, not `Object`, is what a value-kind class resolves
    // to; see `ir_type`) *unless* it implements at least one contract
    // (`fase-3-value-type-contract-dispatch`, design D1): a value-to-contract
    // conversion boxes the value into a real, collector-tracked allocation,
    // and that allocation's descriptor is built by this exact same code
    // path, parameterized over the value type's own `fields`/`methods`/
    // `contracts` — sharing its id with the type's own `IrType::Value` entry
    // (`Base::Class` doc comment: the two tables index the same id space).
    // A value type that never implements a contract keeps the empty
    // placeholder, the same way a still-generic template's is.
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
            let is_unboxed_value =
                matches!(class.kind, ast::ClassKind::Record) && class.contracts.is_empty();
            if !class.type_params.is_empty() || is_unboxed_value {
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
                    .filter(|field| !field.is_static)
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
                        table[method.index] =
                            if class.kind == ast::ClassKind::Abstract || method.is_static {
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
                                // A record's own body takes
                                // `this` by value (`Self::run_method`) — it
                                // needs the box-unboxing wrapper
                                // (`box_thunk_symbol`) rather than its own
                                // symbol directly. A trait default body it
                                // inherited instead (`from_contract`) is
                                // already compiled against a generic
                                // `this: Contract` pointer receiver
                                // (`Self::run` for a contract body, further
                                // below) — exactly what `CallContract`
                                // already passes, value type or not — so
                                // that one needs no wrapper at all, the same
                                // as a class's.
                                if matches!(class.kind, ast::ClassKind::Record)
                                    && supplied.from_contract.is_none()
                                {
                                    box_thunk_symbol(&class.name, &supplied.name)
                                } else {
                                    body_symbol(checked, supplied)
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            }
        })
        .collect();

    // A record's own layout — indexed the same way `objects`
    // is, sharing `checked.classes`' id space (roadmap task 11.5); an
    // ordinary class's or a still-generic template's entry here is the
    // empty placeholder, symmetric with `objects` above.
    let mut values: Vec<ValueLayout> = checked
        .classes
        .iter()
        .map(|class| {
            if !class.type_params.is_empty() || !matches!(class.kind, ast::ClassKind::Record) {
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

    // Tuple value layouts follow all class layouts: a `Base::Tuple(id)` maps
    // to `module.values[checked.tuple_base + id]` (roadmap Phase 3b).
    for (id, tuple) in checked.tuple_types.iter().enumerate() {
        values.push(ValueLayout {
            name: format!("Tuple({})", id),
            fields: tuple
                .elements
                .iter()
                .enumerate()
                .map(|(index, &ty)| ObjectField {
                    name: format!("_{index}"),
                    ty: ir_type(ty, instance_base, enum_instance_base, checked),
                })
                .collect(),
        });
    }

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

    // `checked.weak_types` is pushed first, in the same order, for the same
    // reason `pointer_types` is just above (roadmap Phase 4e, `fase-4e-weak`,
    // design D1).
    module.weak_types = checked
        .weak_types
        .iter()
        .map(|&referent| ir_type(referent, instance_base, enum_instance_base, checked))
        .collect();

    // `checked.native_slice_types`/`checked.native_slice_mut_types` are
    // pushed first, in the same order, for the same reason `pointer_types`
    // is above (roadmap Phase 4e, `fase-4e-native-slice`, design D1).
    module.native_slice_types = checked
        .native_slice_types
        .iter()
        .map(|&element| ir_type(element, instance_base, enum_instance_base, checked))
        .collect();
    module.native_slice_mut_types = checked
        .native_slice_mut_types
        .iter()
        .map(|&element| ir_type(element, instance_base, enum_instance_base, checked))
        .collect();

    // `checked.dependent_types`/`checked.pin_types` are pushed first, in the
    // same order, for the same reason the other generic tables are (roadmap
    // Phase 4e, `phase-4e-memory`, design D1).
    module.dependent_types = checked
        .dependent_types
        .iter()
        .map(|&element| ir_type(element, instance_base, enum_instance_base, checked))
        .collect();
    module.pin_types = checked
        .pin_types
        .iter()
        .map(|&referent| ir_type(referent, instance_base, enum_instance_base, checked))
        .collect();

    // `checked.array_types`/`checked.list_types` are pushed first, in the
    // same order, for the same reason the other generic tables are.
    module.array_types = checked
        .array_types
        .iter()
        .map(|&element| ir_type(element, instance_base, enum_instance_base, checked))
        .collect();
    module.list_types = checked
        .list_types
        .iter()
        .map(|&element| ir_type(element, instance_base, enum_instance_base, checked))
        .collect();
    module.map_types = checked
        .map_types
        .iter()
        .map(|t| {
            (
                ir_type(t.key, instance_base, enum_instance_base, checked),
                ir_type(t.value, instance_base, enum_instance_base, checked),
            )
        })
        .collect();
    module.set_types = checked
        .set_types
        .iter()
        .map(|&element| ir_type(element, instance_base, enum_instance_base, checked))
        .collect();

    // `Regex.split` returns `List<String>` and `Regex.find_all` returns
    // `List<Regex.Match>`; the checker interns each element type only when
    // the method is actually called, so the extern signatures below are
    // registered conditionally on the id existing.
    let list_string_id = checked
        .list_types
        .iter()
        .position(|&t| t == Type::STRING)
        .map(|index| index as u32);
    let list_char_id = checked
        .list_types
        .iter()
        .position(|&t| t == Type::of(Base::Char))
        .map(|index| index as u32);
    let list_u8_id = checked
        .list_types
        .iter()
        .position(|&t| t == Type::of(Base::Int(SemaIntWidth::U8)))
        .map(|index| index as u32);
    let list_u32_id = checked
        .list_types
        .iter()
        .position(|&t| t == Type::of(Base::Int(SemaIntWidth::U32)))
        .map(|index| index as u32);
    let list_regex_match_id = checked
        .list_types
        .iter()
        .position(|&t| t == Type::of(Base::Class(checked.regex_match_class)))
        .map(|index| index as u32);

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

    // The `Duration` helpers `lower_duration_binary`/`lower_to_string_expr`
    // emit `InstKind::Call` for are not Zirk functions — they are `extern "C"`
    // runtime symbols resolved at link time. Registering them here, with their
    // signatures, is what lets the verifier check their calls' arity and
    // result type like any other extern's (and what puts them in codegen's
    // function table).
    let nanos = IrType::Int(IntWidth::I64);
    let f64 = IrType::Float(FloatWidth::F64);

    module.externs.extend([
        ExternFn {
            name: "zirk_regex_from_pattern".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Regex,
        },
        ExternFn {
            name: "zirk_regex_is_match".to_string(),
            params: vec![IrType::Regex, IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_regex_to_string".to_string(),
            params: vec![IrType::Regex],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_regex_replace".to_string(),
            params: vec![IrType::Regex, IrType::String, IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_regex_find".to_string(),
            params: vec![IrType::Regex, IrType::String],
            return_type: IrType::Nullable(Nullable::Object(checked.regex_match_class)),
        },
        ExternFn {
            name: "zirk_regex_match_group_pos".to_string(),
            params: vec![
                IrType::Object(checked.regex_match_class),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_regex_match_group_name".to_string(),
            params: vec![IrType::Object(checked.regex_match_class), IrType::String],
            return_type: IrType::String,
        },
        // `String` built-in methods (roadmap Phase 7, `String` ops).
        ExternFn {
            name: "zirk_str_trim".to_string(),
            params: vec![IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_contains".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_str_starts_with".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_str_ends_with".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_str_substring".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_search".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        // `s[i] = c` (roadmap Phase 7, "String write by index"):
        // (handle, byte offset, grapheme byte length, replacement) → the
        // replacement `String`. The third parameter is `Char`/`String`'s
        // shared representation (ADR-014); the extern's `String` spelling is
        // the canonical one.
        ExternFn {
            name: "zirk_str_set".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
                IrType::String,
            ],
            return_type: IrType::String,
        },
        // `s[start:end:step]` (roadmap Phase 7, `String` slicing):
        // `i64::MIN` marks a part the source left out.
        ExternFn {
            name: "zirk_str_slice".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::String,
        },
        // `s.split(sep)` (roadmap Phase 7, `String` split): a `List<String>`
        // of parts; the `List` id is element-agnostic for the runtime call.
        ExternFn {
            name: "zirk_str_split".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::List(0),
        },
        // `d.abs()`/`d.sign()` on a `Duration` (`native-type-member-surface`):
        // nanoseconds in, nanoseconds or an `i32` sign out.
        ExternFn {
            name: "zirk_duration_abs".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_sign".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_days".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_hours".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_minutes".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_seconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_milliseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_microseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_nanoseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_duration_total_weeks".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_days".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_hours".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_minutes".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_seconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_milliseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_microseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_total_nanoseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Float(FloatWidth::F64),
        },
        ExternFn {
            name: "zirk_duration_whole_weeks".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_days".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_hours".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_minutes".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_seconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_milliseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_microseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_whole_nanoseconds".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_round".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_floor".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_ceil".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_duration_truncate".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        // Civil temporal types (`date-and-time-types`): a `Date` is an
        // `i64` day count since 1970-01-01, a `Time` is an `i64` nanosecond
        // count since midnight, and a `DateTime` is epoch nanoseconds in an
        // `i128`. Validation comes first (`_is_valid` → the IR throws
        // `InvalidDateError`/`InvalidTimeError` on `false`), then the
        // component accessors and the ISO formatters.
        ExternFn {
            name: "zirk_rt_date_is_valid".to_string(),
            params: vec![
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_date_days".to_string(),
            params: vec![
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_date_year".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_month".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_day".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_today".to_string(),
            params: vec![],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_date_to_string".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_rt_time_is_valid".to_string(),
            params: vec![
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_time_nanos".to_string(),
            params: vec![
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_hour".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_time_minute".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_time_second".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_time_nanosecond".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_add_nanos".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_diff".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_now_utc".to_string(),
            params: vec![],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_now_local".to_string(),
            params: vec![],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_to_string".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_rt_datetime_new".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_rt_datetime_days".to_string(),
            params: vec![IrType::Int(IntWidth::I128)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_datetime_nanos".to_string(),
            params: vec![IrType::Int(IntWidth::I128)],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_datetime_now_utc".to_string(),
            params: vec![],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_rt_datetime_now_local".to_string(),
            params: vec![],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_rt_datetime_to_string".to_string(),
            params: vec![IrType::Int(IntWidth::I128)],
            return_type: IrType::String,
        },
        // Calendrical projections (`temporal-rich-api`): pure functions of
        // the day count; `DateTime` delegates through `datetime_days`.
        ExternFn {
            name: "zirk_rt_date_day_of_week".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_day_of_year".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_week_of_year".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_quarter".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_days_in_month".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_days_in_year".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_rt_date_is_leap".to_string(),
            params: vec![IrType::Int(IntWidth::I64)],
            return_type: IrType::Boolean,
        },
        // `start_of`/`end_of` — the `ok` probe validates the unit string,
        // the `value` entrypoint computes the boundary on the passing
        // branch (the `Result` halves the IR builds around them).
        ExternFn {
            name: "zirk_rt_date_start_of_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_date_start_of_value".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_date_end_of_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_date_end_of_value".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_start_of_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_time_start_of_value".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_end_of_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_time_end_of_value".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_datetime_start_of_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_datetime_start_of_value".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::String],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_rt_datetime_end_of_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_datetime_end_of_value".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::String],
            return_type: IrType::Int(IntWidth::I128),
        },
        // Strict ISO 8601 parsing (`ok`/`value` split like `IntN.parse`).
        ExternFn {
            name: "zirk_rt_date_parse_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_date_parse_value".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_time_parse_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_time_parse_value".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_rt_datetime_parse_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_datetime_parse_value".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Int(IntWidth::I128),
        },
        // `format(pattern)` — never fails; unknown text is literal.
        ExternFn {
            name: "zirk_rt_date_format".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_rt_time_format".to_string(),
            params: vec![IrType::Int(IntWidth::I64), IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_rt_datetime_format".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::String],
            return_type: IrType::String,
        },
        // `Range<T>` (roadmap Phase 7): every part travels as an `i64` —
        // a `Duration` is nanoseconds, an `Int32` sign-extends — and the
        // handle itself is element-agnostic, so the same seven entry points
        // serve every `T`.
        ExternFn {
            name: "zirk_range_new".to_string(),
            params: vec![
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::Range,
        },
        ExternFn {
            name: "zirk_range_start".to_string(),
            params: vec![IrType::Range],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_range_end".to_string(),
            params: vec![IrType::Range],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_range_step".to_string(),
            params: vec![IrType::Range],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_range_inclusive".to_string(),
            params: vec![IrType::Range],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_range_reverse".to_string(),
            params: vec![IrType::Range],
            return_type: IrType::Range,
        },
        ExternFn {
            name: "zirk_range_slice".to_string(),
            params: vec![
                IrType::Range,
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I64),
            ],
            return_type: IrType::Range,
        },
        // `Char` classification and normalization (roadmap Phase 7).
        ExternFn {
            name: "zirk_char_is_uppercase".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_lowercase".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_digit".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_letter".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_whitespace".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_to_uppercase".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_char_to_lowercase".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::String,
        },
        // `native-type-member-surface`: the rest of the `Char` surface —
        // metadata (`i32` counts), the ASCII query, and the remaining
        // classification methods.
        ExternFn {
            name: "zirk_char_byte_length".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_char_codepoint_count".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_char_ascii_code".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_char_is_ascii".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_alphabetic".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_numeric".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_is_alphanumeric".to_string(),
            params: vec![IrType::Char],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_char_normalize".to_string(),
            params: vec![IrType::Char, IrType::String],
            return_type: IrType::String,
        },
        // `native-type-member-surface`: the `String` metadata, search,
        // transformation, normalization and view surface.
        ExternFn {
            name: "zirk_str_length".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_str_byte_length".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_str_is_empty".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_str_find".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::Int(IntWidth::I64),
        },
        ExternFn {
            name: "zirk_str_replace".to_string(),
            params: vec![IrType::String, IrType::String, IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_trim_start".to_string(),
            params: vec![IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_trim_end".to_string(),
            params: vec![IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_to_lowercase".to_string(),
            params: vec![IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_to_uppercase".to_string(),
            params: vec![IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_normalize".to_string(),
            params: vec![IrType::String, IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_clone".to_string(),
            params: vec![IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_str_split_whitespace".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_string_id.unwrap_or(0)),
        },
        ExternFn {
            name: "zirk_str_lines".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_string_id.unwrap_or(0)),
        },
        ExternFn {
            name: "zirk_str_chars".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_char_id.unwrap_or(0)),
        },
        ExternFn {
            name: "zirk_str_bytes".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_u8_id.unwrap_or(0)),
        },
        ExternFn {
            name: "zirk_str_codepoints".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_u32_id.unwrap_or(0)),
        },
        ExternFn {
            name: "zirk_char_bytes".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_u8_id.unwrap_or(0)),
        },
        ExternFn {
            name: "zirk_char_codepoints".to_string(),
            params: vec![IrType::String],
            return_type: IrType::List(list_u32_id.unwrap_or(0)),
        },
        // `native-type-member-surface`: integer helpers. Values travel as
        // `i128`; `bits` and `signed` carry the receiver's width semantics.
        ExternFn {
            name: "zirk_int_abs".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_sign".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_int_min".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_max".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_clamp".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_is_zero".to_string(),
            params: vec![IrType::Int(IntWidth::I128)],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_is_even".to_string(),
            params: vec![IrType::Int(IntWidth::I128)],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_is_odd".to_string(),
            params: vec![IrType::Int(IntWidth::I128)],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_bit_count".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::Int(IntWidth::I32)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_int_leading_zeros".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::Int(IntWidth::I32)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_int_trailing_zeros".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::Int(IntWidth::I32)],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_int_rotate_left".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_rotate_right".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I64),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_wrapping_add".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_wrapping_sub".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_wrapping_mul".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_saturating_add".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_saturating_sub".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_saturating_mul".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_to_string_radix".to_string(),
            params: vec![IrType::Int(IntWidth::I128), IrType::Int(IntWidth::I32)],
            return_type: IrType::String,
        },
        // `checked_*` and `IntN.parse` split into an `ok` status and a
        // `value` call — lowering builds the `Result` from the pair.
        ExternFn {
            name: "zirk_int_checked_add_ok".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_checked_add_value".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_checked_sub_ok".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_checked_sub_value".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_checked_mul_ok".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_checked_mul_value".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_checked_div_ok".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_checked_div_value".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_checked_rem_ok".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_checked_rem_value".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_checked_pow_ok".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_checked_pow_value".to_string(),
            params: vec![
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I128),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_parse_ok".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_parse_value".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        ExternFn {
            name: "zirk_int_parse_radix_ok".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_int_parse_radix_value".to_string(),
            params: vec![
                IrType::String,
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
                IrType::Int(IntWidth::I32),
            ],
            return_type: IrType::Int(IntWidth::I128),
        },
        // Float helpers — `f64` is the widest helper signature; lowering
        // `FloatCast`s around the call.
        ExternFn {
            name: "zirk_float_abs".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_sign".to_string(),
            params: vec![f64],
            return_type: IrType::Int(IntWidth::I32),
        },
        ExternFn {
            name: "zirk_float_min".to_string(),
            params: vec![f64, f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_max".to_string(),
            params: vec![f64, f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_clamp".to_string(),
            params: vec![f64, f64, f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_is_zero".to_string(),
            params: vec![f64],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_float_floor".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_ceil".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_round".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_truncate".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_fraction".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_is_finite".to_string(),
            params: vec![f64],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_float_is_infinite".to_string(),
            params: vec![f64],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_float_is_negative".to_string(),
            params: vec![f64],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_float_pow".to_string(),
            params: vec![f64, IrType::Int(IntWidth::I64)],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_powf".to_string(),
            params: vec![f64, f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_sqrt".to_string(),
            params: vec![f64],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_parse_ok".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_float_parse_value".to_string(),
            params: vec![IrType::String],
            return_type: f64,
        },
        ExternFn {
            name: "zirk_float_format".to_string(),
            params: vec![f64, IrType::String],
            return_type: IrType::String,
        },
        // `Regex.parse` — the `ok` half; the handle comes from the existing
        // `zirk_regex_from_pattern`.
        ExternFn {
            name: "zirk_regex_is_valid_pattern".to_string(),
            params: vec![IrType::String],
            return_type: IrType::Boolean,
        },
        ExternFn {
            name: "zirk_rt_duration_to_string".to_string(),
            params: vec![nanos],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_rt_duration_to_string".to_string(),
            params: vec![nanos],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_duration_to_iso_string".to_string(),
            params: vec![nanos],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_duration_format".to_string(),
            params: vec![nanos, IrType::String],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_duration_humanize".to_string(),
            params: vec![nanos, IrType::String, IrType::Int(IntWidth::I32)],
            return_type: IrType::String,
        },
        ExternFn {
            name: "zirk_rt_duration_mul_f64".to_string(),
            params: vec![nanos, f64],
            return_type: nanos,
        },
        ExternFn {
            name: "zirk_rt_duration_div_f64".to_string(),
            params: vec![nanos, f64],
            return_type: nanos,
        },
        ExternFn {
            name: "zirk_rt_duration_div_duration".to_string(),
            params: vec![nanos, nanos],
            return_type: f64,
        },
    ]);

    // Exact base-ten `Float` runtime helpers. The IR models a `Float` value
    // as by-value `IrType::Decimal`; codegen lowers each of these to the
    // by-pointer C ABI (`{ i128, i8 }` out-parameter), the same way it does
    // for `Int128`.
    {
        let dec = IrType::Decimal;
        let i32 = IrType::Int(IntWidth::I32);
        module.externs.extend([
            decimal_binop_extern("zirk_rt_decimal_add", dec),
            decimal_binop_extern("zirk_rt_decimal_sub", dec),
            decimal_binop_extern("zirk_rt_decimal_mul", dec),
            decimal_binop_extern("zirk_rt_decimal_div", dec),
            decimal_binop_extern("zirk_rt_decimal_rem", dec),
            decimal_binop_extern("zirk_rt_decimal_pow", dec),
            decimal_binop_extern("zirk_rt_decimal_min", dec),
            decimal_binop_extern("zirk_rt_decimal_max", dec),
            ExternFn {
                name: "zirk_rt_decimal_pow_i".to_string(),
                params: vec![dec, IrType::Int(IntWidth::I64)],
                return_type: dec,
            },
            // `zirk_rt_decimal_div_ex(a, b, mode, places)`.
            ExternFn {
                name: "zirk_rt_decimal_div_ex".to_string(),
                params: vec![dec, dec, i32, i32],
                return_type: dec,
            },
            // `zirk_rt_decimal_round(value, places, mode)`.
            ExternFn {
                name: "zirk_rt_decimal_round".to_string(),
                params: vec![dec, i32, i32],
                return_type: dec,
            },
            ExternFn {
                name: "zirk_rt_decimal_clamp".to_string(),
                params: vec![dec, dec, dec],
                return_type: dec,
            },
            decimal_unop_extern("zirk_rt_decimal_neg", dec, dec),
            decimal_unop_extern("zirk_rt_decimal_abs", dec, dec),
            decimal_unop_extern("zirk_rt_decimal_sqrt", dec, dec),
            decimal_unop_extern("zirk_rt_decimal_floor", dec, dec),
            decimal_unop_extern("zirk_rt_decimal_ceil", dec, dec),
            decimal_unop_extern("zirk_rt_decimal_truncate", dec, dec),
            decimal_unop_extern("zirk_rt_decimal_fraction", dec, dec),
            decimal_binop_extern("zirk_rt_decimal_cmp", i32),
            decimal_unop_extern("zirk_rt_decimal_sign", dec, i32),
            decimal_unop_extern("zirk_rt_decimal_scale", dec, i32),
            decimal_unop_extern("zirk_rt_decimal_is_zero", dec, IrType::Boolean),
            decimal_unop_extern("zirk_rt_decimal_is_negative", dec, IrType::Boolean),
            decimal_unop_extern("zirk_rt_decimal_is_integer", dec, IrType::Boolean),
            decimal_unop_extern("zirk_str_from_decimal", dec, IrType::String),
            decimal_unop_extern("zirk_rt_decimal_to_f64", dec, f64),
            ExternFn {
                name: "zirk_rt_decimal_from_i128".to_string(),
                params: vec![IrType::Int(IntWidth::I128)],
                return_type: dec,
            },
            decimal_unop_extern(
                "zirk_rt_decimal_to_i128_checked",
                dec,
                IrType::Int(IntWidth::I128),
            ),
            ExternFn {
                name: "zirk_rt_decimal_from_f64".to_string(),
                params: vec![f64],
                return_type: dec,
            },
            ExternFn {
                name: "zirk_rt_decimal_from_literal".to_string(),
                params: vec![IrType::String],
                return_type: dec,
            },
            decimal_unop_extern("zirk_rt_decimal_parse_ok", IrType::String, IrType::Boolean),
            decimal_unop_extern("zirk_rt_decimal_parse_value", IrType::String, dec),
        ]);
    }

    // `Regex.split`/`Regex.find_all` return `List<T>`s whose element types
    // the checker only interns when the methods are called; register the
    // externs only when the id exists.
    if let Some(id) = list_string_id {
        module.externs.push(ExternFn {
            name: "zirk_regex_split".to_string(),
            params: vec![IrType::Regex, IrType::String],
            return_type: IrType::List(id),
        });
    }
    if let Some(id) = list_regex_match_id {
        module.externs.push(ExternFn {
            name: "zirk_regex_find_all".to_string(),
            params: vec![IrType::Regex, IrType::String],
            return_type: IrType::List(id),
        });
    }

    // One entry per `checked.fn_types` id, pushed first and in order so
    // `Base::Function(id)` and `IrType::Callable(id)` share the same number
    // (`Self::ir_type`'s own `Base::Function` arm relies on this).
    //
    // Most ids get their real content right here: the uniform, capture-less
    // `ClosureLayout` design D12 gives a named function reference or a
    // capture-less lambda — `{function pointer, null capture block}`, nothing
    // else — shared by every value of that shape (`Self::lower_expr`'s
    // `ast::Expr::Path` arm, `Self::lower_lambda`'s capture-less branch).
    // `function` is left empty: nothing calls through a canonical layout's
    // *own* stored target — `InstKind::MakeCallable` carries the target it
    // embeds explicitly, precisely so many differently-targeted values
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
    // will need. Nested and local classes lower here too: `class_decls`
    // resolves each one back to its `ClassDecl`.
    let class_decls = class_decls(program, checked);
    for (&id, &class) in &class_decls {
        // The template itself is never lowered — see the comment on its
        // (empty) `ObjectLayout` above. Each specialization below lowers
        // this same declaration's AST again, once per instantiation, under
        // its own id: the type-level substitution already happened when
        // `specialize_class` built its `ClassType`, so what changes here is
        // only which class id `this`/a field/a method resolves against.
        if !checked.classes[id as usize].type_params.is_empty() {
            continue;
        }

        lower_class_body(
            &mut module,
            checked,
            &declarations,
            class,
            id,
            instance_base,
            enum_instance_base,
        );
    }

    for (index, instance) in checked.generic_instances.iter().enumerate() {
        let specialized_id = instance_base + index as u32;
        let original = *class_decls
            .get(&instance.class)
            .expect("a checked class is declared in the program");
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

    // A boxed value's contract-table wrapper (`fase-3-value-type-contract-
    // dispatch`, design D1/D2, `box_thunk_symbol`'s own doc comment): one
    // per own-body method a record supplies to satisfy a
    // contract, unboxing `this` and forwarding into the real by-value body.
    // Skipped for a still-generic template the same way its own methods
    // are, and for a method inherited from a trait's default body
    // (`from_contract`), which already takes a generic `this: Contract`
    // pointer receiver and needs no wrapper.
    for (id, class) in checked.classes.iter().enumerate() {
        let id = id as u32;
        if !class.type_params.is_empty() || !matches!(class.kind, ast::ClassKind::Record) {
            continue;
        }
        // Two contracts the same class implements may share a method name
        // (both requiring `describe()`, say) — deduplicated by the symbol
        // the thunk would get, so the same one is never built twice.
        let mut built = std::collections::HashSet::new();
        for &contract in &class.contracts {
            for required in &checked.contracts[contract as usize].methods {
                let supplied = class
                    .method(&required.name)
                    .expect("the checker verified conformance");
                if supplied.from_contract.is_some() {
                    continue;
                }
                if !built.insert(supplied.name.clone()) {
                    continue;
                }
                let thunk = FunctionLowering::new(
                    &mut module,
                    checked,
                    &declarations,
                    instance_base,
                    enum_instance_base,
                )
                .build_box_thunk(id, supplied);
                module.functions.push(thunk);
            }
        }
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

/// Builds the method bodies of every native failure class by hand
/// (`fase-4d-runtimeerror`, design D8): `message`/`code`/`cause`/
/// `stack_trace` for `DivisionByZeroError`, `InvalidShiftError`,
/// `InvalidRepeatError`, `FloatNanError`, and (roadmap Phase 4e,
/// `fase-4e-native-slice`) `IndexOutOfBoundsError`/`NativeError`. None of
/// them has a `program.classes` entry —
/// `Checker::register_native_exception_hierarchy` injects them the same
/// "table directly" way `Error`/`Throwable`/`RuntimeError`/`StackTrace`
/// already are — so nothing in the ordinary per-class lowering loop below
/// ever reaches them; this is their only source of a real body.
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
        (native.arithmetic_overflow, "E_ARITHMETIC_OVERFLOW"),
        (native.invalid_cast, "E_INVALID_CAST"),
        // Roadmap Phase 4e, `fase-4e-native-slice`: two more native failure
        // classes, built through the exact same
        // `Checker::register_native_failure` closure and needing the same
        // hand-built bodies as the original four above.
        (native.index_out_of_bounds, "E_INDEX_OUT_OF_BOUNDS"),
        (native.native_error, "E_NATIVE_ERROR"),
        // Roadmap Phase 7: `InvalidStepError` (a range or slice whose step
        // is `0`), registered and built through the same closure.
        (native.invalid_step, "E_INVALID_STEP"),
        // `native-type-member-surface`: the `Result`-carried errors of
        // `parse`/`checked_*`/`Regex.parse`.
        (native.parse_error, "E_PARSE"),
        (native.overflow_error, "E_OVERFLOW"),
        (native.regex_error, "E_REGEX"),
        // `enum-static-members`: `LookupError`, the `Error` half of the
        // `Result` `EnumType.from_name`/`EnumType.from_value` produce.
        (native.lookup_error, "E_LOOKUP"),
        // `date-and-time-types`: the validation errors the `Date`/`Time`
        // constructors throw.
        (native.invalid_date, "E_INVALID_DATE"),
        (native.invalid_time, "E_INVALID_TIME"),
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
    // `static` fields lower to module-level getter functions, so they are
    // available without an instance.
    let class_info = &checked.classes[id as usize];
    for (field, info) in class.fields.iter().filter(|f| f.is_static).filter_map(|f| {
        class_info
            .fields
            .iter()
            .find(|i| i.name == f.name.name)
            .map(|i| (f, i))
    }) {
        let lowering = FunctionLowering::new(
            module,
            checked,
            declarations,
            instance_base,
            enum_instance_base,
        );
        let (lowered, lifted) = lowering.run_static_field(&class_info.name, field, info);
        module.functions.push(lowered);
        module.functions.extend(lifted);
    }

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

/// The AST declaration a checked class id names.
///
/// A top-level class is looked up by name; a nested or local class is
/// registered under a qualified (`Outer.Inner`) or mangled
/// (`name$local$id`) name, so the lookup matches on the declaration's
/// name *span* — unique per declaration — instead.
fn class_decls<'a>(
    program: &'a ast::Program,
    checked: &CheckedProgram,
) -> HashMap<u32, &'a ast::ClassDecl> {
    fn register<'a>(
        decl: &'a ast::ClassDecl,
        checked: &CheckedProgram,
        out: &mut HashMap<u32, &'a ast::ClassDecl>,
    ) {
        if let Some(id) = checked
            .classes
            .iter()
            .position(|c| c.span == decl.name.span)
        {
            out.insert(id as u32, decl);
        }
        for nested in &decl.nested {
            register(nested, checked, out);
        }
        for constructor in &decl.constructors {
            walk_block(&constructor.body, checked, out);
        }
        for method in &decl.methods {
            if let Some(body) = &method.body {
                walk_block(body, checked, out);
            }
        }
    }

    fn walk_block<'a>(
        block: &'a ast::Block,
        checked: &CheckedProgram,
        out: &mut HashMap<u32, &'a ast::ClassDecl>,
    ) {
        for stmt in &block.statements {
            walk_stmt(stmt, checked, out);
        }
    }

    fn walk_stmt<'a>(
        stmt: &'a ast::Stmt,
        checked: &CheckedProgram,
        out: &mut HashMap<u32, &'a ast::ClassDecl>,
    ) {
        use ast::Stmt;
        match stmt {
            Stmt::LocalClass(decl) => register(decl, checked, out),
            Stmt::If(s) => {
                walk_block(&s.then_branch, checked, out);
                let mut next = s.else_branch.as_ref();
                while let Some(branch) = next {
                    match branch {
                        ast::ElseBranch::Block(b) => {
                            walk_block(b, checked, out);
                            next = None;
                        }
                        ast::ElseBranch::If(inner) => {
                            walk_block(&inner.then_branch, checked, out);
                            next = inner.else_branch.as_ref();
                        }
                    }
                }
            }
            Stmt::Loop(s) => walk_block(&s.body, checked, out),
            Stmt::ForIn(s) => walk_block(&s.body, checked, out),
            Stmt::Block(b) => walk_block(b, checked, out),
            Stmt::Try(s) => {
                walk_block(&s.body, checked, out);
                for catch in &s.catches {
                    walk_block(&catch.body, checked, out);
                }
                if let Some(finally) = &s.finally {
                    walk_block(finally, checked, out);
                }
            }
            Stmt::Unsafe(s) => walk_block(&s.body, checked, out),
            Stmt::Commit(s) => walk_block(&s.body, checked, out),
            _ => {}
        }
    }

    let mut out = HashMap::new();
    for decl in &program.classes {
        register(decl, checked, &mut out);
    }
    for function in &program.functions {
        walk_block(&function.body, checked, &mut out);
    }
    out
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
                from_contract: m.from_contract,
                overridden: false,
                ..m.clone()
            })
            .collect(),
        contracts: original.contracts.clone(),
        abstract_bases: original.abstract_bases.clone(),
        contract_instances: Vec::new(),
        type_params: Vec::new(),
        shared: original.shared,
        is_final: original.is_final,
        enclosing: original.enclosing,
        nested: original.nested.clone(),
        span: original.span,
    }
}

/// Replaces `T` in `ty` per `subst`, recursing into a nested
/// `Base::EnumInstance`/`Base::ContractInstance`/`Base::Instance`'s own type
/// arguments at any depth — the `specialize_enum`-side counterpart of the
/// checker's `Checker::substitute_type` (`zirk-sema/src/checker.rs`).
///
/// Unlike the checker's version this cannot *intern* a new instance —
/// `checked` is a finished, immutable table by the time lowering runs — so a
/// substituted nested instantiation is looked up by structural equality
/// instead. It is always found: whatever concrete nested instantiation a
/// verified program's substitution produces (`Box<T>` becoming `Box<Int32>`
/// inside `Wrapper<T>`'s own payload, `fase-3-generic-substitution-recursion`)
/// was already interned during checking by the very fix
/// (`Checker::substitute`'s own recursive case) that lets such a program
/// pass the checker at all — a lookup miss here would mean the checker
/// accepted a combination it never actually recorded, which is a checker bug
/// to fix there, not a case to paper over here by leaving `ty` unsubstituted
/// (that is exactly the shallow bug this function replaces: an unsubstituted
/// nested instantiation silently pointed lowering at the wrong specialized
/// layout instead of failing loudly).
fn substitute_generic_type(checked: &CheckedProgram, ty: Type, subst: &[(u32, Type)]) -> Type {
    if let Base::Param(id) = ty.base
        && let Some(&(_, replacement)) = subst.iter().find(|(pid, _)| *pid == id)
    {
        return if ty.nullable {
            replacement.as_nullable()
        } else {
            replacement
        };
    }

    let base = match ty.base {
        Base::ContractInstance(inst_id) => {
            let instance = checked.contract_instances[inst_id as usize].clone();
            let args: Vec<Type> = instance
                .args
                .iter()
                .map(|&a| substitute_generic_type(checked, a, subst))
                .collect();
            let found = checked
                .contract_instances
                .iter()
                .position(|i| i.contract == instance.contract && i.args == args)
                .unwrap_or_else(|| {
                    panic!(
                        "a verified program only substitutes into a contract instantiation the checker already interned"
                    )
                });
            Base::ContractInstance(found as u32)
        }
        Base::EnumInstance(inst_id) => {
            let instance = checked.enum_instances[inst_id as usize].clone();
            let args: Vec<Type> = instance
                .args
                .iter()
                .map(|&a| substitute_generic_type(checked, a, subst))
                .collect();
            let found = checked
                .enum_instances
                .iter()
                .position(|i| i.enum_id == instance.enum_id && i.args == args)
                .unwrap_or_else(|| {
                    panic!(
                        "a verified program only substitutes into an enum instantiation the checker already interned"
                    )
                });
            Base::EnumInstance(found as u32)
        }
        Base::Instance(inst_id) => {
            let instance = checked.generic_instances[inst_id as usize].clone();
            let args: Vec<Type> = instance
                .args
                .iter()
                .map(|&a| substitute_generic_type(checked, a, subst))
                .collect();
            let found = checked
                .generic_instances
                .iter()
                .position(|i| i.class == instance.class && i.args == args)
                .unwrap_or_else(|| {
                    panic!(
                        "a verified program only substitutes into a class instantiation the checker already interned"
                    )
                });
            Base::Instance(found as u32)
        }
        _ => return ty,
    };

    Type {
        base,
        nullable: ty.nullable,
    }
}

/// One generic enum's own `T` replaced by one instantiation's concrete
/// arguments (roadmap task 13.5) — the enum equivalent of
/// [`specialize_class`], for the same reason: `Iteration<T>`'s payload is
/// inline, so its representation genuinely depends on `T` the way a
/// contract's dispatch table never does.
///
/// Recurses into a variant payload that nests `T` inside another generic
/// instantiation (`Bar<Baz<T>>`, `fase-3-generic-substitution-recursion`) via
/// [`substitute_generic_type`] — unlike [`specialize_class`], nothing gates a
/// generic enum's own lowering to the directly-named-`T` case, so this one
/// has always needed to handle the nested shape once the checker itself
/// stopped rejecting it.
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
                        ty: substitute_generic_type(checked, f.ty, &subst),
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

/// The name a `static fn` is emitted under.
pub fn static_method_symbol(class: &str, method: &str) -> String {
    format!("{class}$static${method}")
}

/// The name a `static` field's getter is emitted under.
pub fn static_field_symbol(class: &str, field: &str) -> String {
    format!("{class}$field${field}")
}

/// The name a trait's own default body is emitted under.
///
/// It belongs to the contract, not to any class that adopts it: one body,
/// however many classes reuse it.
pub fn contract_method_symbol(contract: &str, method: &str) -> String {
    format!("{contract}$default${method}")
}

/// The name a boxed value's contract-table entry for one of its own methods
/// is emitted under (`fase-3-value-type-contract-dispatch`, design D1/D2).
///
/// A `record`'s own method (`Self::method_symbol`) receives
/// `this` by value (roadmap task 11.5) — `CallContract`'s existing dispatch
/// always passes the receiver as a pointer (design D2's own premise: it
/// "cannot distinguish" a boxed value's descriptor from a class's), so a
/// contract table cannot point straight at that symbol the way a class's own
/// can. This name is a small wrapper instead: it takes the box's pointer,
/// rebuilds the inline value from its fields (the box's own layout is
/// identical to the value's, `Self::lower_box_value`'s own doc comment), and
/// forwards into the real by-value method — the one, single place this
/// representation gap is bridged, so `CallContract` itself still needs zero
/// changes.
pub fn box_thunk_symbol(class: &str, method: &str) -> String {
    format!("{class}$boxed${method}")
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

const fn sema_int_width(width: IntWidth) -> SemaIntWidth {
    match width {
        IntWidth::I8 => SemaIntWidth::I8,
        IntWidth::I16 => SemaIntWidth::I16,
        IntWidth::I32 => SemaIntWidth::I32,
        IntWidth::I64 => SemaIntWidth::I64,
        IntWidth::I128 => SemaIntWidth::I128,
        IntWidth::U8 => SemaIntWidth::U8,
        IntWidth::U16 => SemaIntWidth::U16,
        IntWidth::U32 => SemaIntWidth::U32,
        IntWidth::U64 => SemaIntWidth::U64,
        IntWidth::U128 => SemaIntWidth::U128,
    }
}

/// A float literal's width, from its optional suffix — mirrors the checker's
/// own `check_float_literal`, since lowering re-derives a literal's type from
/// the tree rather than re-checking it.
/// `fn(Decimal, Decimal) -> ret` — most `zirk_rt_decimal_*` helpers.
fn decimal_binop_extern(name: &str, ret: IrType) -> ExternFn {
    ExternFn {
        name: name.to_string(),
        params: vec![IrType::Decimal, IrType::Decimal],
        return_type: ret,
    }
}

/// `fn(arg) -> ret`.
fn decimal_unop_extern(name: &str, arg: IrType, ret: IrType) -> ExternFn {
    ExternFn {
        name: name.to_string(),
        params: vec![arg],
        return_type: ret,
    }
}

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

const fn sema_float_width(width: FloatWidth) -> SemaFloatWidth {
    match width {
        FloatWidth::F16 => SemaFloatWidth::F16,
        FloatWidth::F32 => SemaFloatWidth::F32,
        FloatWidth::F64 => SemaFloatWidth::F64,
        FloatWidth::F128 => SemaFloatWidth::F128,
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
        Base::Decimal => IrType::Decimal,
        Base::Boolean => IrType::Boolean,
        Base::String => IrType::String,
        Base::Duration => IrType::Int(IntWidth::I64),
        // The civil temporal types lower to plain integers: a `Date` is a
        // day count since 1970-01-01, a `Time` is nanoseconds since
        // midnight, and a `DateTime` is epoch nanoseconds in an `i128`
        // (days·86_400e9 + nanos — the day's nanos alone exceed an `i64`
        // timescale once the day count joins in).
        Base::Date => IrType::Int(IntWidth::I64),
        Base::Time => IrType::Int(IntWidth::I64),
        Base::DateTime => IrType::Int(IntWidth::I128),
        Base::Regex => IrType::Regex,
        Base::Char => IrType::Char,
        // A traditional enum — none of its variants carry data — is exactly
        // its discriminant. One with at least one algebraic variant gets a
        // representation of its own (roadmap task 11.3).
        Base::Enum(id) if enum_has_payload(checked, id) => IrType::Enum(id),
        Base::Enum(_) => IrType::Int(IntWidth::I32),
        // A record is a value, not a reference: neither has
        // identity (roadmap task 11.5). An ordinary class is reached through
        // its address, which is its identity.
        Base::Class(id) if matches!(checked.classes[id as usize].kind, ast::ClassKind::Record) => {
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
        Base::Function(id) => IrType::Callable(id),

        // `checked.pointer_types` and `module.pointer_types` are populated in
        // the same order, once, before any function is lowered (`Self::lower`,
        // right after `module` is built) — so a `Base::Pointer` id and the
        // `IrType::Pointer` id it maps to are always the same number, the
        // same "pushed first, in order" trick `checked.fn_types`/
        // `module.closures` already share (roadmap Phase 4e, design D1).
        Base::Pointer(id) => IrType::Pointer(id),

        // `checked.weak_types` and `module.weak_types` are populated the
        // same way and for the same reason `pointer_types` is, just above
        // (roadmap Phase 4e, `fase-4e-weak`, design D1).
        Base::Weak(id) => IrType::Weak(id),

        // `checked.native_slice_types`/`checked.native_slice_mut_types` and
        // `module.native_slice_types`/`module.native_slice_mut_types` are
        // populated the same way and for the same reason `pointer_types` is,
        // just above (roadmap Phase 4e, `fase-4e-native-slice`, design D1).
        Base::NativeSlice(id) => IrType::NativeSlice(id),
        Base::NativeSliceMut(id) => IrType::NativeSliceMut(id),

        // `checked.dependent_types`/`checked.pin_types` and
        // `module.dependent_types`/`module.pin_types` are populated the same
        // way, for the same reason, just above (roadmap Phase 4e,
        // `phase-4e-memory`, design D1).
        Base::Dependent(id) => IrType::Dependent(id),
        Base::Pin(id) => IrType::Pin(id),

        Base::Array(id) => IrType::Array(id),
        Base::List(id) => IrType::List(id),
        // `Map<K, V>` and `Set<T>` (roadmap Phase 7) — runtime handles.
        // The `module.map_types` and `module.set_types` are populated in the
        // same order as the checker tables, so the id is preserved.
        Base::Map(id) => IrType::Map(id),
        Base::Set(id) => IrType::Set(id),
        // `Range<T>` (roadmap Phase 7): the runtime handle its constructor
        // extern `zirk_range_new` produces — the element type lives only in
        // `checked.range_types`, the handle itself is element-agnostic.
        Base::Range(_) => IrType::Range,

        Base::Tuple(id) => IrType::Value(checked.tuple_base + id),
        Base::Unknown | Base::Null | Base::Param(_) | Base::Union(_) => {
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
    /// Active `unsafe { ... }` frames, innermost last (roadmap Phase 4e,
    /// `fase-4e-unsafe-journal`, design D1/D4) — what a write inside an
    /// active block journals against, and what a propagating exception
    /// rolls back, interleaved with `try_stack` by `pushed_at` (see
    /// `Self::lower_pending_exception_dispatch`).
    unsafe_stack: Vec<UnsafeFrame>,
    /// Monotonic counter handed out to every `TryFrame`/`UnsafeFrame` when
    /// it is pushed (design D2 of `fase-4e-unsafe-journal`) — the two
    /// stacks are tracked independently, but a propagating exception must
    /// process them in true lexical nesting order; comparing this sequence
    /// number is what reconstructs that order without merging their
    /// representations.
    next_scope_seq: u64,
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
    /// The class whose member body is being lowered, if any — what `this`
    /// and, for an `inner class`, the hidden `outer` reference resolve
    /// against.
    current_class: Option<u32>,
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
    /// Push order relative to `unsafe_stack` frames (`FunctionLowering::next_scope_seq`,
    /// `fase-4e-unsafe-journal` design D2) — see
    /// `FunctionLowering::lower_pending_exception_dispatch`.
    pushed_at: u64,
}

/// One active `unsafe { ... }` block's own journal, as
/// [`FunctionLowering::unsafe_stack`] tracks it (roadmap Phase 4e,
/// `fase-4e-unsafe-journal`, design D1/D4).
#[derive(Clone)]
struct UnsafeFrame {
    /// The `<journal>` slot holding this block's own `*mut Journal` handle
    /// — an ordinary local SSA pointer value (design D4), reloaded from this
    /// slot wherever it is needed since a value does not cross blocks
    /// (ADR-007) and journaling happens at scattered points inside the
    /// block's own body.
    journal_slot: SlotId,
    /// Every slot id below this boundary (`FunctionLowering::slots.len()`
    /// at this frame's own `JournalBegin`) was declared before the block —
    /// a `Store` to one of them writes to storage declared outside the
    /// block and must be journaled (design D1); a slot at or above the
    /// boundary was declared inside the block itself and is exempt.
    slot_boundary: u32,
    /// Set once a `commit {}` nested directly inside this block has already
    /// durably committed this journal (design D2): the handle is freed at
    /// that point, so neither a further `JournalRecord` nor this block's
    /// own fall-through/rollback exit may touch it again.
    committed: bool,
    /// Push order relative to `try_stack` frames — see `TryFrame::pushed_at`.
    pushed_at: u64,
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
    /// How many `unsafe` frames were active when this loop started — so
    /// `break`/`continue` can roll back the journals of `unsafe` blocks they
    /// are leaving, the same way exceptions already do
    /// (`Self::lower_pending_exception_dispatch`).
    unsafe_depth: usize,
}

/// Resolver for [`zirk_sema::describe`] used to materialise the `value.type`
/// name at lowering time.
struct TypeNamesResolver<'a>(&'a CheckedProgram);

impl<'a> TypeNames for TypeNamesResolver<'a> {
    fn enum_name(&self, id: u32) -> String {
        self.0.enums[id as usize].name.clone()
    }
    fn function_type(&self, id: u32) -> String {
        let f = &self.0.fn_types[id as usize];
        let params: Vec<String> = f.params.iter().map(|p| describe(*p, self)).collect();
        format!("Fn({}) -> {}", params.join(", "), describe(f.returns, self))
    }
    fn class_name(&self, id: u32) -> String {
        self.0.classes[id as usize].name.clone()
    }
    fn contract_name(&self, id: u32) -> String {
        self.0.contracts[id as usize].name.clone()
    }
    fn type_param_name(&self, id: u32) -> String {
        self.0.type_params[id as usize].name.clone()
    }
    fn instance_name(&self, id: u32) -> String {
        let i = &self.0.generic_instances[id as usize];
        let args: Vec<String> = i.args.iter().map(|a| describe(*a, self)).collect();
        format!(
            "{}<{}>",
            self.0.classes[i.class as usize].name,
            args.join(", ")
        )
    }
    fn contract_instance_name(&self, id: u32) -> String {
        let i = &self.0.contract_instances[id as usize];
        let args: Vec<String> = i.args.iter().map(|a| describe(*a, self)).collect();
        format!(
            "{}<{}>",
            self.0.contracts[i.contract as usize].name,
            args.join(", ")
        )
    }
    fn enum_instance_name(&self, id: u32) -> String {
        let i = &self.0.enum_instances[id as usize];
        let args: Vec<String> = i.args.iter().map(|a| describe(*a, self)).collect();
        format!(
            "{}<{}>",
            self.0.enums[i.enum_id as usize].name,
            args.join(", ")
        )
    }
    fn union_name(&self, id: u32) -> String {
        let parts: Vec<String> = self.0.unions[id as usize]
            .iter()
            .map(|b| describe(Type::of(*b), self))
            .collect();
        format!("Union<{}>", parts.join(" | "))
    }
    fn pointer_element(&self, id: u32) -> Type {
        self.0.pointer_types[id as usize]
    }
    fn weak_element(&self, id: u32) -> Type {
        self.0.weak_types[id as usize]
    }
    fn native_slice_element(&self, id: u32) -> Type {
        self.0.native_slice_types[id as usize]
    }
    fn native_slice_mut_element(&self, id: u32) -> Type {
        self.0.native_slice_mut_types[id as usize]
    }
    fn dependent_element(&self, id: u32) -> Type {
        self.0.dependent_types[id as usize]
    }
    fn pin_element(&self, id: u32) -> Type {
        self.0.pin_types[id as usize]
    }
    fn tuple_name(&self, id: u32) -> String {
        let e: Vec<String> = self.0.tuple_types[id as usize]
            .elements
            .iter()
            .map(|t| describe(*t, self))
            .collect();
        format!("({})", e.join(", "))
    }
    fn array_element(&self, id: u32) -> Type {
        self.0.array_types[id as usize]
    }
    fn list_element(&self, id: u32) -> Type {
        self.0.list_types[id as usize]
    }
    fn range_element(&self, id: u32) -> Type {
        self.0.range_types[id as usize]
    }
    fn map_key(&self, id: u32) -> Type {
        self.0.map_types[id as usize].key
    }
    fn map_value(&self, id: u32) -> Type {
        self.0.map_types[id as usize].value
    }
    fn set_element(&self, id: u32) -> Type {
        self.0.set_types[id as usize]
    }
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
            unsafe_stack: Vec::new(),
            next_scope_seq: 0,
            recursive_call: None,
            current_class: None,
        }
    }

    /// Hands out the next push-order sequence number (design D2 of
    /// `fase-4e-unsafe-journal`) — see `TryFrame::pushed_at`/`UnsafeFrame::pushed_at`.
    fn new_scope_seq(&mut self) -> u64 {
        self.next_scope_seq += 1;
        self.next_scope_seq
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

    /// Human-readable name for a semantic `Type`, used for `value.type`.
    fn type_name(&self, ty: Type) -> String {
        describe(ty, &TypeNamesResolver(self.checked))
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

    /// An `Int64` constant. `ConstInt` carries the value at its declared
    /// width, so it is emitted directly as `Int64` here.
    fn const_i64(&mut self, value: i128, span: Span) -> Operand {
        self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I64), span)
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

        // `Weak<T>` (roadmap Phase 4e, `fase-4e-weak`, design D1): same
        // treatment as `Pointer<T>` above.
        if reference.name == "Weak" {
            let referent = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .weak_types
                .iter()
                .position(|&t| t == referent)
                .expect("the checker interned every Weak<T> it type-checked")
                as u32;
            return Type::of(Base::Weak(id));
        }

        // `NativeSlice<T>`/`NativeSliceMut<T>` (roadmap Phase 4e,
        // `fase-4e-native-slice`, design D1): same treatment as
        // `Pointer<T>`/`Weak<T>` above.
        if reference.name == "NativeSlice" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .native_slice_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every NativeSlice<T> it type-checked")
                as u32;
            return Type::of(Base::NativeSlice(id));
        }
        if reference.name == "NativeSliceMut" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .native_slice_mut_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every NativeSliceMut<T> it type-checked")
                as u32;
            return Type::of(Base::NativeSliceMut(id));
        }

        // `Dependent<T>` / `Pin<T>` (roadmap Phase 4e, `phase-4e-memory`,
        // design D1): same treatment as `Pointer<T>`/`Weak<T>` above.
        if reference.name == "Dependent" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .dependent_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every Dependent<T> it type-checked")
                as u32;
            return Type::of(Base::Dependent(id));
        }
        if reference.name == "Pin" {
            let referent = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .pin_types
                .iter()
                .position(|&t| t == referent)
                .expect("the checker interned every Pin<T> it type-checked")
                as u32;
            return Type::of(Base::Pin(id));
        }

        // `Array<T>` / `List<T>`: same interned-by-content treatment as slices.
        if reference.name == "Array" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .array_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every Array<T> it type-checked")
                as u32;
            return Type::of(Base::Array(id));
        }
        if reference.name == "List" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .list_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every List<T> it type-checked")
                as u32;
            return Type::of(Base::List(id));
        }

        // `Map<K, V>` / `Set<T>` (roadmap Phase 7): same interned-by-content
        // treatment — the checker already interned every shape.
        if reference.name == "Map" {
            let key = self.resolve_written_type(&reference.arguments[0]);
            let value = self.resolve_written_type(&reference.arguments[1]);
            let id = self
                .checked
                .map_types
                .iter()
                .position(|t| t.key == key && t.value == value)
                .expect("the checker interned every Map<K, V> it type-checked")
                as u32;
            return Type::of(Base::Map(id));
        }
        if reference.name == "Set" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .set_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every Set<T> it type-checked")
                as u32;
            return Type::of(Base::Set(id));
        }

        // `Range<T>` (roadmap Phase 7): same interned-by-content treatment —
        // the element type the checker stored is resolved and matched back
        // against `checked.range_types`.
        if reference.name == "Range" {
            let element = self.resolve_written_type(&reference.arguments[0]);
            let id = self
                .checked
                .range_types
                .iter()
                .position(|&t| t == element)
                .expect("the checker interned every Range<T> it type-checked")
                as u32;
            return Type::of(Base::Range(id));
        }

        // `Tuple<T...>` (roadmap Phase 3b): same interned-by-content treatment.
        if reference.name == "Tuple" {
            let elements: Vec<Type> = reference
                .arguments
                .iter()
                .map(|a| self.resolve_written_type(a))
                .collect();
            let tuple = TupleType { elements };
            let id = self
                .checked
                .tuple_types
                .iter()
                .position(|t| *t == tuple)
                .expect("the checker interned every Tuple<T...> it type-checked")
                as u32;
            return Type::of(Base::Tuple(id));
        }

        let declared = self.declaration_of(&reference.name, reference.span);
        if let Some(ty) = Type::from_name(&reference.name) {
            return ty;
        }
        // A `type` alias is transparent (roadmap Phase 7): it expands to the
        // target it was declared with, resolved here the same way the
        // checker's own `resolve_type` does — cycles were already rejected
        // there, so this cannot recurse forever.
        if let Some(target) = self.checked.type_aliases.get(&declared).cloned() {
            let mut resolved = self.resolve_written_type(&target);
            resolved.nullable = reference.nullable;
            return resolved;
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
            let class = &self.checked.classes[id];
            let mut args: Vec<Type> = reference
                .arguments
                .iter()
                .map(|a| self.resolve_written_type(a))
                .collect();

            // Apply trailing type-parameter defaults at use sites the same way
            // the checker does (`resolve_class_reference`): `Box<>` with a
            // default `T = Int32` is the same `Base::Instance` as `Box<Int32>`.
            let params = &class.type_params;
            if args.len() < params.len() {
                let can_default = params[args.len()..]
                    .iter()
                    .all(|&p| self.checked.type_params[p as usize].default.is_some());
                if can_default {
                    let mut subst: Vec<(u32, Type)> =
                        params.iter().copied().zip(args.iter().copied()).collect();
                    for &param in &params[args.len()..] {
                        let default = self.checked.type_params[param as usize]
                            .default
                            .expect("the checker only omits arguments when a default exists");
                        let ty = substitute_generic_type(self.checked, default, &subst);
                        args.push(ty);
                        subst.push((param, ty));
                    }
                }
            }

            if args.is_empty() {
                return Type::of(Base::Class(id as u32));
            }
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
            let actual = self.type_of_operand(value);
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
            } else if let IrType::Value(id) = actual
                && matches!(base, Nullable::Object(_) | Nullable::Contract(_))
            {
                // A `record` widened directly into `T??`'s
                // present half where `T` is a contract it implements
                // (design D1): box first, exactly like the non-nullable
                // path below, then retag if the box's own id is not
                // already the destination's.
                let boxed = self.lower_box_value(value, id, expr.span());
                if base.inner() == IrType::Object(id) {
                    boxed
                } else {
                    self.emit(InstKind::Retype(boxed), base.inner(), expr.span())
                }
            } else if matches!(actual, IrType::Object(_) | IrType::Contract(_))
                && matches!(base, Nullable::Object(_) | Nullable::Contract(_))
            {
                self.emit(InstKind::Retype(value), base.inner(), expr.span())
            } else if Self::is_numeric_ir_type(actual) && Self::is_numeric_ir_type(base.inner()) {
                self.convert_numeric(value, actual, base.inner(), expr.span())
            } else {
                value
            };

            return self.emit(InstKind::Wrap { base, value }, expected, expr.span());
        }

        let value = self.lower_expr(expr);
        let actual = self.type_of_operand(value);
        if actual == expected {
            return value;
        }

        // The checker already proved this is a `record`
        // implementing the contract expected here (design D1,
        // `fase-3-value-type-contract-dispatch`): unlike a class instance,
        // whose address already *is* the shape a contract-typed reference
        // needs, a value type is stored inline with no header — it has to
        // be boxed into a fresh, collector-tracked allocation first, whose
        // descriptor is built the same way a class's own is (this value
        // type's own id doubles as its box's `IrType::Object` id).
        if let IrType::Value(id) = actual
            && matches!(expected, IrType::Object(_) | IrType::Contract(_))
        {
            let boxed = self.lower_box_value(value, id, expr.span());
            return if expected == IrType::Object(id) {
                boxed
            } else {
                self.emit(InstKind::Retype(boxed), expected, expr.span())
            };
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

        // The checker has already proven `actual` can be converted to
        // `expected` without loss (`Type::accepts`). Emit the right numeric
        // conversion (including Int -> Float) and fall back to the value
        // itself when the type already matches.
        if Self::is_numeric_ir_type(actual) && Self::is_numeric_ir_type(expected) {
            return self.convert_numeric(value, actual, expected, expr.span());
        }

        value
    }

    /// Boxes a `record` value into a fresh, collector-tracked
    /// allocation (`fase-3-value-type-contract-dispatch`, design D1) — the
    /// one step that lets it be held through a contract-typed reference.
    ///
    /// `id` is the value type's own id, shared between its `IrType::Value`
    /// entry (`module.values[id]`, its existing inline layout) and its
    /// `IrType::Object` entry (`module.objects[id]`, built by the same
    /// class-descriptor code path `zirk_ir::lower_class_body`'s caller
    /// uses for an ordinary class, populated for a value type only when it
    /// implements at least one contract). Building the box is nothing but
    /// an `Alloc` of that same id, then reading each field back out of the
    /// inline value (`LoadField` already supports an `IrType::Value`
    /// operand) and writing it into the fresh allocation at the same index
    /// — no new instruction kind, and no store ever targets the box again
    /// after this (design D3: the box is read-only from here on).
    fn convert_numeric(&mut self, value: Operand, from: IrType, to: IrType, span: Span) -> Operand {
        if from == to {
            return value;
        }
        match (from, to) {
            (IrType::Int(_), IrType::Int(_)) => self.emit(InstKind::IntCast(value), to, span),
            (IrType::Float(_), IrType::Float(_)) => self.emit(InstKind::FloatCast(value), to, span),
            (IrType::Int(_), IrType::Float(_)) => self.emit(InstKind::IntToFloat(value), to, span),
            (IrType::Float(_), IrType::Int(_)) => self.emit(InstKind::FloatToInt(value), to, span),
            // Exact base-ten `Float` conversions.
            (IrType::Int(_), IrType::Decimal) => self.emit(InstKind::IntToDecimal(value), to, span),
            (IrType::Decimal, IrType::Int(_)) => self.emit(InstKind::DecimalToInt(value), to, span),
            (IrType::Float(_), IrType::Decimal) => {
                self.emit(InstKind::FloatToDecimal(value), to, span)
            }
            (IrType::Decimal, IrType::Float(_)) => {
                self.emit(InstKind::DecimalToFloat(value), to, span)
            }
            _ => value,
        }
    }

    const fn is_numeric_ir_type(ty: IrType) -> bool {
        matches!(ty, IrType::Int(_) | IrType::Float(_) | IrType::Decimal)
    }

    /// The smallest IR numeric type that can represent every value of `left`
    /// and `right` without loss, using the checker's own `Type::common_numeric`.
    fn common_numeric_ir_type(&self, left: IrType, right: IrType) -> Option<IrType> {
        let ty = |ir: IrType| -> Option<Type> {
            match ir {
                IrType::Int(w) => Some(Type::of(Base::Int(sema_int_width(w)))),
                IrType::Float(w) => Some(Type::of(Base::Float(sema_float_width(w)))),
                IrType::Decimal => Some(Type::FLOAT),
                _ => None,
            }
        };
        let l = ty(left)?;
        let r = ty(right)?;
        let common = l.common_numeric(r)?;
        Some(self.ir_type(common))
    }

    fn lower_box_value(&mut self, value: Operand, id: u32, span: Span) -> Operand {
        let object = self.emit(InstKind::Alloc(id), IrType::Object(id), span);
        let field_count = self.module.values[id as usize].fields.len();
        for index in 0..field_count {
            let field_ty = self.module.values[id as usize].fields[index].ty;
            let field_value = self.emit(
                InstKind::LoadField {
                    object: value,
                    index: index as u32,
                },
                field_ty,
                span,
            );
            self.emit_effect(
                InstKind::StoreField {
                    object,
                    index: index as u32,
                    value: field_value,
                },
                span,
            );
        }
        object
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

        self.current_class = Some(id);
        let this = self.declare_slot("this", IrType::Object(id), class.name.span);
        self.initialize_defaults(this, id, class.name.span);

        // An `inner class`'s constructor takes the enclosing instance as a
        // hidden first argument, bound into the `outer` field before the
        // written body runs.
        let mut params = vec![this];
        if let Some(parent) = self.checked.classes[id as usize].enclosing {
            let outer = self.declare_slot("outer", IrType::Object(parent), class.name.span);
            params.push(outer);
            let object = self.emit(InstKind::Load(this), IrType::Object(id), class.name.span);
            let value = self.emit(
                InstKind::Load(outer),
                IrType::Object(parent),
                class.name.span,
            );
            let index = self.checked.classes[id as usize]
                .fields
                .iter()
                .filter(|f| !f.is_static)
                .position(|f| f.name == "outer" && f.owner == id)
                .expect("the checker declares `outer` on an inner class")
                as u32;
            self.emit_effect(
                InstKind::StoreField {
                    object,
                    index,
                    value,
                },
                class.name.span,
            );
        }

        let resolved: Vec<IrType> = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.constructors.get(index))
            .map(|params| params.iter().map(|p| self.ir_type(p.ty)).collect())
            .expect("the checker records every constructor");

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
        let field_types: Vec<IrType> = self.module.objects[id as usize]
            .fields
            .iter()
            .map(|f| f.ty)
            .collect();
        let checked_fields: Vec<&zirk_sema::FieldInfo> = self.checked.classes[id as usize]
            .fields
            .iter()
            .filter(|f| !f.is_static)
            .collect();

        for (index, ty) in field_types.iter().enumerate() {
            let Some(checked) = checked_fields.get(index) else {
                continue;
            };
            let value = if let Some(default) = &checked.default {
                self.lower_expr(default)
            } else {
                let Some(value) = self.default_value(*ty, span) else {
                    // No default: the checker already required the constructor to
                    // write it.
                    continue;
                };
                value
            };
            let object = self.emit(InstKind::Load(this), IrType::Object(id), span);
            self.emit_effect(
                InstKind::StoreField {
                    object,
                    index: index as u32,
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
            IrType::Decimal => self.emit(InstKind::ConstDecimal("0".to_string()), ty, span),
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
            | IrType::Callable(_)
            | IrType::Object(_)
            | IrType::Contract(_)
            | IrType::Value(_)
            | IrType::Enum(_)
            // A `Pointer<T>` field is unreachable in a verified program: the
            // checker's escape rule (design D4) rejects every assignment of
            // a `Pointer<T>` value into a field, so nothing here ever needs
            // a default for one.
            | IrType::Pointer(_)
            // A `Weak<T>` field has no default the same way an ordinary
            // class reference does not (roadmap Phase 4e, `fase-4e-weak`):
            // it is a reference, and there is no handle to default to.
            | IrType::Weak(_)
            // A journal handle is never a Zirk-visible field type — it only
            // ever appears as the `<journal>` slot's own type, always
            // explicitly stored right after `JournalBegin`
            // (roadmap Phase 4e, `fase-4e-unsafe-journal`).
            | IrType::JournalHandle
            // A `NativeSlice<T>`/`NativeSliceMut<T>` field is unreachable in
            // a verified program for the same reason `Pointer<T>` is
            // (roadmap Phase 4e, `fase-4e-native-slice`, design D4): the
            // checker's generalized escape rule rejects every assignment of
            // one into a field.
            | IrType::NativeSlice(_)
            | IrType::NativeSliceMut(_)
            // A `Range<T>` is a runtime handle — there is no constant form
            // of one to default to, the same reason an `Object` has none.
            | IrType::Range
            // `Dependent<T>`/`Pin<T>` fields have no default for the same
            // reason references do not (roadmap Phase 4e, `phase-4e-memory`).
            | IrType::Dependent(_)
            | IrType::Pin(_)
            | IrType::Regex
            // `Array<T>`/`List<T>`/`Map`/`Set` are managed references with no default.
            | IrType::Array(_)
            | IrType::List(_)
            | IrType::Map(_)
            | IrType::Set(_) => {
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

        let is_static = found.is_static;

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());
        self.current_class = Some(id);

        // A record's method receives `this` by value too
        // (roadmap task 11.5): there is nothing to point at.
        let mut params = Vec::new();
        if !is_static {
            let this_ty = self.ir_type(Type::of(Base::Class(id)));
            let this = self.declare_slot("this", this_ty, class.name.span);
            params.push(this);
        }

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
            name: if is_static {
                static_method_symbol(&self.checked.classes[id as usize].name, &method.name.name)
            } else {
                method_symbol(&self.checked.classes[id as usize].name, &method.name.name)
            },
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

    /// Lowers a `static` field as a zero-argument getter that returns the
    /// field's default value.
    fn run_static_field(
        mut self,
        class_name: &str,
        field: &ast::FieldDecl,
        info: &zirk_sema::FieldInfo,
    ) -> (Function, Vec<Function>) {
        let return_type = self.ir_type(info.ty);
        self.return_type = return_type;

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let value = if let Some(default) = &info.default {
            self.lower_expr(default)
        } else {
            self.default_value(return_type, field.name.span)
                .expect("a static field must be initializable")
        };

        self.terminate(Terminator::Return(Some(value)));
        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            name: static_field_symbol(class_name, &field.name.name),
            params: Vec::new(),
            return_type,
            gc_roots: gc_roots_of(self.module, &self.slots),
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: field.name.span,
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

    /// `box_thunk_symbol(class, method)`'s own body
    /// (`fase-3-value-type-contract-dispatch`, design D1/D2): unbox `this`
    /// (the box's pointer) back into the inline value `Self::run_method`'s
    /// real body expects, then forward the call unchanged. This is the one
    /// wrapper that bridges the box's pointer receiver to the value's own
    /// by-value one — everywhere else (`CallContract`'s own dispatch,
    /// `Self::lower_box_value`'s construction) reuses an existing path with
    /// no change at all.
    fn build_box_thunk(mut self, class_id: u32, method: &zirk_sema::MethodInfo) -> Function {
        let span = method.span;
        let entry = self.new_block();
        self.current = entry;

        let this_ty = IrType::Object(class_id);
        let this = self.declare_slot("this", this_ty, span);
        let object = self.emit(InstKind::Load(this), this_ty, span);

        let field_count = self.module.values[class_id as usize].fields.len();
        let mut fields = Vec::with_capacity(field_count);
        for index in 0..field_count {
            let field_ty = self.module.values[class_id as usize].fields[index].ty;
            fields.push(self.emit(
                InstKind::LoadField {
                    object,
                    index: index as u32,
                },
                field_ty,
                span,
            ));
        }
        let receiver = self.emit(
            InstKind::BuildValue {
                class: class_id,
                fields,
            },
            IrType::Value(class_id),
            span,
        );

        let mut params = vec![this];
        let mut args = vec![receiver];
        for (index, param) in method.params.iter().enumerate() {
            let ty = self.ir_type(param.ty);
            let slot = self.declare_slot(&format!("<box_arg{index}>"), ty, span);
            params.push(slot);
            args.push(self.emit(InstKind::Load(slot), ty, span));
        }

        let return_type = self.ir_type(method.returns);
        self.return_type = return_type;
        let callee = method_symbol(&self.checked.classes[class_id as usize].name, &method.name);

        if return_type == IrType::Void {
            self.emit_effect(InstKind::Call { callee, args }, span);
            self.terminate(Terminator::Return(None));
        } else {
            let result = self.emit(InstKind::Call { callee, args }, return_type, span);
            self.terminate(Terminator::Return(Some(result)));
        }

        Function {
            name: box_thunk_symbol(&self.checked.classes[class_id as usize].name, &method.name),
            params,
            return_type,
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
            // `unsafe { }`/`commit { }` (roadmap Phase 4e,
            // `fase-4e-unsafe-journal`, design D1/D2): the transactional
            // journal/rollback contract `fase-4e-unsafe-pointer-extern`
            // deferred as its own D5/D6.
            ast::Stmt::Unsafe(s) => self.lower_unsafe_block(&s.body),
            ast::Stmt::Commit(s) => self.lower_commit_block(&s.body),
            // A local class emits nothing where it is declared: its members
            // lower as ordinary functions through `class_decls`, and its
            // name resolves to the same layout a top-level one gets.
            ast::Stmt::LocalClass(_) => {}
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

        match &stmt.pattern {
            ast::Pattern::Binding(ident) => {
                let slot = self.declare_slot(&ident.name, ty, ident.span);
                if let Some((operand, _)) = value {
                    self.emit_effect(InstKind::Store(slot, operand), stmt.span);
                }
            }
            ast::Pattern::Wildcard(_) => {
                // No binding is declared; the value, if any, was already
                // lowered for its side effects.
            }
            ast::Pattern::Tuple(_) => {
                let slot = self.declare_slot("<tuple>", ty, stmt.pattern.span());
                if let Some((operand, _)) = value {
                    self.emit_effect(InstKind::Store(slot, operand), stmt.span);
                }
                self.lower_pattern_bindings(&stmt.pattern, slot, ty, stmt.span);
            }
            _ => unreachable!("the checker only allows irrefutable patterns in `let`"),
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
            // `view[i] = value` (roadmap Phase 4e, `fase-4e-native-slice`,
            // design D5): the element type its `NativeSliceMut<T>` receiver
            // carries — the checker already rejected a write through a
            // read-only `NativeSlice<T>` before this ever lowers.
            ast::AssignTarget::Index(index) => {
                let receiver_ty = self.type_of(&index.receiver, index.receiver.span());
                if receiver_ty == IrType::String {
                    // The checker rejects `s[i] = c` as non-writable, but the
                    // lowerer must not panic on the way to that diagnostic.
                    return IrType::Char;
                }
                self.native_slice_element_type(receiver_ty)
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
                self.journal_writes_to_slot(slot, span);
                self.emit_effect(InstKind::Store(slot, value), span);
            }
            ast::AssignTarget::Field(field) => {
                let object = self.lower_expr(&field.object);
                let (index, _ty) = self.field_position(&field.object, &field.name.name);
                self.journal_writes_to_field(object, index, span);
                self.emit_effect(
                    InstKind::StoreField {
                        object,
                        index,
                        value,
                    },
                    span,
                );
            }
            // `view[i] = value` (design D5): `value` is already a plain
            // operand by the time this runs (this function's own doc
            // comment), computed by the caller in whatever block was
            // current then — spilled *immediately*, before anything below
            // (the receiver, the index, or the bounds check's own branch)
            // gets a chance to open a new block and strand it (ADR-007 D7).
            // The receiver is spilled right after it is lowered for the
            // same reason, before the index expression gets that same
            // chance.
            ast::AssignTarget::Index(index_expr) => {
                let value_ty = self.type_of_operand(value);
                let value_slot = self.spill(value, value_ty, span);

                let receiver = self.lower_expr(&index_expr.receiver);
                let receiver_ty = self.type_of_operand(receiver);
                if receiver_ty == IrType::String {
                    // `s[i] = c` (roadmap Phase 7): same lowering
                    // `lower_assign` reaches — the value `lower_multi_assign`
                    // staged is already a plain operand.
                    self.lower_string_index_write(index_expr, value, span);
                    return;
                }
                let receiver_slot = self.spill(receiver, receiver_ty, span);

                let index_operand =
                    self.lower_expr_as(&index_expr.index, IrType::Int(IntWidth::U64));
                let index_slot = self.spill(index_operand, IrType::Int(IntWidth::U64), span);

                let (receiver, index) = self.lower_native_slice_bounds_check(
                    receiver_slot,
                    receiver_ty,
                    index_slot,
                    span,
                );
                let value = self.emit(InstKind::Load(value_slot), value_ty, span);
                self.emit_effect(
                    InstKind::NativeSliceStore {
                        receiver,
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
                self.journal_writes_to_slot(slot, stmt.span);
                self.emit_effect(InstKind::Store(slot, value), stmt.span);
            }
            ast::AssignTarget::Field(field) => {
                // The object is evaluated before the value, which is the order
                // it is written in. The value may now introduce a guard branch
                // (`+`, `-`, `*`, `/`, `%` with overflow or division-by-zero
                // checks) that moves `self.current` to a fresh block — so the
                // object computed here does not survive to the `StoreField`
                // below unless it goes through a slot first, the same holder
                // pattern `lower_safe_field` uses for its own receiver (values
                // do not cross blocks, ADR-007).
                let object = self.lower_expr(&field.object);
                let (index, ty) = self.field_position(&field.object, &field.name.name);
                let object_ty = self.type_of(&field.object, field.object.span());
                let holder = self.declare_slot("<assign_object>", object_ty, field.span);
                self.emit_effect(InstKind::Store(holder, object), stmt.span);
                let held = Held::Spilled(holder, object_ty);
                let value = self.lower_expr_as(&stmt.value, ty);
                let object = self.reload(held, field.span);
                self.journal_writes_to_field(object, index, stmt.span);
                self.emit_effect(
                    InstKind::StoreField {
                        object,
                        index,
                        value,
                    },
                    stmt.span,
                );
            }
            // `view[i] = value;` (roadmap Phase 4e, `fase-4e-native-slice`,
            // design D5): the receiver is spilled right after it is
            // lowered, and the index right after it, *before* `stmt.value`
            // is lowered — `view[0] = view[0] + 1;` is exactly the shape
            // that breaks otherwise: `stmt.value` here itself contains
            // another index read, which opens its own `fail`/`cont` split
            // (`Self::lower_native_slice_bounds_check`) and would strand an
            // unspilled receiver/index computed before it (ADR-007 D7).
            // `value` is spilled too, since the bounds check just below
            // opens a split of its own regardless of what `stmt.value` did.
            ast::AssignTarget::Index(index_expr) => {
                // `s[i] = c` (roadmap Phase 7, "String write by index"):
                // a `String` cannot be mutated in place — its bytes are
                // inline and a replacement grapheme may be a different
                // length — so the write lowers to a fresh `String` stored
                // back into the variable the checker required the receiver
                // to be.
                if self.type_of(&index_expr.receiver, index_expr.receiver.span()) == IrType::String
                {
                    // `Char` and `String` share one runtime representation
                    // (ADR-014): whichever the checker accepted is already
                    // the handle `zirk_str_set` reads.
                    let value = self.lower_expr(&stmt.value);
                    self.lower_string_index_write(index_expr, value, stmt.span);
                    return;
                }

                let receiver = self.lower_expr(&index_expr.receiver);
                let receiver_ty = self.type_of_operand(receiver);
                let receiver_slot = self.spill(receiver, receiver_ty, stmt.span);

                if matches!(receiver_ty, IrType::Array(_) | IrType::List(_)) {
                    let index_operand =
                        self.lower_expr_as(&index_expr.index, IrType::Int(IntWidth::I64));
                    let index_slot =
                        self.spill(index_operand, IrType::Int(IntWidth::I64), stmt.span);
                    let element_ty = self.array_list_element_type(receiver_ty);
                    let receiver = self.emit(InstKind::Load(receiver_slot), receiver_ty, stmt.span);
                    let index = self.emit(
                        InstKind::Load(index_slot),
                        IrType::Int(IntWidth::I64),
                        stmt.span,
                    );
                    let value = self.lower_expr_as(&stmt.value, element_ty);
                    self.emit_effect(
                        InstKind::ArrayListStore {
                            receiver,
                            index,
                            value,
                        },
                        stmt.span,
                    );
                    return;
                }

                let index_operand =
                    self.lower_expr_as(&index_expr.index, IrType::Int(IntWidth::U64));
                let index_slot = self.spill(index_operand, IrType::Int(IntWidth::U64), stmt.span);

                let element_ty = self.native_slice_element_type(receiver_ty);
                let value = self.lower_expr_as(&stmt.value, element_ty);
                let value_slot = self.spill(value, element_ty, stmt.span);

                let (receiver, index) = self.lower_native_slice_bounds_check(
                    receiver_slot,
                    receiver_ty,
                    index_slot,
                    stmt.span,
                );
                let value = self.emit(InstKind::Load(value_slot), element_ty, stmt.span);
                self.emit_effect(
                    InstKind::NativeSliceStore {
                        receiver,
                        index,
                        value,
                    },
                    stmt.span,
                );
            }
        }
    }

    /// `s[i] = c` where `s` is a `String` variable (roadmap Phase 7): builds
    /// the replacement `String` through `zirk_str_set` — bounds-checked the
    /// same way a `s[i]` read is (`StringGraphemeOffset` answering `-1` when
    /// the index is out of range throws `IndexOutOfBoundsError`) — and stores
    /// it back into the variable's slot. `value` is an already-lowered
    /// `Char`/`String` handle.
    fn lower_string_index_write(
        &mut self,
        index_expr: &ast::IndexExpr,
        value: Operand,
        span: Span,
    ) {
        let ast::Expr::Path(name) = &*index_expr.receiver else {
            unreachable!("the checker only writes through a `String` variable")
        };
        let slot = self.lookup_slot(&name.name);
        let value_ty = self.type_of_operand(value);
        let value_slot = self.spill(value, value_ty, span);

        let string = self.emit(InstKind::Load(slot), IrType::String, index_expr.span);
        let string_slot = self.spill(string, IrType::String, span);
        let index = self.lower_expr_as(&index_expr.index, IrType::Int(IntWidth::I64));
        let index_slot = self.spill(index, IrType::Int(IntWidth::I64), span);

        let string = self.emit(InstKind::Load(string_slot), IrType::String, span);
        let index = self.emit(InstKind::Load(index_slot), IrType::Int(IntWidth::I64), span);
        let offset = self.emit(
            InstKind::StringGraphemeOffset { string, index },
            IrType::Int(IntWidth::I64),
            span,
        );
        let offset_slot = self.spill(offset, IrType::Int(IntWidth::I64), span);

        let minus_one = self.const_i64(-1, span);
        let offset = self.emit(
            InstKind::Load(offset_slot),
            IrType::Int(IntWidth::I64),
            span,
        );
        let is_out = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: offset,
                right: minus_one,
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_out,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program indexing a string registered the exception hierarchy");
        self.throw_native_failure(
            native.index_out_of_bounds,
            "string index out of bounds",
            span,
        );

        self.current = cont;
        let string = self.emit(InstKind::Load(string_slot), IrType::String, span);
        let offset = self.emit(
            InstKind::Load(offset_slot),
            IrType::Int(IntWidth::I64),
            span,
        );
        let length = self.emit(
            InstKind::GraphemeLenAt { string, offset },
            IrType::Int(IntWidth::I64),
            span,
        );
        let value = self.emit(InstKind::Load(value_slot), value_ty, span);
        let replaced = self.emit(
            InstKind::Call {
                callee: "zirk_str_set".to_string(),
                args: vec![string, offset, length, value],
            },
            IrType::String,
            span,
        );
        self.journal_writes_to_slot(slot, span);
        self.emit_effect(InstKind::Store(slot, replaced), span);
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
            unsafe_depth: self.unsafe_stack.len(),
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
        let iterable_ty = self.type_of(&stmt.iterable, stmt.iterable.span());
        if iterable_ty == IrType::String {
            return self.lower_for_in_string(stmt);
        }
        if iterable_ty == IrType::Range {
            return self.lower_for_in_range_value(stmt);
        }
        if matches!(iterable_ty, IrType::Array(_) | IrType::List(_)) {
            return self.lower_for_in_array_list(stmt, iterable_ty);
        }
        self.lower_for_in_iterable(stmt);
    }

    /// The element type `for ... in` over a `Range<T>` binds — read back
    /// from the checker's record of the iterable expression (`expr_types`
    /// keyed by the `RangeExpr`'s own span, or the iterable's), lowered to
    /// its `i32`/`i64` width.
    fn range_loop_element_type(&mut self, span: Span) -> IrType {
        let Some(&ty) = self.checked.expr_types.get(&span) else {
            return IrType::Int(IntWidth::I32);
        };
        let Base::Range(id) = ty.base else {
            return IrType::Int(IntWidth::I32);
        };
        let element = self.checked.range_types[id as usize];
        self.ir_type(element)
    }

    /// Lowers `for i in a..b { ... }`, `a..=b` and `a..b..step` (roadmap
    /// Phase 7, `Range<T>`): the literal form never materializes a `Range`
    /// object — the loop runs a counter of the element's own width
    /// (`Int32`, or `i64` nanoseconds for `Duration`) straight off the
    /// written operands, exactly the values `zirk_range_new` would store.
    fn lower_for_in_range(&mut self, stmt: &ast::ForInStmt, range: &ast::RangeExpr) {
        let element = self.range_loop_element_type(range.span);
        let start = self.lower_expr_as(&range.start, element);
        let end = self.lower_expr_as(&range.end, element);
        let step = match &range.step {
            Some(step) => self.lower_expr_as(step, element),
            None => self.const_int_at(1, element, stmt.span),
        };
        let inclusive = range.inclusive;
        self.emit_range_loop(stmt, element, start, end, step, inclusive);
    }

    /// `for x in range` where `range` is a `Range<T>` value rather than a
    /// literal (roadmap Phase 7): the object's fields feed the exact same
    /// counter loop the literal spelling takes. The runtime stores every
    /// part as `i64`, so each is read back through `IntCast` into the
    /// element's own width.
    fn lower_for_in_range_value(&mut self, stmt: &ast::ForInStmt) {
        let element = self.range_loop_element_type(stmt.iterable.span());
        let range = self.lower_expr(&stmt.iterable);
        let getter = |this: &mut Self, name: &str| {
            let value = this.emit(
                InstKind::Call {
                    callee: name.to_string(),
                    args: vec![range],
                },
                IrType::Int(IntWidth::I64),
                stmt.span,
            );
            if element == IrType::Int(IntWidth::I64) {
                value
            } else {
                this.emit(InstKind::IntCast(value), element, stmt.span)
            }
        };
        let start = getter(self, "zirk_range_start");
        let end = getter(self, "zirk_range_end");
        let step = getter(self, "zirk_range_step");
        let inclusive = self.emit(
            InstKind::Call {
                callee: "zirk_range_inclusive".to_string(),
                args: vec![range],
            },
            IrType::Int(IntWidth::I64),
            stmt.span,
        );
        let zero = self.const_int_at(0, IrType::Int(IntWidth::I64), stmt.span);
        let inclusive = self.emit(
            InstKind::Binary {
                op: BinaryOp::NotEq,
                left: inclusive,
                right: zero,
            },
            IrType::Boolean,
            stmt.span,
        );
        self.emit_range_loop_dynamic(stmt, element, start, end, step, inclusive);
    }

    /// The loop `for ... in <range>` emits once its operands are known.
    /// `step` may carry any sign, so the keep-going test cannot pick its
    /// comparison at lowering time: it computes
    /// `forward ? cur {<|<=} end : cur {>|>=} end` — a `step` of `0`
    /// throws `InvalidStepError` before the loop is entered, since such a
    /// stride can never advance.
    fn emit_range_loop(
        &mut self,
        stmt: &ast::ForInStmt,
        element: IrType,
        start: Operand,
        end: Operand,
        step: Operand,
        inclusive: bool,
    ) {
        self.scopes.push(HashMap::new());

        let binding = self.declare_slot(&stmt.binding.name, element, stmt.binding.span);
        self.emit_effect(InstKind::Store(binding, start), stmt.span);

        // `end` and `step` are evaluated once, before the loop:
        // re-evaluating them each iteration would call any function in
        // them repeatedly.
        let limit = self.declare_slot("<range end>", element, stmt.span);
        self.emit_effect(InstKind::Store(limit, end), stmt.span);
        let stride = self.declare_slot("<range step>", element, stmt.span);
        self.emit_effect(InstKind::Store(stride, step), stmt.span);

        // `a..b..0` can never advance — a controlled `InvalidStepError`
        // (roadmap Phase 7), thrown before the loop starts.
        let stride_value = self.emit(InstKind::Load(stride), element, stmt.span);
        let zero = self.const_int_at(0, element, stmt.span);
        let zero_step = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: stride_value,
                right: zero,
            },
            IrType::Boolean,
            stmt.span,
        );
        let fail_block = self.new_block();
        let ok_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: zero_step,
            then_block: fail_block,
            else_block: ok_block,
        });
        self.current = fail_block;
        let invalid_step = self
            .checked
            .native_exceptions
            .expect("a program constructing a range registered the exception hierarchy")
            .invalid_step;
        self.throw_native_failure(invalid_step, "a range's step cannot be zero", stmt.span);
        self.terminate(Terminator::Jump(ok_block));
        self.current = ok_block;

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let current = self.emit(InstKind::Load(binding), element, stmt.span);
        let bound = self.emit(InstKind::Load(limit), element, stmt.span);
        // `forward ? cur {<|<=} bound : cur {>|>=} bound` — the step's sign
        // is not known until run time, so both comparisons run and the sign
        // picks one.
        let within = self.emit(
            InstKind::Binary {
                op: if inclusive {
                    BinaryOp::LtEq
                } else {
                    BinaryOp::Lt
                },
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        let within_back = self.emit(
            InstKind::Binary {
                op: if inclusive {
                    BinaryOp::GtEq
                } else {
                    BinaryOp::Gt
                },
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        let stride_value = self.emit(InstKind::Load(stride), element, stmt.span);
        let zero = self.const_int_at(0, element, stmt.span);
        let forward = self.emit(
            InstKind::Binary {
                op: BinaryOp::Gt,
                left: stride_value,
                right: zero,
            },
            IrType::Boolean,
            stmt.span,
        );
        let backward = self.emit(
            InstKind::Unary {
                op: UnaryOp::Not,
                operand: forward,
            },
            IrType::Boolean,
            stmt.span,
        );
        let keep_forward = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: forward,
                right: within,
            },
            IrType::Boolean,
            stmt.span,
        );
        let keep_backward = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: backward,
                right: within_back,
            },
            IrType::Boolean,
            stmt.span,
        );
        let keep_going = self.emit(
            InstKind::Binary {
                op: BinaryOp::Or,
                left: keep_forward,
                right: keep_backward,
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
            unsafe_depth: self.unsafe_stack.len(),
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let value = self.emit(InstKind::Load(binding), element, stmt.span);
        let stride_value = self.emit(InstKind::Load(stride), element, stmt.span);
        let next = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: value,
                right: stride_value,
            },
            element,
            stmt.span,
        );
        self.emit_effect(InstKind::Store(binding, next), stmt.span);
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    /// The value-form twin of [`Self::emit_range_loop`]: `inclusive` is
    /// itself dynamic here (read from the range object), so the endpoint
    /// compare is `(forward && (incl ? cur <= end : cur < end)) ||
    /// (!forward && (incl ? cur >= end : cur > end))` — the flag picks the
    /// comparison the literal spelling writes into the IR directly.
    fn emit_range_loop_dynamic(
        &mut self,
        stmt: &ast::ForInStmt,
        element: IrType,
        start: Operand,
        end: Operand,
        step: Operand,
        inclusive: Operand,
    ) {
        self.scopes.push(HashMap::new());

        let binding = self.declare_slot(&stmt.binding.name, element, stmt.binding.span);
        self.emit_effect(InstKind::Store(binding, start), stmt.span);

        // `end`, `step` and the `..=` flag are evaluated once, before the
        // loop: re-evaluating them each iteration would re-run whatever
        // produced the range.
        let limit = self.declare_slot("<range end>", element, stmt.span);
        self.emit_effect(InstKind::Store(limit, end), stmt.span);
        let stride = self.declare_slot("<range step>", element, stmt.span);
        self.emit_effect(InstKind::Store(stride, step), stmt.span);
        let incl = self.declare_slot("<range inclusive>", IrType::Boolean, stmt.span);
        self.emit_effect(InstKind::Store(incl, inclusive), stmt.span);

        // A range object can only be built by `zirk_range_new`, whose own
        // caller guards `step == 0`; slices and `reverse()` produce
        // non-zero steps. The check repeats anyway — a hand-off through a
        // slot is exactly where such an invariant is easiest to lose.
        let stride_value = self.emit(InstKind::Load(stride), element, stmt.span);
        let zero = self.const_int_at(0, element, stmt.span);
        let zero_step = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: stride_value,
                right: zero,
            },
            IrType::Boolean,
            stmt.span,
        );
        let fail_block = self.new_block();
        let ok_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: zero_step,
            then_block: fail_block,
            else_block: ok_block,
        });
        self.current = fail_block;
        let invalid_step = self
            .checked
            .native_exceptions
            .expect("a program constructing a range registered the exception hierarchy")
            .invalid_step;
        self.throw_native_failure(invalid_step, "a range's step cannot be zero", stmt.span);
        self.terminate(Terminator::Jump(ok_block));
        self.current = ok_block;

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let current = self.emit(InstKind::Load(binding), element, stmt.span);
        let bound = self.emit(InstKind::Load(limit), element, stmt.span);
        let inclusive = self.emit(InstKind::Load(incl), IrType::Boolean, stmt.span);
        let exclusive = self.emit(
            InstKind::Unary {
                op: UnaryOp::Not,
                operand: inclusive,
            },
            IrType::Boolean,
            stmt.span,
        );

        // Within the bound, forward: `incl ? cur <= end : cur < end`.
        let lt = self.emit(
            InstKind::Binary {
                op: BinaryOp::Lt,
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        let le = self.emit(
            InstKind::Binary {
                op: BinaryOp::LtEq,
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        let incl_le = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: inclusive,
                right: le,
            },
            IrType::Boolean,
            stmt.span,
        );
        let excl_lt = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: exclusive,
                right: lt,
            },
            IrType::Boolean,
            stmt.span,
        );
        let within_fwd = self.emit(
            InstKind::Binary {
                op: BinaryOp::Or,
                left: incl_le,
                right: excl_lt,
            },
            IrType::Boolean,
            stmt.span,
        );

        // Within the bound, backward: `incl ? cur >= end : cur > end`.
        let gt = self.emit(
            InstKind::Binary {
                op: BinaryOp::Gt,
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        let ge = self.emit(
            InstKind::Binary {
                op: BinaryOp::GtEq,
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        let incl_ge = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: inclusive,
                right: ge,
            },
            IrType::Boolean,
            stmt.span,
        );
        let excl_gt = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: exclusive,
                right: gt,
            },
            IrType::Boolean,
            stmt.span,
        );
        let within_bwd = self.emit(
            InstKind::Binary {
                op: BinaryOp::Or,
                left: incl_ge,
                right: excl_gt,
            },
            IrType::Boolean,
            stmt.span,
        );

        // The step's sign picks which direction's test answers.
        let stride_value = self.emit(InstKind::Load(stride), element, stmt.span);
        let zero = self.const_int_at(0, element, stmt.span);
        let forward = self.emit(
            InstKind::Binary {
                op: BinaryOp::Gt,
                left: stride_value,
                right: zero,
            },
            IrType::Boolean,
            stmt.span,
        );
        let backward = self.emit(
            InstKind::Unary {
                op: UnaryOp::Not,
                operand: forward,
            },
            IrType::Boolean,
            stmt.span,
        );
        let keep_fwd = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: forward,
                right: within_fwd,
            },
            IrType::Boolean,
            stmt.span,
        );
        let keep_bwd = self.emit(
            InstKind::Binary {
                op: BinaryOp::And,
                left: backward,
                right: within_bwd,
            },
            IrType::Boolean,
            stmt.span,
        );
        let keep_going = self.emit(
            InstKind::Binary {
                op: BinaryOp::Or,
                left: keep_fwd,
                right: keep_bwd,
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
            unsafe_depth: self.unsafe_stack.len(),
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let value = self.emit(InstKind::Load(binding), element, stmt.span);
        let stride_value = self.emit(InstKind::Load(stride), element, stmt.span);
        let next = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: value,
                right: stride_value,
            },
            element,
            stmt.span,
        );
        self.emit_effect(InstKind::Store(binding, next), stmt.span);
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    /// Lowers `for x in arrayOrList { ... }` using a counter and the
    /// collection's own length instruction.
    fn lower_for_in_array_list(&mut self, stmt: &ast::ForInStmt, iterable_ty: IrType) {
        self.scopes.push(HashMap::new());

        let iterable = self.lower_expr(&stmt.iterable);
        let iterable_slot = self.declare_slot("<iterable>", iterable_ty, stmt.iterable.span());
        self.emit_effect(InstKind::Store(iterable_slot, iterable), stmt.span);

        let iterable_value = self.emit(InstKind::Load(iterable_slot), iterable_ty, stmt.span);
        let length = self.emit(
            if matches!(iterable_ty, IrType::Array(_)) {
                InstKind::ArrayLength(iterable_value)
            } else {
                InstKind::ListLength(iterable_value)
            },
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        let length_slot = self.declare_slot("<length>", IrType::Int(IntWidth::U64), stmt.span);
        self.emit_effect(InstKind::Store(length_slot, length), stmt.span);

        let element_ty = self.array_list_element_type(iterable_ty);
        let counter = self.declare_slot(
            &stmt.binding.name,
            IrType::Int(IntWidth::U64),
            stmt.binding.span,
        );
        let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), stmt.span);
        let zero = self.emit(
            InstKind::IntCast(zero),
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        self.emit_effect(InstKind::Store(counter, zero), stmt.span);

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let current = self.emit(
            InstKind::Load(counter),
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        let bound = self.emit(
            InstKind::Load(length_slot),
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        let keep_going = self.emit(
            InstKind::Binary {
                op: BinaryOp::Lt,
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
            unsafe_depth: self.unsafe_stack.len(),
        });

        self.current = body_block;
        let iterable_value = self.emit(InstKind::Load(iterable_slot), iterable_ty, stmt.span);
        let index = self.emit(
            InstKind::Load(counter),
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        let element = self.emit(
            InstKind::ArrayListLoad {
                receiver: iterable_value,
                index,
            },
            element_ty,
            stmt.span,
        );
        let binding = self.declare_slot(&stmt.binding.name, element_ty, stmt.binding.span);
        self.emit_effect(InstKind::Store(binding, element), stmt.span);
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let value = self.emit(
            InstKind::Load(counter),
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        let one = self.emit(InstKind::ConstInt(1), IrType::Int(IntWidth::I32), stmt.span);
        let one = self.emit(
            InstKind::IntCast(one),
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        let next = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: value,
                right: one,
            },
            IrType::Int(IntWidth::U64),
            stmt.span,
        );
        self.emit_effect(InstKind::Store(counter, next), stmt.span);
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
            unsafe_depth: self.unsafe_stack.len(),
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
            InstKind::ConstInt(item as i128),
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
            unsafe_depth: self.unsafe_stack.len(),
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
        self.run_exit_cleanup(loop_target.try_depth, loop_target.unsafe_depth, _stmt.span);
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
        self.run_exit_cleanup(loop_target.try_depth, loop_target.unsafe_depth, _stmt.span);
        self.terminate(Terminator::Jump(target));

        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_return(&mut self, stmt: &ast::ReturnStmt) {
        let expected = self.return_type;
        let value = stmt.value.as_ref().map(|e| self.lower_expr_as(e, expected));
        // Spilled ahead of `run_exit_cleanup` below, the same reason
        // `Self::lower_and_hold` spills any operand a later, block-opening
        // expression would otherwise strand (ADR-007): `finally` or
        // `JournalRollback` on the way out may itself open blocks — a call
        // inside it does, unconditionally, since D11 (`fase-4d-runtimeerror`)
        // — and `value` was computed in the block *before* that, so using it
        // directly in this statement's own `Terminator::Return` below (built
        // in whichever block the cleanup left `self.current` at) would use a
        // value from a block this one is no longer that same block (found
        // via a `return call();` inside a `match ... with` arm whose own
        // resource-close `finally` calls a `Void` method).
        let holder = value.map(|operand| {
            let ty = self.type_of_operand(operand);
            let slot = self.declare_slot("<return_value>", ty, stmt.span);
            self.emit_effect(InstKind::Store(slot, operand), stmt.span);
            (slot, ty)
        });
        // A `return` leaves every enclosing `try` and `unsafe` frame, not
        // just the innermost one (roadmap Phase 4b/4e, design D2/D3) —
        // unlike `break`/`continue`, which only exit the ones nested *inside*
        // the loop they target.
        self.run_exit_cleanup(0, 0, stmt.span);
        let value = holder.map(|(slot, ty)| self.emit(InstKind::Load(slot), ty, stmt.span));
        self.terminate(Terminator::Return(value));
    }

    /// Runs the cleanup owed by `return`/`break`/`continue` leaving one or
    /// more enclosing `try` and `unsafe` frames, from `try_depth` and
    /// `unsafe_depth` onward, innermost first. `try` frames run their
    /// `finally`; `unsafe` frames emit `JournalRollback` for any journal not
    /// already durably committed. The two kinds of frames are interleaved by
    /// their lexical `pushed_at` order, the same as a propagating exception
    /// does (`Self::lower_pending_exception_dispatch`, roadmap Phase 4e,
    /// design D2).
    fn run_exit_cleanup(&mut self, try_depth: usize, unsafe_depth: usize, span: Span) {
        enum Scope {
            Try(TryFrame),
            Unsafe(UnsafeFrame),
        }

        let mut scopes: Vec<Scope> = self
            .try_stack
            .iter()
            .skip(try_depth)
            .cloned()
            .map(Scope::Try)
            .chain(
                self.unsafe_stack
                    .iter()
                    .skip(unsafe_depth)
                    .cloned()
                    .map(Scope::Unsafe),
            )
            .collect();
        scopes.sort_by_key(|scope| match scope {
            Scope::Try(frame) => frame.pushed_at,
            Scope::Unsafe(frame) => frame.pushed_at,
        });

        let full_try_stack = std::mem::take(&mut self.try_stack);
        let full_unsafe_stack = std::mem::take(&mut self.unsafe_stack);
        let mut rolled_back: Vec<SlotId> = Vec::new();

        for index in (0..scopes.len()).rev() {
            let mut outer_try = Vec::new();
            let mut outer_unsafe = Vec::new();
            for scope in &scopes[..index] {
                match scope {
                    Scope::Try(frame) => outer_try.push(frame.clone()),
                    Scope::Unsafe(frame) => outer_unsafe.push(frame.clone()),
                }
            }
            self.try_stack = outer_try;
            self.unsafe_stack = outer_unsafe;

            match &scopes[index] {
                Scope::Try(frame) => {
                    self.lower_finally_block(&frame.finally);
                }
                Scope::Unsafe(frame) => {
                    if !frame.committed {
                        let journal = self.emit(
                            InstKind::Load(frame.journal_slot),
                            IrType::JournalHandle,
                            span,
                        );
                        self.emit_effect(InstKind::JournalRollback(journal), span);
                        rolled_back.push(frame.journal_slot);
                    }
                }
            }
        }

        self.try_stack = full_try_stack;
        self.unsafe_stack = full_unsafe_stack;

        for slot in rolled_back {
            for frame in self.unsafe_stack.iter_mut() {
                if frame.journal_slot == slot {
                    frame.committed = true;
                }
            }
        }
    }

    // --- Expressions ------------------------------------------------------

    fn lower_expr(&mut self, expr: &ast::Expr) -> Operand {
        let span = expr.span();

        match expr {
            ast::Expr::Int(lit) => {
                let ty = self
                    .checked
                    .expr_types
                    .get(&span)
                    .copied()
                    .filter(|t| !t.is_unknown())
                    .map(|t| self.ir_type(t))
                    .unwrap_or(IrType::Int(IntWidth::I32));
                match ty {
                    IrType::Float(w) => {
                        let text = lit.value.to_string();
                        self.emit(InstKind::ConstFloat(w, text), ty, span)
                    }
                    IrType::Int(_) => self.emit(InstKind::ConstInt(lit.value), ty, span),
                    _ => self.emit(
                        InstKind::ConstInt(lit.value),
                        IrType::Int(IntWidth::I32),
                        span,
                    ),
                }
            }
            ast::Expr::Duration(lit) => self.emit(
                InstKind::ConstInt(lit.nanos as i128),
                IrType::Int(IntWidth::I64),
                span,
            ),
            ast::Expr::Float(lit) => {
                let ty = self
                    .checked
                    .expr_types
                    .get(&span)
                    .copied()
                    .filter(|t| !t.is_unknown())
                    .map(|t| self.ir_type(t))
                    .unwrap_or(IrType::Float(float_literal_width(lit)));
                match ty {
                    IrType::Int(_) => {
                        let parsed: f64 = lit.text.parse().unwrap_or(f64::NAN);
                        if !parsed.is_nan() && parsed.fract() == 0.0 {
                            self.emit(InstKind::ConstInt(parsed as i128), ty, span)
                        } else {
                            let width = float_literal_width(lit);
                            self.emit(
                                InstKind::ConstFloat(width, lit.text.clone()),
                                IrType::Float(width),
                                span,
                            )
                        }
                    }
                    IrType::Float(w) => {
                        self.emit(InstKind::ConstFloat(w, lit.text.clone()), ty, span)
                    }
                    IrType::Decimal => {
                        self.emit(InstKind::ConstDecimal(lit.text.clone()), ty, span)
                    }
                    _ => {
                        let width = float_literal_width(lit);
                        self.emit(
                            InstKind::ConstFloat(width, lit.text.clone()),
                            IrType::Float(width),
                            span,
                        )
                    }
                }
            }
            ast::Expr::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Expr::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }
            ast::Expr::Regex(lit) => {
                let pattern_id = self.module.intern_string(&lit.pattern);
                let pattern = self.emit(InstKind::ConstString(pattern_id), IrType::String, span);
                self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_from_pattern".to_string(),
                        args: vec![pattern],
                    },
                    IrType::Regex,
                    span,
                )
            }
            ast::Expr::Char(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstChar(id), IrType::Char, span)
            }

            ast::Expr::Path(ident) => {
                if let Some(slot) = self.try_lookup_slot(&ident.name) {
                    let ty = self.slot_type(slot);
                    self.emit(InstKind::Load(slot), ty, span)
                } else if ident.name == "outer"
                    && let Some(class) = self.current_class
                    && let Some(parent) = self.checked.classes[class as usize].enclosing
                {
                    // `outer` reads the hidden field an `inner class`
                    // stores its enclosing instance in.
                    let index = self.checked.classes[class as usize]
                        .fields
                        .iter()
                        .filter(|f| !f.is_static)
                        .position(|f| f.name == "outer" && f.owner == class)
                        .expect("the checker declares `outer` on an inner class")
                        as u32;
                    let this = self.lookup_slot("this");
                    let object = self.emit(InstKind::Load(this), IrType::Object(class), span);
                    self.emit(
                        InstKind::LoadField { object, index },
                        IrType::Object(parent),
                        span,
                    )
                } else {
                    // A bare function name with no local shadowing it is a
                    // boxed callable value (`Checker::check_path`, roadmap
                    // Phase 4d, design D12): the canonical, capture-less
                    // layout the `Self::lower` pre-pass already reserved for
                    // this shape, targeting this specific function.
                    let (id, target) = self
                        .named_function_value(&ident.name, ident.span)
                        .expect("a verified program only names declared variables or functions");
                    self.emit(
                        InstKind::MakeCallable {
                            target,
                            captures: Vec::new(),
                        },
                        IrType::Callable(id),
                        span,
                    )
                }
            }

            ast::Expr::Unary(e) => {
                let operand = self.lower_expr(&e.operand);
                let ty = self.type_of_operand(operand);
                match e.op {
                    ast::UnaryOp::Neg if matches!(ty, IrType::Int(_)) => {
                        // Integer negation overflows for `Int.MIN`, so lower it
                        // to a guarded `0 - x` instead of a raw `Unary`.
                        let zero = self.const_int_at(0, ty, span);
                        self.emit_checked_binary(BinaryOp::Sub, zero, operand, ty, span)
                    }
                    ast::UnaryOp::Neg if ty == IrType::Decimal => self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_decimal_neg".to_string(),
                            args: vec![operand],
                        },
                        IrType::Decimal,
                        span,
                    ),
                    ast::UnaryOp::Neg => self.emit(
                        InstKind::Unary {
                            op: UnaryOp::Neg,
                            operand,
                        },
                        ty,
                        span,
                    ),
                    ast::UnaryOp::BitNot => self.emit(
                        InstKind::Unary {
                            op: UnaryOp::BitNot,
                            operand,
                        },
                        ty,
                        span,
                    ),
                    ast::UnaryOp::Not => self.emit(
                        InstKind::Unary {
                            op: UnaryOp::Not,
                            operand,
                        },
                        IrType::Boolean,
                        span,
                    ),
                }
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

            // `x == y` / `x != y` where both operands are a `record`/`value
            // class` (`fase-3-structural-equality`, design D1): a `record`/
            // `record` never defines `_equals` (the check just above
            // only ever matches `IrType::Object`, never `IrType::Value`),
            // so this is the only place its comparison is built — a
            // conjunction of per-field comparisons, generated at this call
            // site rather than through a synthesized per-type function.
            ast::Expr::Binary(e)
                if matches!(e.op, ast::BinaryOp::Eq | ast::BinaryOp::NotEq)
                    && matches!(self.type_of(&e.left, e.left.span()), IrType::Value(_)) =>
            {
                self.lower_structural_equality(e, span)
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
                // The binary itself may introduce a guard branch (integer or
                // `Float` arithmetic, `Shl`/`Shr`), in which case the left
                // value has to be held in a slot so it survives to the block
                // where the operation is emitted.
                let will_branch = self.opens_blocks(&ast::Expr::Binary(e.clone()));
                let held = self.lower_and_hold(&e.left, will_branch);
                let right = self.lower_expr(&e.right);
                let left = self.reload(held, e.left.span());

                // Civil temporal arithmetic (`date-and-time-types`) is
                // decided before `Duration`'s own dispatch: `Time + 2h`
                // carries a `Duration` operand but wraps around midnight
                // rather than producing a bare nanosecond count.
                if self.is_temporal(&e.left) || self.is_temporal(&e.right) {
                    return self.lower_temporal_binary(e, left, right, span);
                }

                // `Duration` keeps an `i64` representation but has its own
                // arithmetic rules (runtime calls for scalar multiply/divide,
                // ordinary integer add/sub, and a ratio for `Div`).
                if self.is_duration(&e.left) || self.is_duration(&e.right) {
                    return self.lower_duration_binary(e, left, right, span);
                }

                let op = binary_op(e.op);

                let left_ty = self.type_of(&e.left, e.left.span());
                let right_ty = self.type_of(&e.right, e.right.span());

                // Numeric operands are lowered to the common type the checker
                // already computed, which is recorded in `checked.expr_types`.
                // The semantic type may differ from the value's actual IR type
                // for literals that were contextually inferred (e.g. `1.0`
                // treated as `Int8`), so each operand is converted from its
                // own actual type to the common type.
                let (left, right, operand_type) = if Self::is_numeric_ir_type(left_ty)
                    && Self::is_numeric_ir_type(right_ty)
                {
                    if matches!(
                        op,
                        BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Mul
                            | BinaryOp::Div
                            | BinaryOp::Rem
                    ) {
                        let common = self
                            .common_numeric_ir_type(left_ty, right_ty)
                            .unwrap_or(left_ty);
                        let left_actual = self.type_of_operand(left);
                        let right_actual = self.type_of_operand(right);
                        let left = self.convert_numeric(left, left_actual, common, e.left.span());
                        let right =
                            self.convert_numeric(right, right_actual, common, e.right.span());
                        (left, right, common)
                    } else {
                        let left_actual = self.type_of_operand(left);
                        let right_actual = self.type_of_operand(right);
                        let left = self.convert_numeric(left, left_actual, left_ty, e.left.span());
                        let right =
                            self.convert_numeric(right, right_actual, left_ty, e.right.span());
                        (left, right, left_ty)
                    }
                } else {
                    match (left_ty, right_ty) {
                        (base, IrType::Nullable(_))
                            if op == BinaryOp::Identical
                                && !matches!(base, IrType::Nullable(_)) =>
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
                            if op == BinaryOp::Identical
                                && !matches!(base, IrType::Nullable(_)) =>
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
                    }
                };

                if operand_type == IrType::Decimal {
                    return self.lower_decimal_binary(op, left, right, span);
                }

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

            // Calling a callable value goes through the boxed callable, not a name.
            ast::Expr::Call(e) if self.is_callable_call(e) => {
                let IrType::Callable(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_callable_call`")
                };
                let branching = e.args.iter().any(|a| self.opens_blocks(&a.value));
                let held = self.lower_and_hold(&e.callee, branching);

                let expected = self.module.closures[id as usize].params.clone();
                let returns = self.module.closures[id as usize].returns;
                let args = self.lower_held_args(&e.args, &expected);
                let callee = self.reload(held, e.callee.span());
                self.emit(
                    InstKind::CallCallable {
                        callable: callee,
                        args,
                    },
                    returns,
                    span,
                )
            }

            ast::Expr::Call(e) => {
                if self.is_pointer_from_call(e) {
                    return self.lower_pointer_from(&e.args[0].value, span);
                }
                if self.is_pointer_method_call(e) {
                    return self.lower_pointer_method_call(e, span);
                }
                if self.is_weak_from_call(e) {
                    return self.lower_weak_from(&e.args[0].value, span);
                }
                if self.is_weak_method_call(e) {
                    return self.lower_weak_method_call(e, span);
                }
                if self.is_pin_call(e) {
                    return self.lower_pin_call(e, span);
                }
                if self.is_derived_clone_call(e) {
                    return self.lower_derived_clone_call(e, span);
                }
                if self.checked.temporal_constructions.contains(&e.span) {
                    return self.lower_temporal_construction(e, span);
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
                // `Outer.Nested(...)` / `o.Inner(...)`: a construction whose
                // callee is a field access. The method-call lowerers below
                // would try `Outer`/`o` as a value first — `Outer` names a
                // class, not a variable — so the checker-recorded resolution
                // is consulted before any of them.
                if let Some(&id) = self.checked.resolved_constructions.get(&e.span)
                    && matches!(&*e.callee, ast::Expr::Field(_))
                {
                    return self.lower_construction(e, id, span);
                }
                // `native-type-member-surface` statics and native methods
                // have to be lowered before any lookup that would try to
                // resolve their base name as a variable or contract method
                // (`Int32.parse(...)` / `Int32.MAX.to_string()`).
                // `enum-static-members`: `Direction.keys()`/`values()`/
                // `from_name`/`from_value` and the `Enums.*` helpers are
                // lowered ahead of the native statics for the same reason —
                // their base is a type name no value lookup can resolve.
                if let Some(operand) = self.lower_enum_static_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_native_static_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_native_to_string_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_scalar_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_string_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_char_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_duration_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_temporal_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_regex_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_regex_match_group_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_contract_call(e, span) {
                    let ty = self.type_of_operand(operand);
                    return self.lower_throws_check(operand, ty, span);
                }
                if let Some(operand) = self.lower_result_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_range_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_array_list_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_map_set_method_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_array_list_construction(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_class_static_call(e, span) {
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
                        ast::ClassKind::Record
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
            ast::Expr::Index(e) => self.lower_index_read(e, span),
            ast::Expr::Slice(e) => self.lower_slice(e, span),
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
                self.emit(
                    InstKind::ConstInt(value as i128),
                    IrType::Int(IntWidth::I32),
                    span,
                )
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

            // `unsafe { }`/`commit { }` used as a value (roadmap Phase 4e,
            // `fase-4e-unsafe-journal`), evaluated for its last expression
            // instead of only its effects — same journaling as the
            // statement form.
            ast::Expr::Unsafe(u) => self.lower_unsafe_block_value(&u.body),
            ast::Expr::Commit(c) => self.lower_commit_block_value(&c.body),
            ast::Expr::Transfer(e) => self.lower_transfer(e, span),

            // `null` has no type of its own: it only appears where a
            // destination supplies one, and `lower_expr_as` handles it there.
            ast::Expr::Tuple(tuple) => {
                let ty = self.type_of(expr, expr.span());
                let IrType::Value(layout_id) = ty else {
                    unreachable!("a tuple literal has a value layout")
                };
                let mut fields = Vec::with_capacity(tuple.elements.len());
                for element in &tuple.elements {
                    fields.push(self.lower_expr(element));
                }
                self.emit(
                    InstKind::BuildValue {
                        class: layout_id,
                        fields,
                    },
                    ty,
                    span,
                )
            }
            ast::Expr::Range(range) => self.lower_range_value(range, span),
            ast::Expr::Null(_) => {
                unreachable!("lowering received a construct the checker should have rejected")
            }
        }
    }

    /// `start..end(..step)` as a `Range<T>` value (roadmap Phase 7): each
    /// part widens to `i64` — the runtime's `ZirkRange` word — and a `step`
    /// of `0` throws `InvalidStepError` before the object exists, since such
    /// a stride could never advance.
    fn lower_range_value(&mut self, range: &ast::RangeExpr, span: Span) -> Operand {
        let i64_ty = IrType::Int(IntWidth::I64);
        let start = self.lower_expr_as(&range.start, i64_ty);
        let start = self.spill(start, i64_ty, range.start.span());
        let end = self.lower_expr_as(&range.end, i64_ty);
        let end = self.spill(end, i64_ty, range.end.span());
        let step = match &range.step {
            Some(step) => self.lower_expr_as(step, i64_ty),
            None => self.const_i64(1, span),
        };
        let step = self.spill(step, i64_ty, span);

        let step_value = self.emit(InstKind::Load(step), i64_ty, span);
        let zero = self.const_i64(0, span);
        let zero_step = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: step_value,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let keep = self.new_block();
        self.terminate(Terminator::Branch {
            condition: zero_step,
            then_block: fail,
            else_block: keep,
        });
        self.current = fail;
        let invalid_step = self
            .checked
            .native_exceptions
            .expect("a program constructing a range registered the exception hierarchy")
            .invalid_step;
        self.throw_native_failure(invalid_step, "a range's step cannot be zero", span);
        self.terminate(Terminator::Jump(keep));
        self.current = keep;

        let start = self.emit(InstKind::Load(start), i64_ty, span);
        let end = self.emit(InstKind::Load(end), i64_ty, span);
        let step = self.emit(InstKind::Load(step), i64_ty, span);
        let inclusive = self.const_i64(range.inclusive as i128, span);
        self.emit(
            InstKind::Call {
                callee: "zirk_range_new".to_string(),
                args: vec![start, end, step, inclusive],
            },
            IrType::Range,
            span,
        )
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

    /// Lowers `x == y` / `x != y` between two values of the same `record`/
    /// `record` type (`fase-3-structural-equality`, design D1): both
    /// operands are lowered once, then compared field by field through
    /// [`Self::lower_field_conjunction`]; `!=` is the same comparison
    /// negated, the same split every other `Eq`/`NotEq` lowering in this
    /// file already uses.
    fn lower_structural_equality(&mut self, expr: &ast::BinaryExpr, span: Span) -> Operand {
        let held = self.lower_and_hold(&expr.left, self.opens_blocks(&expr.right));
        let right = self.lower_expr(&expr.right);
        let left = self.reload(held, expr.left.span());

        let IrType::Value(id) = self.type_of(&expr.left, expr.left.span()) else {
            unreachable!("this arm only ever matches a `record` left operand")
        };

        let result = self.lower_field_conjunction(id, left, right, span);

        if expr.op == ast::BinaryOp::NotEq {
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

    /// ANDs together every field's own equality between two `record`/`value
    /// class` values of the same layout `id`, short-circuiting on the first
    /// difference — the same block-per-decision shape
    /// [`Self::lower_short_circuit`] uses for `&&`, generalized here from
    /// two operands to however many fields the type declares (design D1).
    ///
    /// A type with no fields (possible for a `record` with an empty field
    /// list) is trivially always equal to itself.
    fn lower_field_conjunction(
        &mut self,
        id: u32,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Operand {
        let fields = self.module.values[id as usize].fields.clone();
        if fields.is_empty() {
            return self.emit(InstKind::ConstBool(true), IrType::Boolean, span);
        }

        // Every field but the last is read from a different block than the
        // one before it (the short-circuit branch below moves the
        // insertion point) — an IR value does not cross blocks in this
        // representation, so `left`/`right` themselves are spilled to a
        // slot here and reloaded fresh at each field access, the same
        // `Self::lower_and_hold`/`reload` pattern used everywhere else an
        // earlier operand is held across a branch.
        let value_ty = IrType::Value(id);
        let left_slot = self.declare_slot("<structeq.left>", value_ty, span);
        let right_slot = self.declare_slot("<structeq.right>", value_ty, span);
        self.emit_effect(InstKind::Store(left_slot, left), span);
        self.emit_effect(InstKind::Store(right_slot, right), span);

        let result = self.declare_slot("<structeq>", IrType::Boolean, span);
        let continue_block = self.new_block();

        for (index, field) in fields.iter().enumerate() {
            let left = self.emit(InstKind::Load(left_slot), value_ty, span);
            let right = self.emit(InstKind::Load(right_slot), value_ty, span);
            let l_field = self.emit(
                InstKind::LoadField {
                    object: left,
                    index: index as u32,
                },
                field.ty,
                span,
            );
            let r_field = self.emit(
                InstKind::LoadField {
                    object: right,
                    index: index as u32,
                },
                field.ty,
                span,
            );
            let field_eq = self.lower_field_equality(field.ty, l_field, r_field, span);

            if index + 1 == fields.len() {
                // The last field decides the whole result outright, however
                // it came out — nothing left to short-circuit against.
                self.emit_effect(InstKind::Store(result, field_eq), span);
                self.terminate(Terminator::Jump(continue_block));
            } else {
                let rest_block = self.new_block();
                let false_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: field_eq,
                    then_block: rest_block,
                    else_block: false_block,
                });

                self.current = false_block;
                let false_val = self.emit(InstKind::ConstBool(false), IrType::Boolean, span);
                self.emit_effect(InstKind::Store(result, false_val), span);
                self.terminate(Terminator::Jump(continue_block));

                self.current = rest_block;
            }
        }

        self.current = continue_block;
        self.emit(InstKind::Load(result), IrType::Boolean, span)
    }

    /// One field's own equality, as part of a `record`'s
    /// derived comparison — `left`/`right` are that field already loaded
    /// from both operands.
    ///
    /// A nested `record` field recurses the same way its own
    /// top-level `==` would (design D1); a `class` reference compares by
    /// whatever the existing rule for that class's own `==` already is —
    /// its own `_equals` if declared, `is` identity otherwise (design D2,
    /// confirmed against `Checker::operator_method_of`'s own resolution —
    /// there is no error here the way a bare top-level `x == y` between two
    /// `class` instances with no `_equals` gets: the checker's own
    /// `structural_equality_unsupported_field` already lets a `class` field
    /// through unconditionally, precisely because this is what it resolves
    /// to). Every other field type reaching here (a scalar, `String`,
    /// `Char`, or a payload-less enum) is exactly what the ordinary
    /// `Binary { op: Eq }` shape already compares correctly on its own —
    /// the same instruction the pre-existing generic `Eq`/`NotEq` lowering
    /// arm emits for those types outside a `record` too.
    fn lower_field_equality(
        &mut self,
        field_ty: IrType,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Operand {
        match field_ty {
            IrType::Value(id) => self.lower_field_conjunction(id, left, right, span),
            IrType::Object(id) => {
                if let Some(method) = self.checked.classes[id as usize].method("_equals") {
                    let symbol = body_symbol(self.checked, method);
                    let returns = self.ir_type(method.returns);
                    self.emit(
                        InstKind::Call {
                            callee: symbol,
                            args: vec![left, right],
                        },
                        returns,
                        span,
                    )
                } else {
                    self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Identical,
                            left,
                            right,
                        },
                        IrType::Boolean,
                        span,
                    )
                }
            }
            _ => self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left,
                    right,
                },
                IrType::Boolean,
                span,
            ),
        }
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

    /// Lowers a lambda into a module function plus a boxed callable value.
    ///
    /// The body becomes a function whose leading parameters are the captures,
    /// and the value pairs a function pointer with a heap-allocated capture
    /// block (`MakeCallable`). The capture block lets the callable escape the
    /// frame that created it (roadmap Phase 4d, `phase-4d-callables`).
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
        // overwrites it; only the target this particular `MakeCallable`
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
            InstKind::MakeCallable {
                target: name,
                captures,
            },
            IrType::Callable(id),
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
        if !expr.acquisitions.is_empty() {
            return self.lower_grouped_match(expr, span);
        }

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
            // A regex pattern binds the `Regex.Match` its test found — the
            // `Regex.Match?` `lower_regex_pattern_test` held in a slot is
            // unwrapped here, on the path where the match exists.
            if let ast::Pattern::Regex(r) = &arm.pattern
                && let Some(binding) = &r.binding
            {
                let held = self
                    .try_lookup_slot("<regex match>")
                    .expect("a regex pattern's own test stored the match");
                let held_ty = IrType::Nullable(Nullable::Object(self.checked.regex_match_class));
                let object_ty = IrType::Object(self.checked.regex_match_class);
                let held = self.emit(InstKind::Load(held), held_ty, binding.span);
                let value = self.emit(InstKind::Unwrap(held), object_ty, binding.span);
                let slot = self.declare_slot(&binding.name, object_ty, binding.span);
                self.emit_effect(InstKind::Store(slot, value), binding.span);
            }
            // A tuple pattern destructures its elements into the names each
            // sub-pattern binds (roadmap Phase 3b).
            if let ast::Pattern::Tuple(_) = &arm.pattern {
                let current =
                    self.load_scrutinee_narrowed(scrutinee, scrutinee_type, narrowed_type, span);
                let slot = self.declare_slot("<tuple>", narrowed_type, span);
                self.emit_effect(InstKind::Store(slot, current), span);
                self.lower_pattern_bindings(&arm.pattern, slot, narrowed_type, span);
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
                let pushed_at = self.new_scope_seq();
                self.try_stack.push(TryFrame {
                    catches: Vec::new(),
                    finally: Some(finally.clone()),
                    pushed_at,
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

    /// Lowers a grouped `match ... with` by acquiring left-to-right and
    /// closing right-to-left (roadmap Phase 4c). Acquisition and close
    /// errors are merged into `ResourceFailure<BodyError,CloseError>` for
    /// the `error` branch, and each user `close()` is guarded by an
    /// `IsCancelled` check.
    fn lower_grouped_match(&mut self, expr: &ast::MatchExpr, span: Span) -> Operand {
        self.scopes.push(HashMap::new());

        let continue_block = self.new_block();
        let error_block = if expr.error.is_some() {
            self.new_block()
        } else {
            continue_block
        };

        // First pass: gather acquisition types and the unified semantic error
        // type. Every acquired resource is declared in the `match` scope before
        // any branch needs it, so `body` and failure close blocks see it.
        let mut acquired: Vec<(u32, u32, u32, IrType, SlotId, Type, String)> = Vec::new();
        let mut sema_error = Type::UNKNOWN;
        for acquisition in &expr.acquisitions {
            let result_ty = self.type_of(&acquisition.expr, acquisition.expr.span());
            let IrType::Enum(module_id) = result_ty else {
                self.scopes.pop();
                return self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
            };
            let instance =
                &self.checked.enum_instances[(module_id - self.enum_instance_base) as usize];
            if Some(instance.enum_id) != self.checked.native_result {
                self.scopes.pop();
                return self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
            }
            if sema_error.is_unknown() {
                sema_error = instance.args[1];
            }
            let r = self.ir_type(instance.args[0]);
            let e = self.ir_type(instance.args[1]);
            let ok_field = self.module.enums[module_id as usize].variants[0][0];
            let error_field = self.module.enums[module_id as usize].variants[1][0];
            let binding_slot =
                self.declare_slot(&acquisition.binding.name, r, acquisition.binding.span);
            acquired.push((
                module_id,
                ok_field,
                error_field,
                e,
                binding_slot,
                instance.args[1],
                acquisition.binding.name.clone(),
            ));
        }

        // Resolve the concrete `ResourceFailure<error_type, error_type>` layout
        // the checker interned for this grouped match.
        let failure_info = if expr.error.is_some() && !sema_error.is_unknown() {
            let failure_enum = self
                .checked
                .enums
                .iter()
                .position(|e| e.name == "ResourceFailure")
                .expect("checker registered ResourceFailure") as u32;
            let failure_instance = self
                .checked
                .enum_instances
                .iter()
                .position(|inst| {
                    inst.enum_id == failure_enum && inst.args == [sema_error, sema_error]
                })
                .expect("checker interned ResourceFailure<error_type, error_type>")
                as u32;
            let failure_module_id = self.enum_instance_base + failure_instance;
            let failure_type = IrType::Enum(failure_module_id);
            Some((failure_module_id, failure_type))
        } else {
            None
        };

        let error_slot = failure_info.map(|(_, ty)| self.declare_slot("<error>", ty, span));

        let e = acquired.first().map(|a| a.3).unwrap_or(IrType::Void);
        let body_error_slot = self.declare_slot("<body_error>", e, span);
        let close_error_slot = self.declare_slot("<close_error>", e, span);
        let close_failed_slot = self.declare_slot("<close_failed>", IrType::Boolean, span);
        let false_const = self.emit(InstKind::ConstBool(false), IrType::Boolean, span);
        self.emit_effect(InstKind::Store(close_failed_slot, false_const), span);

        // Chain the acquisitions left-to-right.  `next_success_target` is the
        // block where the next acquired resource (or the body) begins.
        let body_block = self.new_block();
        let mut next_success_target = body_block;

        for (i, (acquisition, (_module_id, ok_field, error_field, e, binding_slot, _, _))) in
            expr.acquisitions.iter().zip(acquired.iter()).enumerate()
        {
            let is_last = i == expr.acquisitions.len() - 1;

            let result_ty = self.type_of(&acquisition.expr, acquisition.expr.span());
            let value = self.lower_expr(&acquisition.expr);
            let result_slot = self.declare_slot("<result>", result_ty, span);
            self.emit_effect(InstKind::Store(result_slot, value), span);

            let object = self.emit(InstKind::Load(result_slot), result_ty, span);
            let discriminant = self.emit(
                InstKind::Discriminant(object),
                IrType::Int(IntWidth::I32),
                span,
            );
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
            let fail_block = self.new_block();
            self.terminate(Terminator::Branch {
                condition: is_ok,
                then_block: ok_block,
                else_block: fail_block,
            });

            // Success: extract the resource and store it in its binding slot.
            self.current = ok_block;
            let object = self.emit(InstKind::Load(result_slot), result_ty, span);
            let payload = self.emit(
                InstKind::LoadField {
                    object,
                    index: *ok_field,
                },
                self.slots[binding_slot.0 as usize].ty,
                span,
            );
            self.emit_effect(InstKind::Store(*binding_slot, payload), span);

            if is_last {
                self.terminate(Terminator::Jump(body_block));
            } else {
                let next_entry = self.new_block();
                self.terminate(Terminator::Jump(next_entry));
                next_success_target = next_entry;
            }

            // Failure: capture the acquisition error, close already-acquired
            // resources right-to-left with cancellation checks, and merge.
            self.current = fail_block;
            let object = self.emit(InstKind::Load(result_slot), result_ty, span);
            let err = self.emit(
                InstKind::LoadField {
                    object,
                    index: *error_field,
                },
                *e,
                span,
            );
            self.emit_effect(InstKind::Store(body_error_slot, err), span);

            for a in expr.acquisitions.iter().take(i).rev() {
                self.lower_resource_cleanup(
                    &a.binding.name,
                    close_error_slot,
                    close_failed_slot,
                    span,
                );
            }

            if let Some((failure_module_id, failure_type)) = failure_info {
                let close_failed =
                    self.emit(InstKind::Load(close_failed_slot), IrType::Boolean, span);
                let body_only_block = self.new_block();
                let body_and_close_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: close_failed,
                    then_block: body_and_close_block,
                    else_block: body_only_block,
                });

                self.current = body_only_block;
                let body_err = self.emit(InstKind::Load(body_error_slot), *e, span);
                let body_value = self.emit(
                    InstKind::BuildEnum {
                        enum_id: failure_module_id,
                        variant: 0,
                        fields: vec![body_err],
                    },
                    failure_type,
                    span,
                );
                self.emit_effect(InstKind::Store(error_slot.unwrap(), body_value), span);
                self.terminate(Terminator::Jump(error_block));

                self.current = body_and_close_block;
                let body_err = self.emit(InstKind::Load(body_error_slot), *e, span);
                let close_err = self.emit(InstKind::Load(close_error_slot), *e, span);
                let body_and_close_value = self.emit(
                    InstKind::BuildEnum {
                        enum_id: failure_module_id,
                        variant: 2,
                        fields: vec![body_err, close_err],
                    },
                    failure_type,
                    span,
                );
                self.emit_effect(
                    InstKind::Store(error_slot.unwrap(), body_and_close_value),
                    span,
                );
                self.terminate(Terminator::Jump(error_block));
            } else {
                self.terminate(Terminator::Jump(error_block));
            }

            // Move to the entry of the next acquisition, if any.
            if !is_last {
                self.current = next_success_target;
            }
        }

        // All acquisitions succeeded: run the body with a `finally` that closes
        // every acquired resource right-to-left on every exit.
        let mut finally_stmts = Vec::new();
        for a in expr.acquisitions.iter().rev() {
            let close = Self::resource_close_block(&a.binding.name, span);
            finally_stmts.extend(close.statements.clone());
        }
        let finally = Some(ast::Block {
            statements: finally_stmts,
            span,
        });

        self.current = body_block;
        let pushed_at = self.new_scope_seq();
        self.try_stack.push(TryFrame {
            catches: Vec::new(),
            finally: finally.clone(),
            pushed_at,
        });

        if let Some(body) = &expr.body {
            match body.as_ref() {
                ast::ArmBody::Expr(e) => {
                    let _ = self.lower_expr(e);
                }
                ast::ArmBody::Block(b) if self.return_type == IrType::Void => {
                    self.lower_block(b);
                }
                ast::ArmBody::Block(b) => {
                    let _ = self.lower_block_value(b);
                }
            }
        }

        self.try_stack.pop();
        if !self.is_terminated(self.current) {
            // Normal body completion: close with cancellation/result checks so a
            // close failure can be merged into `ResourceFailure.Close`.
            for a in expr.acquisitions.iter().rev() {
                self.lower_resource_cleanup(
                    &a.binding.name,
                    close_error_slot,
                    close_failed_slot,
                    span,
                );
            }
            if let Some((failure_module_id, failure_type)) = failure_info {
                let close_failed =
                    self.emit(InstKind::Load(close_failed_slot), IrType::Boolean, span);
                let close_ok_block = self.new_block();
                let close_err_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: close_failed,
                    then_block: close_err_block,
                    else_block: close_ok_block,
                });

                self.current = close_err_block;
                let close_err = self.emit(InstKind::Load(close_error_slot), e, span);
                let close_value = self.emit(
                    InstKind::BuildEnum {
                        enum_id: failure_module_id,
                        variant: 1,
                        fields: vec![close_err],
                    },
                    failure_type,
                    span,
                );
                self.emit_effect(InstKind::Store(error_slot.unwrap(), close_value), span);
                self.terminate(Terminator::Jump(error_block));

                self.current = close_ok_block;
            }
            if !self.is_terminated(self.current) {
                self.terminate(Terminator::Jump(continue_block));
            }
        }

        // Error branch: the `error` slot was already populated with a
        // `ResourceFailure` value before jumping here.
        if expr.error.is_some() {
            self.current = error_block;
            if let Some(error) = &expr.error {
                if let Some((_, failure_type)) = failure_info {
                    let error_slot_id = error_slot.unwrap();
                    self.lower_pattern_bindings(&error.pattern, error_slot_id, failure_type, span);
                }
                match &error.body {
                    ast::ArmBody::Expr(e) => {
                        let _ = self.lower_expr(e);
                    }
                    ast::ArmBody::Block(b) if self.return_type == IrType::Void => {
                        self.lower_block(b);
                    }
                    ast::ArmBody::Block(b) => {
                        let _ = self.lower_block_value(b);
                    }
                }
            }
            if !self.is_terminated(self.current) {
                self.terminate(Terminator::Jump(continue_block));
            }
        }

        self.current = continue_block;
        self.scopes.pop();

        self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span)
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
        // `re'pattern' name?` finds the regex in the `String` scrutinee; the
        // `Regex.Match?` it produces goes into a slot the arm's own binding
        // reads (`lower_match`'s `Pattern::Regex` arm), since values do not
        // cross blocks (ADR-007).
        if let ast::Pattern::Regex(r) = pattern {
            return self.lower_regex_pattern_test(
                r,
                scrutinee,
                scrutinee_type,
                narrowed_type,
                span,
            );
        }
        let expected = match pattern {
            ast::Pattern::Int(lit) => self.emit(
                InstKind::ConstInt(lit.value),
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
                self.emit(
                    InstKind::ConstInt(value as i128),
                    IrType::Int(IntWidth::I32),
                    span,
                )
            }
            // `null` tests absence rather than a value, so it is the one
            // pattern that does not compare against anything — and the one
            // test that must read the raw, still-nullable slot.
            ast::Pattern::Null(_) => {
                let value = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                return self.emit(InstKind::IsNull(value), IrType::Boolean, span);
            }
            ast::Pattern::Tuple(_) => {
                unreachable!("tuple patterns are not lowered yet")
            }
            ast::Pattern::Regex(_) => {
                unreachable!("handled above, before the comparison chain")
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

    /// `re'pattern' name?` against a `String` scrutinee: compiles the
    /// literal, calls `zirk_regex_find`, and holds the `Regex.Match?` in a
    /// slot the arm body reads when a binding follows the literal.
    fn lower_regex_pattern_test(
        &mut self,
        pattern: &ast::RegexPattern,
        scrutinee: SlotId,
        scrutinee_type: IrType,
        narrowed_type: IrType,
        span: Span,
    ) -> Operand {
        let pattern_id = self.module.intern_string(&pattern.pattern);
        let pattern_string = self.emit(InstKind::ConstString(pattern_id), IrType::String, span);
        let regex = self.emit(
            InstKind::Call {
                callee: "zirk_regex_from_pattern".to_string(),
                args: vec![pattern_string],
            },
            IrType::Regex,
            span,
        );
        let text = self.load_scrutinee_narrowed(scrutinee, scrutinee_type, narrowed_type, span);
        let found_ty = IrType::Nullable(Nullable::Object(self.checked.regex_match_class));
        let found = self.emit(
            InstKind::Call {
                callee: "zirk_regex_find".to_string(),
                args: vec![regex, text],
            },
            found_ty,
            span,
        );
        let held = self.declare_slot("<regex match>", found_ty, span);
        self.emit_effect(InstKind::Store(held, found), span);
        let found = self.emit(InstKind::Load(held), found_ty, span);
        let is_null = self.emit(InstKind::IsNull(found), IrType::Boolean, span);
        self.emit(
            InstKind::Unary {
                op: UnaryOp::Not,
                operand: is_null,
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
            ast::Pattern::Tuple(t) => {
                let IrType::Value(layout_id) = scrutinee_type else {
                    return;
                };
                let fields: Vec<_> = self.module.values[layout_id as usize]
                    .fields
                    .iter()
                    .map(|f| f.ty)
                    .collect();
                for (sub_pattern, field_ty) in t.elements.iter().zip(fields.iter()) {
                    self.declare_pattern_types(sub_pattern, *field_ty);
                }
            }
            ast::Pattern::Regex(r) => {
                if let Some(binding) = &r.binding {
                    self.declare_slot(
                        &binding.name,
                        IrType::Object(self.checked.regex_match_class),
                        binding.span,
                    );
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
        let this_slot = self.try_lookup_slot("this")?;
        let this_ty = self.slot_type(this_slot);

        // `TraitName.super.method(...)`: a direct call to the selected trait's
        // default body, with `this` as the receiver.
        if let ast::Expr::Field(field) = &*call.callee
            && let ast::Expr::Super(super_expr) = &*field.object
            && let Some(trait_name) = &super_expr.trait_name
        {
            let contract_id = self
                .checked
                .contracts
                .iter()
                .position(|c| c.name == trait_name.name)?;
            let method = self.checked.contracts[contract_id].method(&field.name.name)?;
            let name =
                contract_method_symbol(&self.checked.contracts[contract_id].name, &method.name);
            let returns = self.ir_type(method.returns);
            let params: Vec<IrType> = method.params.iter().map(|p| self.ir_type(p.ty)).collect();

            let receiver = self.emit(InstKind::Load(this_slot), this_ty, span);
            let mut args = vec![receiver];
            for (arg, ty) in call.args.iter().zip(params) {
                args.push(self.lower_expr_as(&arg.value, ty));
            }
            return Some(self.emit(InstKind::Call { callee: name, args }, returns, span));
        }

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

            let receiver = self.emit(InstKind::Load(this_slot), this_ty, span);
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

        let receiver = self.emit(InstKind::Load(this_slot), this_ty, span);
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
        // `ClassName.method(...)` names a class, not a contract value, and
        // `type_of` cannot resolve it.
        if let ast::Expr::Path(base) = &*field.object
            && self.checked.classes.iter().any(|c| c.name == base.name)
        {
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
        // `ClassName.method(...)` names a type, not a value; `type_of` cannot
        // resolve it and the static-call path handles it.
        if let ast::Expr::Path(base) = &*field.object
            && self.checked.classes.iter().any(|c| c.name == base.name)
        {
            return None;
        }
        // A record's own method is reached the same way an
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

    /// Whether `class` is, or (transitively) implements, `target` through
    /// `extends` or `implements` abstract classes.
    fn is_abstract_base_of(&self, class: u32, target: u32) -> bool {
        if class == target {
            return true;
        }
        let cls = &self.checked.classes[class as usize];
        if let Some(base) = cls.base
            && self.is_abstract_base_of(base, target)
        {
            return true;
        }
        cls.abstract_bases
            .iter()
            .any(|&b| self.is_abstract_base_of(b, target))
    }

    /// Whether `id` is a class that can be thrown/caught: it implements the
    /// compiler-known `Throwable`.
    fn is_throwable_class(&self, id: u32) -> bool {
        let Some(native) = self.checked.native_exceptions else {
            return false;
        };
        self.is_abstract_base_of(id, native.throwable)
    }

    /// Whether `id` is one of the native failure classes
    /// (`fase-4d-runtimeerror`, D9) — `DivisionByZeroError`,
    /// `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError`, and
    /// (roadmap Phase 4e, `fase-4e-native-slice`) `IndexOutOfBoundsError`/
    /// `NativeError`, all built through the checker's own
    /// `register_native_failure` closure and sharing this same "hand-built
    /// body" treatment. Used by [`Self::lower_construction`]'s own special
    /// case for a program's explicit `Type("reason")` of one of these, the
    /// same way [`Self::lower_construction`] already special-cases
    /// `StackTrace`.
    fn is_native_failure_class(&self, id: u32) -> bool {
        self.checked.native_exceptions.is_some_and(|n| {
            id == n.division_by_zero
                || id == n.invalid_shift
                || id == n.invalid_repeat
                || id == n.float_nan
                || id == n.index_out_of_bounds
                || id == n.native_error
                || id == n.invalid_step
                || id == n.parse_error
                || id == n.overflow_error
                || id == n.regex_error
                || id == n.lookup_error
                || id == n.invalid_date
                || id == n.invalid_time
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
    /// A small integer constant emitted at the requested Int width.
    fn const_int_at(&mut self, value: i128, ty: IrType, span: Span) -> Operand {
        self.emit(InstKind::ConstInt(value), ty, span)
    }

    /// Division/remainder guard: before `Div`/`Rem` over integers runs,
    /// first compares the divisor to zero and throws `DivisionByZeroError`,
    /// then (for signed widths) checks the `MIN / -1` overflow pair and
    /// throws `ArithmeticOverflowError`. Both `zirk_rt_division_by_zero`
    /// and `zirk_rt_overflow` were driven up from codegen into `zirk-ir`
    /// so a `try`/`catch` around the expression can intercept them.
    fn checked_int_division(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        operand_type: IrType,
        span: Span,
    ) -> Operand {
        let IrType::Int(width) = operand_type else {
            unreachable!("the checker only lowers integer division/remainder")
        };

        let left_slot = self.spill(left, operand_type, span);
        let right_slot = self.spill(right, operand_type, span);

        // --- zero divisor ---
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
        let zero_fail = self.new_block();
        let pre_overflow = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_zero,
            then_block: zero_fail,
            else_block: pre_overflow,
        });

        self.current = zero_fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with integer division registered the exception hierarchy");
        self.throw_native_failure(native.division_by_zero, "division by zero", span);

        // --- MIN / -1 overflow ---
        self.current = pre_overflow;
        let safe = if width.signed() {
            let left = self.emit(InstKind::Load(left_slot), operand_type, span);
            let right = self.emit(InstKind::Load(right_slot), operand_type, span);

            let one = self.const_int_at(1, operand_type, span);
            let bits_minus_one = self.const_int_at((width.bits() as i128) - 1, operand_type, span);
            let min = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Shl,
                    left: one,
                    right: bits_minus_one,
                },
                operand_type,
                span,
            );
            let minus_one = self.const_int_at(-1, operand_type, span);

            let left_is_min = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left,
                    right: min,
                },
                IrType::Boolean,
                span,
            );
            let right_is_minus_one = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left: right,
                    right: minus_one,
                },
                IrType::Boolean,
                span,
            );
            let overflows = self.emit(
                InstKind::Binary {
                    op: BinaryOp::And,
                    left: left_is_min,
                    right: right_is_minus_one,
                },
                IrType::Boolean,
                span,
            );

            let overflow_fail = self.new_block();
            let safe = self.new_block();
            self.terminate(Terminator::Branch {
                condition: overflows,
                then_block: overflow_fail,
                else_block: safe,
            });

            self.current = overflow_fail;
            self.throw_native_failure(native.arithmetic_overflow, "arithmetic overflow", span);

            safe
        } else {
            pre_overflow
        };

        self.current = safe;
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
        let bits = self.const_int_at(width.bits() as i128, amount_ty, span);

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

    /// Invalid-repeat guard (D10): before `String * Int` runs, checks the
    /// count fits in the `Int32` the runtime expects: it must be non-negative
    /// and no larger than `Int32::MAX`. Counts of any integer width are widened
    /// to `Int128` first so the bounds check is exact, then narrowed to `Int32`
    /// once it has passed.
    fn checked_repeat(&mut self, string: Operand, count: Operand, span: Span) -> Operand {
        let count_ty = self.type_of_operand(count);
        let i128_ty = IrType::Int(IntWidth::I128);
        let count = self.convert_numeric(count, count_ty, i128_ty, span);

        let string_slot = self.spill(string, IrType::String, span);
        let count_slot = self.spill(count, i128_ty, span);

        let count_reloaded = self.emit(InstKind::Load(count_slot), i128_ty, span);
        let zero = self.const_int_at(0, i128_ty, span);
        let i32_max = self.const_int_at(i32::MAX as i128, i128_ty, span);

        let is_negative = self.emit(
            InstKind::Binary {
                op: BinaryOp::Lt,
                left: count_reloaded,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let too_large = self.emit(
            InstKind::Binary {
                op: BinaryOp::Gt,
                left: count_reloaded,
                right: i32_max,
            },
            IrType::Boolean,
            span,
        );
        let invalid = self.emit(
            InstKind::Binary {
                op: BinaryOp::Or,
                left: is_negative,
                right: too_large,
            },
            IrType::Boolean,
            span,
        );

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
            .expect("a program with a string repetition registered the exception hierarchy");
        self.throw_native_failure(
            native.invalid_repeat,
            "a string can only be repeated a non-negative number of times",
            span,
        );

        self.current = cont;
        let string = self.emit(InstKind::Load(string_slot), IrType::String, span);
        let count = self.emit(InstKind::Load(count_slot), i128_ty, span);
        let count = self.convert_numeric(count, i128_ty, IrType::Int(IntWidth::I32), span);
        self.emit(InstKind::Repeat { string, count }, IrType::String, span)
    }

    /// `Date(y, m, d)` / `Time(h, m, s?, ns?)` / `DateTime(date, time)`
    /// (`date-and-time-types`): lowers the validated constructors. The
    /// calendar/range check runs in the runtime (`zirk_rt_*_is_valid`) on a
    /// branch of its own — the same shape `checked_repeat` uses — throwing
    /// `InvalidDateError`/`InvalidTimeError` on the failing side, since a
    /// `Date(2026, 2, 30)` is a controlled error, not a silent rollover.
    /// `DateTime` needs no check: its `date`/`time` arguments are already
    /// validated values.
    fn lower_temporal_construction(&mut self, call: &ast::CallExpr, span: Span) -> Operand {
        let ast::Expr::Path(callee) = &*call.callee else {
            unreachable!("the checker only records a path callee")
        };
        let native = self
            .checked
            .native_exceptions
            .expect("a program with a temporal construction registered the exception hierarchy");

        // Lower and convert every component to its runtime width, spilling
        // them so the validity branch can cross them into the continuation.
        let arg_target = match callee.name.as_str() {
            "Date" => IrType::Int(IntWidth::I32),
            "Time" => IrType::Int(IntWidth::I64),
            "DateTime" => IrType::Int(IntWidth::I64),
            _ => unreachable!("the checker only records temporal type names"),
        };
        let mut slots: Vec<(SlotId, IrType)> = Vec::new();
        for (index, arg) in call.args.iter().enumerate() {
            // `Time`'s first three components are `i32`; only `nanosecond`
            // is `i64`. `Date` is all `i32`. `DateTime`'s halves are `i64`.
            let target = match callee.name.as_str() {
                "Time" if index < 3 => IrType::Int(IntWidth::I32),
                _ => arg_target,
            };
            let value = self.lower_expr(&arg.value);
            let value_ty = self.type_of_operand(value);
            let value = self.convert_numeric(value, value_ty, target, span);
            let slot = self.spill(value, target, span);
            slots.push((slot, target));
        }
        // `Time(h, m)`'s omitted `second`/`nanosecond` are zero.
        if callee.name == "Time" {
            for index in call.args.len()..4 {
                let ty = if index < 3 {
                    IrType::Int(IntWidth::I32)
                } else {
                    IrType::Int(IntWidth::I64)
                };
                let zero = self.const_int_at(0, ty, span);
                let slot = self.spill(zero, ty, span);
                slots.push((slot, ty));
            }
        }

        if callee.name == "DateTime" {
            let days = self.emit(InstKind::Load(slots[0].0), IrType::Int(IntWidth::I64), span);
            let nanos = self.emit(InstKind::Load(slots[1].0), IrType::Int(IntWidth::I64), span);
            return self.emit(
                InstKind::Call {
                    callee: "zirk_rt_datetime_new".to_string(),
                    args: vec![days, nanos],
                },
                IrType::Int(IntWidth::I128),
                span,
            );
        }

        let (is_valid, build, failure, message) = match callee.name.as_str() {
            "Date" => (
                "zirk_rt_date_is_valid",
                "zirk_rt_date_days",
                native.invalid_date,
                "invalid date: month or day out of range",
            ),
            "Time" => (
                "zirk_rt_time_is_valid",
                "zirk_rt_time_nanos",
                native.invalid_time,
                "invalid time: component out of range",
            ),
            _ => unreachable!("handled above"),
        };
        let args: Vec<Operand> = slots
            .iter()
            .map(|&(slot, ty)| self.emit(InstKind::Load(slot), ty, span))
            .collect();
        let valid = self.emit(
            InstKind::Call {
                callee: is_valid.to_string(),
                args: args.clone(),
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: valid,
            then_block: cont,
            else_block: fail,
        });

        self.current = fail;
        self.throw_native_failure(failure, message, span);

        self.current = cont;
        // The operands handed to `is_valid` above were produced before the
        // branch — a value never crosses a block (ADR-007), so the build
        // call reloads each component from its slot.
        let args: Vec<Operand> = slots
            .iter()
            .map(|&(slot, ty)| self.emit(InstKind::Load(slot), ty, span))
            .collect();
        self.emit(
            InstKind::Call {
                callee: build.to_string(),
                args,
            },
            IrType::Int(IntWidth::I64),
            span,
        )
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

    /// Overflow guard for integer `+`, `-`, and `*` (roadmap Phase 4b):
    /// spills the operands, calls `InstKind::CheckedArithmetic` to get the
    /// `overflowed` flag, throws `ArithmeticOverflowError` on the failing
    /// side, and emits the real `Binary` operation on the other.
    fn checked_int_arithmetic(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        operand_type: IrType,
        span: Span,
    ) -> Operand {
        let IrType::Int(width) = operand_type else {
            unreachable!("the checker only lets overflow checks run over integers")
        };

        let left_slot = self.spill(left, operand_type, span);
        let right_slot = self.spill(right, operand_type, span);

        let left_reloaded = self.emit(InstKind::Load(left_slot), operand_type, span);
        let right_reloaded = self.emit(InstKind::Load(right_slot), operand_type, span);
        let overflowed = self.emit(
            InstKind::CheckedArithmetic {
                op,
                left: left_reloaded,
                right: right_reloaded,
                signed: width.signed(),
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: overflowed,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with integer arithmetic registered the exception hierarchy");
        self.throw_native_failure(native.arithmetic_overflow, "arithmetic overflow", span);

        self.current = cont;
        let left = self.emit(InstKind::Load(left_slot), operand_type, span);
        let right = self.emit(InstKind::Load(right_slot), operand_type, span);
        let result_type = op.result_type(operand_type);
        self.emit(InstKind::Binary { op, left, right }, result_type, span)
    }

    /// Emits a binary operation, guarded by whichever of the six native
    /// checks `fase-4d-runtimeerror` and `fase-4b-excepciones` moved into
    /// `zirk-ir` applies to it: division/remainder, shift and integer
    /// `+`/`-`/`*` are guarded before the operation runs; `Float` arithmetic
    /// is guarded after (`guard_nan`). Every other operator (comparison,
    /// bitwise, logical, `is`) has no native failure to guard.
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
        if matches!(
            (op, operand_type),
            (
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul,
                IrType::Int(_)
            )
        ) {
            return self.checked_int_arithmetic(op, left, right, operand_type, span);
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

        // A `throw` inside a `catch` carries the previously-caught exception as
        // its suppressed cause (`Throwable.suppressed()`).
        if stmt.value.is_some()
            && let Some((slot, ty)) = self.current_catch
        {
            let suppressed = self.emit(InstKind::Load(slot), ty, stmt.span);
            self.emit_effect(
                InstKind::SetSuppressed {
                    exception,
                    suppressed,
                },
                stmt.span,
            );
        }

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
            // `Self::lower_variant_construction` does for `Ok(a_void_call())`
            // (found via `FakeFile::close`'s own `return Ok(this.nothing())`).
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

        // A snapshot, walked from innermost to outermost by push order —
        // interleaving `try_stack` and `unsafe_stack` (roadmap Phase 4e,
        // `fase-4e-unsafe-journal`, design D2), since the two are tracked
        // independently but a propagating exception must process them in
        // their true lexical nesting order: a `try`/`catch` nested inside an
        // `unsafe {}` block that covers the exception must be tested
        // *before* that block's own journal is even considered, and must
        // stop the walk from reaching it at all if it matches.
        //
        // Crucially, this combined snapshot is also what `self.try_stack`/
        // `self.unsafe_stack` are temporarily narrowed to (to just the
        // scopes still outer than whichever one's own action is currently
        // lowering) below. Without the narrowing, a call inside a `finally`
        // (D11, `fase-4d-runtimeerror`: *every* call now runs this same
        // dispatch after it, not only a `throws`-declared one — a
        // `Resource<E>`'s own `close()` call, synthesized by
        // `Self::resource_close_block`, is exactly such a call and declares
        // no `throws` of its own) would still see this same frame — not yet
        // actually popped from the live stack at this point — as active,
        // re-enter it, and lower its `finally` again, forever: found via
        // `resources_match_with_closes_on_thrown_exception` recursing until
        // the generated program's own stack overflowed. Real exception
        // semantics agree with the narrowing regardless: a `finally` (or a
        // rollback) runs in the context of whatever is *outer* to its own
        // scope, never wrapped by that scope itself.
        enum Scope {
            Try(TryFrame),
            Unsafe(UnsafeFrame),
        }

        let mut scopes: Vec<Scope> = self
            .try_stack
            .iter()
            .cloned()
            .map(Scope::Try)
            .chain(self.unsafe_stack.iter().cloned().map(Scope::Unsafe))
            .collect();
        scopes.sort_by_key(|scope| match scope {
            Scope::Try(frame) => frame.pushed_at,
            Scope::Unsafe(frame) => frame.pushed_at,
        });

        let full_try_stack = std::mem::take(&mut self.try_stack);
        let full_unsafe_stack = std::mem::take(&mut self.unsafe_stack);

        for index in (0..scopes.len()).rev() {
            // Everything still outer than `scopes[index]` — restored before
            // its own action runs (see above).
            let mut outer_try = Vec::new();
            let mut outer_unsafe = Vec::new();
            for scope in &scopes[..index] {
                match scope {
                    Scope::Try(frame) => outer_try.push(frame.clone()),
                    Scope::Unsafe(frame) => outer_unsafe.push(frame.clone()),
                }
            }
            self.try_stack = outer_try;
            self.unsafe_stack = outer_unsafe;

            match &scopes[index] {
                Scope::Try(frame) => {
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
                    // None of this frame's own catches covers it: its
                    // `finally` still runs before the search continues
                    // further out, exactly as it would on any other exit
                    // from this `try`.
                    self.lower_finally_block(&frame.finally);
                }
                Scope::Unsafe(frame) => {
                    // A frame already durably committed by its own nested
                    // `commit {}` (design D2) has no journal left to roll
                    // back — its handle was already freed at that point.
                    if !frame.committed {
                        let journal = self.emit(
                            InstKind::Load(frame.journal_slot),
                            IrType::JournalHandle,
                            span,
                        );
                        self.emit_effect(InstKind::JournalRollback(journal), span);
                    }
                }
            }
        }
        self.try_stack = full_try_stack;
        self.unsafe_stack = full_unsafe_stack;

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
            ast::Pattern::Regex(r) => r.binding.as_ref().is_some_and(|b| b.name == name),
            _ => false,
        }
    }

    /// Synthesizes `binding.close()` as an expression node (roadmap Phase 4c).
    /// `Resource<E>`'s `close(): Result<Void,E>` is lowered through the
    /// ordinary method-call path, which resolves `binding` by its own slot in
    /// `self.scopes` — no checker-side table is consulted, since none was
    /// built for this synthetic node.
    fn resource_close_call(name: &str, span: Span) -> ast::Expr {
        let object = ast::Expr::Path(ast::Ident::new(name.to_string(), span));
        let callee = ast::Expr::Field(ast::FieldExpr {
            object: Box::new(object),
            name: ast::Ident::new("close".to_string(), span),
            safe: false,
            span,
        });
        ast::Expr::Call(ast::CallExpr {
            callee: Box::new(callee),
            args: Vec::new(),
            span,
        })
    }

    /// Synthesizes `binding.close();` as a one-statement block (roadmap
    /// Phase 4c) for `finally` clean-up paths where the result is not merged.
    fn resource_close_block(name: &str, span: Span) -> ast::Block {
        let call = Self::resource_close_call(name, span);
        ast::Block {
            statements: vec![ast::Stmt::Expr(ast::ExprStmt { expr: call, span })],
            span,
        }
    }

    /// Lowers one resource `close()` guarded by `zirk_rt_is_cancelled` and, on
    /// the first close error, stores the error in `close_error_slot` and sets
    /// `close_failed_slot`. Cancellation skips the close entirely.
    fn lower_resource_cleanup(
        &mut self,
        name: &str,
        close_error_slot: SlotId,
        close_failed_slot: SlotId,
        span: Span,
    ) {
        let cancelled = self.emit(InstKind::IsCancelled, IrType::Boolean, span);
        let do_close = self.new_block();
        let after_close = self.new_block();
        self.terminate(Terminator::Branch {
            condition: cancelled,
            then_block: after_close,
            else_block: do_close,
        });

        self.current = do_close;
        let close_call = Self::resource_close_call(name, span);
        let result = self.lower_expr(&close_call);
        let result_ty = self.type_of_operand(result);

        let IrType::Enum(result_module_id) = result_ty else {
            self.terminate(Terminator::Jump(after_close));
            return;
        };
        let error_field = self.module.enums[result_module_id as usize].variants[1][0];
        let e_ty = self.module.enums[result_module_id as usize].fields[error_field as usize].ty;

        // Spill the close result to a slot so it can be re-read after the
        // branches below (values do not cross blocks, ADR-007).
        let close_result_slot = self.declare_slot("<close_result>", result_ty, span);
        self.emit_effect(InstKind::Store(close_result_slot, result), span);

        let result_loaded = self.emit(InstKind::Load(close_result_slot), result_ty, span);
        let discriminant = self.emit(
            InstKind::Discriminant(result_loaded),
            IrType::Int(IntWidth::I32),
            span,
        );
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
        let err_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_ok,
            then_block: ok_block,
            else_block: err_block,
        });

        self.current = err_block;
        let result_loaded = self.emit(InstKind::Load(close_result_slot), result_ty, span);
        let err = self.emit(
            InstKind::LoadField {
                object: result_loaded,
                index: error_field,
            },
            e_ty,
            span,
        );
        // Spill the error to a slot so it can be stored into the first-error
        // slot across the `is_first` branch.
        let last_error_slot = self.declare_slot("<last_close_error>", e_ty, span);
        self.emit_effect(InstKind::Store(last_error_slot, err), span);

        let close_failed = self.emit(InstKind::Load(close_failed_slot), IrType::Boolean, span);
        let false_const = self.emit(InstKind::ConstBool(false), IrType::Boolean, span);
        let is_first = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: close_failed,
                right: false_const,
            },
            IrType::Boolean,
            span,
        );
        let store_block = self.new_block();
        let skip_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_first,
            then_block: store_block,
            else_block: skip_block,
        });

        self.current = store_block;
        let err = self.emit(InstKind::Load(last_error_slot), e_ty, span);
        self.emit_effect(InstKind::Store(close_error_slot, err), span);
        let true_const = self.emit(InstKind::ConstBool(true), IrType::Boolean, span);
        self.emit_effect(InstKind::Store(close_failed_slot, true_const), span);
        self.terminate(Terminator::Jump(skip_block));

        self.current = skip_block;
        self.terminate(Terminator::Jump(after_close));

        self.current = ok_block;
        self.terminate(Terminator::Jump(after_close));

        self.current = after_close;
    }

    /// Lowers the bindings introduced by the `error` branch of a grouped
    /// `match ... with` against a `ResourceFailure` value stored in `scrutinee`.
    fn lower_pattern_bindings(
        &mut self,
        pattern: &ast::Pattern,
        scrutinee: SlotId,
        scrutinee_type: IrType,
        span: Span,
    ) {
        match pattern {
            ast::Pattern::Binding(ident) => {
                let value = self.emit(InstKind::Load(scrutinee), scrutinee_type, ident.span);
                let slot = self.declare_slot(&ident.name, scrutinee_type, ident.span);
                self.emit_effect(InstKind::Store(slot, value), ident.span);
            }
            ast::Pattern::Wildcard(_) => {}
            ast::Pattern::Tuple(t) => {
                let IrType::Value(layout_id) = scrutinee_type else {
                    return;
                };
                let layout = self.module.values[layout_id as usize].clone();
                let object = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                for (sub_pattern, (index, field)) in
                    t.elements.iter().zip(layout.fields.iter().enumerate())
                {
                    let value = self.emit(
                        InstKind::LoadField {
                            object,
                            index: index as u32,
                        },
                        field.ty,
                        span,
                    );
                    match sub_pattern {
                        ast::Pattern::Binding(ident) => {
                            let slot = self.declare_slot(&ident.name, field.ty, ident.span);
                            self.emit_effect(InstKind::Store(slot, value), ident.span);
                        }
                        ast::Pattern::Wildcard(_) => {}
                        _ => {}
                    }
                }
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
                let object = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                for (sub_pattern, index) in v.bindings.iter().zip(indices) {
                    let field_ty = self.module.enums[module_id as usize].fields[index as usize].ty;
                    let value = self.emit(InstKind::LoadField { object, index }, field_ty, span);
                    match sub_pattern {
                        ast::Pattern::Binding(ident) => {
                            let slot = self.declare_slot(&ident.name, field_ty, ident.span);
                            self.emit_effect(InstKind::Store(slot, value), ident.span);
                        }
                        ast::Pattern::Wildcard(_) => {}
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn lower_finally_block(&mut self, finally: &Option<ast::Block>) {
        if let Some(block) = finally {
            self.lower_block(block);
        }
    }

    // --- `unsafe { }`/`commit { }` transactional journal (roadmap Phase 4e,
    // `fase-4e-unsafe-journal`, design D1/D2/D4) -------------------------

    /// `JournalBegin` on entry to an `unsafe { ... }` block, and pushes its
    /// own frame — shared by the statement and expression-value forms
    /// (design D1). Returns the slot the journal handle lives in, so the
    /// caller can reload it once `body` is done lowering.
    fn begin_unsafe_frame(&mut self, span: Span) -> SlotId {
        let boundary = self.slots.len() as u32;
        let journal = self.emit(InstKind::JournalBegin, IrType::JournalHandle, span);
        let journal_slot = self.declare_slot("<journal>", IrType::JournalHandle, span);
        self.emit_effect(InstKind::Store(journal_slot, journal), span);

        let pushed_at = self.new_scope_seq();
        self.unsafe_stack.push(UnsafeFrame {
            journal_slot,
            slot_boundary: boundary,
            committed: false,
            pushed_at,
        });
        journal_slot
    }

    /// Pops the innermost `unsafe` frame and, on the block's own normal
    /// fall-through path — and only if a nested `commit {}` has not already
    /// durably committed (and freed) this same journal (design D2) —
    /// durably commits it (design D1). The rollback-on-exception path never
    /// reaches here: a pending exception already diverted control away
    /// through `Self::lower_pending_exception_dispatch` before this point
    /// (see that function's own handling of `UnsafeFrame`).
    fn end_unsafe_frame(&mut self, journal_slot: SlotId, span: Span) {
        let frame = self
            .unsafe_stack
            .pop()
            .expect("begin_unsafe_frame pushed exactly one frame this call pairs with");
        debug_assert_eq!(frame.journal_slot, journal_slot);
        if !frame.committed && self.block_mut(self.current).terminator.is_none() {
            let journal = self.emit(InstKind::Load(journal_slot), IrType::JournalHandle, span);
            self.emit_effect(InstKind::JournalCommit(journal), span);
        }
    }

    /// `unsafe { ... }` in statement position.
    fn lower_unsafe_block(&mut self, block: &ast::Block) {
        let journal_slot = self.begin_unsafe_frame(block.span);
        self.lower_block(block);
        self.end_unsafe_frame(journal_slot, block.span);
    }

    /// `unsafe { ... }` in expression-value position (`mut result = unsafe { ... };`).
    fn lower_unsafe_block_value(&mut self, block: &ast::Block) -> Operand {
        let journal_slot = self.begin_unsafe_frame(block.span);
        let value = self.lower_block_value(block);
        self.end_unsafe_frame(journal_slot, block.span);
        value
    }

    /// `commit { ... }`'s own entry (design D2): durably commits the
    /// *enclosing* `unsafe` block's journal so far, before `body` itself is
    /// lowered — publishing every reversible write before the irreversible
    /// effect the commit boundary exists to guard. A second `commit {}`
    /// reaching the same already-committed frame (e.g. nested directly
    /// inside the first) is a no-op: the handle is already freed, and
    /// nothing more is left to publish.
    fn commit_enclosing_unsafe(&mut self, span: Span) {
        let frame = self
            .unsafe_stack
            .last_mut()
            .expect("the checker requires `commit {}` only inside an enclosing `unsafe {}`");
        if frame.committed {
            return;
        }
        let slot = frame.journal_slot;
        frame.committed = true;
        let journal = self.emit(InstKind::Load(slot), IrType::JournalHandle, span);
        self.emit_effect(InstKind::JournalCommit(journal), span);
    }

    /// `commit { ... }` in statement position.
    fn lower_commit_block(&mut self, block: &ast::Block) {
        self.commit_enclosing_unsafe(block.span);
        self.lower_block(block);
    }

    /// `commit { ... }` in expression-value position.
    fn lower_commit_block_value(&mut self, block: &ast::Block) -> Operand {
        self.commit_enclosing_unsafe(block.span);
        self.lower_block_value(block)
    }

    /// `transfer(expr)` (roadmap Phase 4c): lower the operand and call the
    /// runtime transfer helper, returning the same resource pointer.
    fn lower_transfer(&mut self, expr: &ast::TransferExpr, span: Span) -> Operand {
        let source = self.lower_expr(&expr.expr);
        let ty = self.type_of(&expr.expr, expr.expr.span());
        self.emit(InstKind::ResourceTransfer { source }, ty, span)
    }

    /// Before a `Store` to `slot` (design D1/D3): emits `JournalRecordSlot`
    /// against every currently open, not-yet-committed `unsafe` frame whose
    /// own boundary `slot` was declared before — a slot declared inside a
    /// given frame is exempt from *that* frame (nothing outside it could
    /// observe rolling back a write to storage the frame itself allocated),
    /// but is still journaled by any further-outer frame it may also be
    /// nested inside.
    fn journal_writes_to_slot(&mut self, slot: SlotId, span: Span) {
        let targets: Vec<SlotId> = self
            .unsafe_stack
            .iter()
            .filter(|frame| !frame.committed && slot.0 < frame.slot_boundary)
            .map(|frame| frame.journal_slot)
            .collect();
        for journal_slot in targets {
            let journal = self.emit(InstKind::Load(journal_slot), IrType::JournalHandle, span);
            self.emit_effect(InstKind::JournalRecordSlot { journal, slot }, span);
        }
    }

    /// Before a `StoreField` (design D1/D3): emits `JournalRecordField`
    /// against every currently open, not-yet-committed `unsafe` frame — a
    /// field belongs to heap state, not to this function's own lexical
    /// scoping, so (unlike a slot) there is no "declared inside the block"
    /// exemption to apply to it.
    fn journal_writes_to_field(&mut self, object: Operand, index: u32, span: Span) {
        let targets: Vec<SlotId> = self
            .unsafe_stack
            .iter()
            .filter(|frame| !frame.committed)
            .map(|frame| frame.journal_slot)
            .collect();
        for journal_slot in targets {
            let journal = self.emit(InstKind::Load(journal_slot), IrType::JournalHandle, span);
            self.emit_effect(
                InstKind::JournalRecordField {
                    journal,
                    object,
                    index,
                },
                span,
            );
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

        let pushed_at = self.new_scope_seq();
        self.try_stack.push(TryFrame {
            catches: catches.clone(),
            finally: stmt.finally.clone(),
            pushed_at,
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

    /// Whether `call` is a `ClassName.method(...)` `static fn` call.
    fn is_class_static_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        let ast::Expr::Path(base) = &*field.object else {
            return false;
        };
        self.checked
            .classes
            .iter()
            .find(|c| c.name == base.name)
            .and_then(|class| class.method(&field.name.name))
            .is_some_and(|method| method.is_static)
    }

    /// The `MethodInfo` for a `ClassName.method(...)` `static fn` call.
    fn class_static_method_of(&self, call: &ast::CallExpr) -> Option<&zirk_sema::MethodInfo> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        let ast::Expr::Path(base) = &*field.object else {
            return None;
        };
        let class = self.checked.classes.iter().find(|c| c.name == base.name)?;
        class
            .method(&field.name.name)
            .filter(|method| method.is_static)
    }

    /// `ClassName.method(...)` for a `static fn`.
    fn lower_class_static_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        let ast::Expr::Path(base) = &*field.object else {
            return None;
        };
        let class_id = self
            .checked
            .classes
            .iter()
            .position(|c| c.name == base.name)?;
        let class = self.checked.classes[class_id].clone();
        let method = class.method(&field.name.name).cloned()?;
        if !method.is_static {
            return None;
        }

        let returns = self.ir_type(method.returns);
        let expected: Vec<IrType> = method.params.iter().map(|p| self.ir_type(p.ty)).collect();
        let args = self.lower_held_args(&call.args, &expected);
        let callee = static_method_symbol(&class.name, &method.name);
        Some(self.emit(InstKind::Call { callee, args }, returns, span))
    }

    /// Lowers `value.method(...)` where the receiver is reached by contract.
    fn lower_contract_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        // `ClassName.method(...)` names a class, not a contract value, and
        // `type_of` cannot resolve it.
        if let ast::Expr::Path(base) = &*field.object
            && self.checked.classes.iter().any(|c| c.name == base.name)
        {
            return None;
        }
        let IrType::Contract(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };

        let contract = &self.checked.contracts[id as usize];
        let method = contract.method(&field.name.name)?;
        let index = method.index as u32;
        // The method's declared return type may name the contract's own
        // type parameter (`Iterator<T>.next(): Iteration<T>`) — the call
        // site knows the substitution (`it.next()` on `Iterator<Int32>`
        // produces `Iteration<Int32>`), so the checker's recorded type wins
        // whenever it recorded one.
        let returns = self
            .checked
            .expr_types
            .get(&span)
            .copied()
            .filter(|ty| !ty.is_unknown())
            .map(|ty| self.ir_type(ty))
            .unwrap_or_else(|| self.ir_type(method.returns));
        // The parameter types are similarly the concrete types the checker
        // saw at the call site, so `T` in `put(x: T)` becomes the
        // instantiated `Int32`/`String`/etc. for this receiver.
        let params: Vec<IrType> = call
            .args
            .iter()
            .map(|arg| self.type_of(&arg.value, arg.value.span()))
            .collect();

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
    /// record's own `to_string()` method already goes
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
        let receiver = self.lower_expr(&field.object);
        Some(self.lower_to_string_expr(&field.object, receiver, span))
    }

    fn lower_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }

        // `Throwable` compiler intrinsics — `stack_trace()` and `suppressed()`
        // are handled by the runtime, not by virtual dispatch on a method table.
        if let Some(operand) = self.lower_exception_intrinsic_call(call, span) {
            return Some(operand);
        }
        // A record's own method call is the same shape as an
        // ordinary object's — see `Self::method_of`. `extends` is rejected
        // for either kind, so no subclass can ever redefine one of its
        // methods: `virtual_index` below always comes out `None`, which is
        // what keeps this call direct rather than through a table a value
        // has no header to carry.
        // `Pin<T>` implicitly unpins for method calls (roadmap Phase 4e,
        // `phase-4e-memory`, design D1).
        let receiver_ty = self.type_of(&field.object, field.object.span());
        let id = match receiver_ty {
            IrType::Object(id) | IrType::Value(id) => id,
            IrType::Pin(id) => match self.module.pin_types[id as usize] {
                IrType::Object(cid) | IrType::Value(cid) => cid,
                _ => return None,
            },
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

        // Hold the receiver in a slot across the argument lowering: a later
        // argument may introduce a branch (integer overflow, `Shl`/`Shr`,
        // `Float` NaN, a call's own throws check) and the receiver would not
        // survive to the `Call` that lives on the far side of it.
        let will_branch = call.args.iter().any(|arg| self.opens_blocks(&arg.value));
        let receiver = self.lower_and_hold(&field.object, will_branch);
        let mut args = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }
        let receiver = self.reload(receiver, field.object.span());

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

    /// `Throwable.stack_trace()` and `Throwable.suppressed()` are compiler
    /// intrinsics: they are not dispatched through the object's method table,
    /// but answered by the runtime from metadata attached to the exception.
    fn exception_intrinsic_type(&self, call: &ast::CallExpr) -> Option<IrType> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !call.args.is_empty() || field.safe {
            return None;
        }
        let IrType::Object(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };
        if !self.is_throwable_class(id) {
            return None;
        }
        match field.name.name.as_str() {
            "stack_trace" => Some(IrType::String),
            "suppressed" => Some(IrType::Nullable(Nullable::Object(
                self.throwable_object_id(),
            ))),
            _ => None,
        }
    }

    /// `regex.matches(text)` and `regex.replace(text, replacement)`: built-in
    /// runtime calls.
    fn lower_regex_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("matches", 1) | ("replace", 2) | ("find", 1) | ("split", 1) | ("find_all", 1)
        ) {
            return None;
        }
        if self.type_of(&field.object, field.object.span()) != IrType::Regex {
            return None;
        }
        let receiver = self.lower_expr(&field.object);
        match field.name.name.as_str() {
            "matches" => {
                let text = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_is_match".to_string(),
                        args: vec![receiver, text],
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "replace" => {
                let text = self.lower_expr(&call.args[0].value);
                let repl = self.lower_expr(&call.args[1].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_replace".to_string(),
                        args: vec![receiver, text, repl],
                    },
                    IrType::String,
                    span,
                ))
            }
            "find" => {
                let text = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_find".to_string(),
                        args: vec![receiver, text],
                    },
                    IrType::Nullable(Nullable::Object(self.checked.regex_match_class)),
                    span,
                ))
            }
            "split" => {
                let text = self.lower_expr(&call.args[0].value);
                let list_id = self
                    .checked
                    .list_types
                    .iter()
                    .position(|&t| t == Type::STRING)
                    .expect("the checker interned `List<String>` for `Regex.split`")
                    as u32;
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_split".to_string(),
                        args: vec![receiver, text],
                    },
                    IrType::List(list_id),
                    span,
                ))
            }
            "find_all" => {
                let text = self.lower_expr(&call.args[0].value);
                let list_id = self.regex_match_list_id();
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_find_all".to_string(),
                        args: vec![receiver, text],
                    },
                    IrType::List(list_id),
                    span,
                ))
            }
            _ => unreachable!("name checked above"),
        }
    }

    /// The interned `List<Regex.Match>` id — present whenever `find_all` is
    /// reachable, since checking the call interns it.
    fn regex_match_list_id(&self) -> u32 {
        self.checked
            .list_types
            .iter()
            .position(|&t| t == Type::of(Base::Class(self.checked.regex_match_class)))
            .expect("the checker interned `List<Regex.Match>` for `Regex.find_all`") as u32
    }

    /// `Regex.Match.group(n)` / `Regex.Match.group(name)`: built-in runtime
    /// calls that re-run captures on the match's haystack.
    fn lower_regex_match_group_call(
        &mut self,
        call: &ast::CallExpr,
        span: Span,
    ) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if field.name.name != "group" || call.args.len() != 1 {
            return None;
        }
        let receiver_ty = self.type_of(&field.object, field.object.span()).unwrapped();
        let IrType::Object(id) = receiver_ty else {
            return None;
        };
        if id != self.checked.regex_match_class {
            return None;
        }

        // The runtime expects a `RegexMatch` object pointer so it can read
        // `haystack`, `start`, and `regex` without copying the object.
        let receiver = self.lower_expr(&field.object);

        let arg = self.lower_expr(&call.args[0].value);
        let arg_ty = self.type_of_operand(arg);
        let (callee, params) = if matches!(arg_ty, IrType::Int(_)) {
            ("zirk_regex_match_group_pos", vec![receiver, arg])
        } else {
            ("zirk_regex_match_group_name", vec![receiver, arg])
        };

        Some(self.emit(
            InstKind::Call {
                callee: callee.to_string(),
                args: params,
            },
            IrType::String,
            span,
        ))
    }

    /// `String` built-in methods (roadmap Phase 7, `String` ops).
    fn lower_string_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("trim", 0)
                | ("trim_start", 0)
                | ("trim_end", 0)
                | ("to_lowercase", 0)
                | ("to_uppercase", 0)
                | ("clone", 0)
                | ("is_empty", 0)
                | ("split_whitespace", 0)
                | ("lines", 0)
                | ("bytes", 0)
                | ("codepoints", 0)
                | ("chars", 0)
                | ("contains", 1)
                | ("starts_with", 1)
                | ("ends_with", 1)
                | ("substring", 2)
                | ("search", 1)
                | ("find", 1)
                | ("replace", 2)
                | ("normalize", 1)
                | ("split", 1)
        ) {
            return None;
        }
        let receiver_ty = self.type_of(&field.object, field.object.span());
        if !matches!(receiver_ty, IrType::String) {
            return None;
        }
        // The `List<T>`-returning methods get their element id from the
        // checker's own record — the extern ABI ignores it, but the IR
        // value's type is what the verifier and later passes read.
        let result_ty = |lower: &Self| {
            lower
                .checked
                .expr_types
                .get(&call.span)
                .map(|&ty| lower.ir_type(ty))
                .unwrap_or(IrType::String)
        };
        let receiver = self.lower_expr(&field.object);
        match field.name.name.as_str() {
            "trim" | "trim_start" | "trim_end" | "to_lowercase" | "to_uppercase" | "clone" => {
                Some(self.emit(
                    InstKind::Call {
                        callee: format!("zirk_str_{}", field.name.name),
                        args: vec![receiver],
                    },
                    IrType::String,
                    span,
                ))
            }
            "is_empty" => Some(self.emit(
                InstKind::Call {
                    callee: "zirk_str_is_empty".to_string(),
                    args: vec![receiver],
                },
                IrType::Boolean,
                span,
            )),
            "split_whitespace" | "lines" | "chars" | "bytes" | "codepoints" => Some(self.emit(
                InstKind::Call {
                    callee: format!("zirk_str_{}", field.name.name),
                    args: vec![receiver],
                },
                result_ty(self),
                span,
            )),
            "find" => {
                // `Int64?` — `-1` from the runtime maps to `null`.
                let arg = self.lower_expr(&call.args[0].value);
                let index = self.emit(
                    InstKind::Call {
                        callee: "zirk_str_find".to_string(),
                        args: vec![receiver, arg],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                );
                let index_slot =
                    self.declare_slot("<find_index>", IrType::Int(IntWidth::I64), span);
                self.emit_effect(InstKind::Store(index_slot, index), span);
                let minus_one = self.emit(InstKind::ConstInt(-1), IrType::Int(IntWidth::I64), span);
                let absent = self.emit(
                    InstKind::Binary {
                        op: BinaryOp::Eq,
                        left: index,
                        right: minus_one,
                    },
                    IrType::Boolean,
                    span,
                );
                let nullable = IrType::Nullable(Nullable::Int(IntWidth::I64));
                let result_slot = self.declare_slot("<find>", nullable, span);
                let null_block = self.new_block();
                let some_block = self.new_block();
                let continue_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: absent,
                    then_block: null_block,
                    else_block: some_block,
                });
                self.current = null_block;
                let null = self.emit(
                    InstKind::NullValue(Nullable::Int(IntWidth::I64)),
                    nullable,
                    span,
                );
                self.emit_effect(InstKind::Store(result_slot, null), span);
                self.terminate(Terminator::Jump(continue_block));
                self.current = some_block;
                let index = self.emit(InstKind::Load(index_slot), IrType::Int(IntWidth::I64), span);
                let wrapped = self.emit(
                    InstKind::Wrap {
                        base: Nullable::Int(IntWidth::I64),
                        value: index,
                    },
                    nullable,
                    span,
                );
                self.emit_effect(InstKind::Store(result_slot, wrapped), span);
                self.terminate(Terminator::Jump(continue_block));
                self.current = continue_block;
                Some(self.emit(InstKind::Load(result_slot), nullable, span))
            }
            "replace" => {
                let needle = self.lower_expr(&call.args[0].value);
                let repl = self.lower_expr(&call.args[1].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_replace".to_string(),
                        args: vec![receiver, needle, repl],
                    },
                    IrType::String,
                    span,
                ))
            }
            "normalize" => {
                let form = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_normalize".to_string(),
                        args: vec![receiver, form],
                    },
                    IrType::String,
                    span,
                ))
            }
            "contains" => {
                let arg = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_contains".to_string(),
                        args: vec![receiver, arg],
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "starts_with" => {
                let arg = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_starts_with".to_string(),
                        args: vec![receiver, arg],
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "ends_with" => {
                let arg = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_ends_with".to_string(),
                        args: vec![receiver, arg],
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "substring" => {
                let start = self.lower_expr_as(&call.args[0].value, IrType::Int(IntWidth::I64));
                let end = self.lower_expr_as(&call.args[1].value, IrType::Int(IntWidth::I64));
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_substring".to_string(),
                        args: vec![receiver, start, end],
                    },
                    IrType::String,
                    span,
                ))
            }
            "search" => {
                let arg = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_search".to_string(),
                        args: vec![receiver, arg],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                ))
            }
            "split" => {
                let arg = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_str_split".to_string(),
                        args: vec![receiver, arg],
                    },
                    IrType::List(self.string_list_id()),
                    span,
                ))
            }
            _ => unreachable!("name checked above"),
        }
    }

    /// The interned `List<String>` id — present whenever `split` is
    /// reachable, since checking the call interns it.
    fn string_list_id(&self) -> u32 {
        self.checked
            .list_types
            .iter()
            .position(|&t| t == Type::STRING)
            .expect("the checker interned `List<String>` for `String.split`") as u32
    }

    /// `d.abs()`/`d.sign()`/`d.is_zero()`/`d.is_positive()`/`d.is_negative()`
    /// on a `Duration` (roadmap Phase 7, `native-type-member-surface`): the
    /// receiver is a nanosecond `i64`, so predicates are plain comparisons
    /// and `abs`/`sign` are runtime calls. `abs` on `i64::MIN` traps like
    /// `Int64` arithmetic does — `ArithmeticOverflowError`, checked here in
    /// IR before the helper runs.
    fn lower_duration_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("abs", 0)
                | ("sign", 0)
                | ("is_zero", 0)
                | ("is_positive", 0)
                | ("is_negative", 0)
                | ("total_weeks", 0)
                | ("total_days", 0)
                | ("total_hours", 0)
                | ("total_minutes", 0)
                | ("total_seconds", 0)
                | ("total_milliseconds", 0)
                | ("total_microseconds", 0)
                | ("total_nanoseconds", 0)
                | ("whole_weeks", 0)
                | ("whole_days", 0)
                | ("whole_hours", 0)
                | ("whole_minutes", 0)
                | ("whole_seconds", 0)
                | ("whole_milliseconds", 0)
                | ("whole_microseconds", 0)
                | ("whole_nanoseconds", 0)
                | ("round", 1)
                | ("floor", 1)
                | ("ceil", 1)
                | ("truncate", 1)
                | ("to_iso_string", 0)
                | ("format", 1)
                | ("humanize", 2)
        ) || field.safe
        {
            return None;
        }
        let is_duration = self
            .checked
            .expr_types
            .get(&field.object.span())
            .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Duration));
        if !is_duration {
            return None;
        }
        let receiver = self.lower_expr(&field.object);
        match field.name.name.as_str() {
            "abs" => {
                let slot = self.spill(receiver, IrType::Int(IntWidth::I64), span);
                let value = self.emit(InstKind::Load(slot), IrType::Int(IntWidth::I64), span);
                let min = self.emit(
                    InstKind::ConstInt(i64::MIN as i128),
                    IrType::Int(IntWidth::I64),
                    span,
                );
                let is_min = self.emit(
                    InstKind::Binary {
                        op: BinaryOp::Eq,
                        left: value,
                        right: min,
                    },
                    IrType::Boolean,
                    span,
                );
                let trap = self.new_block();
                let ok = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: is_min,
                    then_block: trap,
                    else_block: ok,
                });
                self.current = trap;
                let native = self
                    .checked
                    .native_exceptions
                    .expect("the checker registered the exception hierarchy");
                self.throw_native_failure(
                    native.arithmetic_overflow,
                    "Duration.abs on the minimum representable duration",
                    span,
                );
                self.current = ok;
                let value = self.emit(InstKind::Load(slot), IrType::Int(IntWidth::I64), span);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_duration_abs".to_string(),
                        args: vec![value],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                ))
            }
            "sign" => Some(self.emit(
                InstKind::Call {
                    callee: "zirk_duration_sign".to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I32),
                span,
            )),
            "is_zero" | "is_positive" | "is_negative" => {
                let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I64), span);
                let op = match field.name.name.as_str() {
                    "is_zero" => BinaryOp::Eq,
                    "is_positive" => BinaryOp::Gt,
                    _ => BinaryOp::Lt,
                };
                Some(self.emit(
                    InstKind::Binary {
                        op,
                        left: receiver,
                        right: zero,
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            total if total.starts_with("total_") => Some(self.emit(
                InstKind::Call {
                    callee: format!("zirk_duration_{total}"),
                    args: vec![receiver],
                },
                IrType::Float(FloatWidth::F64),
                span,
            )),
            whole if whole.starts_with("whole_") => Some(self.emit(
                InstKind::Call {
                    callee: format!("zirk_duration_{whole}"),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I64),
                span,
            )),
            op if matches!(op, "round" | "floor" | "ceil" | "truncate") => {
                let unit = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: format!("zirk_duration_{op}"),
                        args: vec![receiver, unit],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                ))
            }
            "to_iso_string" => Some(self.emit(
                InstKind::Call {
                    callee: "zirk_duration_to_iso_string".to_string(),
                    args: vec![receiver],
                },
                IrType::String,
                span,
            )),
            "format" => {
                let template = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_duration_format".to_string(),
                        args: vec![receiver, template],
                    },
                    IrType::String,
                    span,
                ))
            }
            "humanize" => {
                let locale = self.lower_expr(&call.args[0].value);
                let max_units = self.lower_expr(&call.args[1].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_duration_humanize".to_string(),
                        args: vec![receiver, locale, max_units],
                    },
                    IrType::String,
                    span,
                ))
            }
            _ => unreachable!("name checked above"),
        }
    }

    /// Whether `call` is a `Duration` sign/magnitude method — consulted by
    /// `is_callable_call` so the callee is never type-queried as a field
    /// (a `Duration` lowers to `Int64`, which has no field layout).
    fn is_duration_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("abs", 0)
                | ("sign", 0)
                | ("is_zero", 0)
                | ("is_positive", 0)
                | ("is_negative", 0)
                | ("total_weeks", 0)
                | ("total_days", 0)
                | ("total_hours", 0)
                | ("total_minutes", 0)
                | ("total_seconds", 0)
                | ("total_milliseconds", 0)
                | ("total_microseconds", 0)
                | ("total_nanoseconds", 0)
                | ("whole_weeks", 0)
                | ("whole_days", 0)
                | ("whole_hours", 0)
                | ("whole_minutes", 0)
                | ("whole_seconds", 0)
                | ("whole_milliseconds", 0)
                | ("whole_microseconds", 0)
                | ("whole_nanoseconds", 0)
                | ("round", 1)
                | ("floor", 1)
                | ("ceil", 1)
                | ("truncate", 1)
                | ("to_iso_string", 0)
                | ("format", 1)
                | ("humanize", 2)
        ) || field.safe
        {
            return false;
        }
        self.checked
            .expr_types
            .get(&field.object.span())
            .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Duration))
    }

    /// Maps a semantic integer width to the IR's own `IntWidth`.
    fn ir_int_width(&self, width: zirk_sema::IntWidth) -> IntWidth {
        use IntWidth;
        use zirk_sema::IntWidth as S;
        match width {
            S::I8 => IntWidth::I8,
            S::I16 => IntWidth::I16,
            S::I32 => IntWidth::I32,
            S::I64 => IntWidth::I64,
            S::I128 => IntWidth::I128,
            S::U8 => IntWidth::U8,
            S::U16 => IntWidth::U16,
            S::U32 => IntWidth::U32,
            S::U64 => IntWidth::U64,
            S::U128 => IntWidth::U128,
        }
    }

    /// Maps a semantic float width to the IR's own `FloatWidth`.
    fn ir_float_width(&self, width: zirk_sema::FloatWidth) -> FloatWidth {
        use FloatWidth;
        use zirk_sema::FloatWidth as S;
        match width {
            S::F16 => FloatWidth::F16,
            S::F32 => FloatWidth::F32,
            S::F64 => FloatWidth::F64,
            S::F128 => FloatWidth::F128,
        }
    }

    /// `v.<method>(...)` on a `Date`/`Time`/`DateTime` receiver
    /// (`temporal-rich-api`): comparison queries are pure IR, `with_*`
    /// revalidates through the constructor's own guard, `start_of`/`end_of`
    /// and `format`/`parse` go through the runtime.
    fn lower_temporal_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if field.safe {
            return None;
        }
        let sema = self.checked.expr_types.get(&field.object.span()).copied()?;
        let base = sema.base;
        if !matches!(
            base,
            zirk_sema::Base::Date | zirk_sema::Base::Time | zirk_sema::Base::DateTime
        ) {
            return None;
        }
        let name = field.name.name.as_str();
        let value_ty = self.ir_type(sema);
        let i64ty = IrType::Int(IntWidth::I64);
        let i32ty = IrType::Int(IntWidth::I32);
        match name {
            "is_before" | "is_after" | "is_same" | "is_same_or_before" | "is_same_or_after"
                if call.args.len() == 1 =>
            {
                // The argument may open blocks (a `Date(...)` construction
                // validates through a branch), so the receiver is held in a
                // slot — a value never crosses a block (ADR-007).
                let receiver = self.lower_expr(&field.object);
                let receiver_slot = self.spill(receiver, value_ty, span);
                let right = self.lower_expr(&call.args[0].value);
                let right_ty = self.type_of_operand(right);
                let right = self.convert_numeric(right, right_ty, value_ty, span);
                let right_slot = self.spill(right, value_ty, span);
                let left = self.emit(InstKind::Load(receiver_slot), value_ty, span);
                let right = self.emit(InstKind::Load(right_slot), value_ty, span);
                let op = match name {
                    "is_before" => BinaryOp::Lt,
                    "is_after" => BinaryOp::Gt,
                    "is_same" => BinaryOp::Eq,
                    "is_same_or_before" => BinaryOp::LtEq,
                    _ => BinaryOp::GtEq,
                };
                Some(self.emit(InstKind::Binary { op, left, right }, IrType::Boolean, span))
            }
            "is_between" if call.args.len() == 2 => {
                // Same slot discipline: each operand can end in a different
                // post-guard block, so all three cross through slots.
                let receiver = self.lower_expr(&field.object);
                let receiver_slot = self.spill(receiver, value_ty, span);
                let lo = self.lower_expr(&call.args[0].value);
                let lo = self.convert_numeric(lo, self.type_of_operand(lo), value_ty, span);
                let lo_slot = self.spill(lo, value_ty, span);
                let hi = self.lower_expr(&call.args[1].value);
                let hi = self.convert_numeric(hi, self.type_of_operand(hi), value_ty, span);
                let hi_slot = self.spill(hi, value_ty, span);
                let receiver = self.emit(InstKind::Load(receiver_slot), value_ty, span);
                let lo = self.emit(InstKind::Load(lo_slot), value_ty, span);
                let hi = self.emit(InstKind::Load(hi_slot), value_ty, span);
                let ge_lo = self.emit(
                    InstKind::Binary {
                        op: BinaryOp::GtEq,
                        left: receiver,
                        right: lo,
                    },
                    IrType::Boolean,
                    span,
                );
                let le_hi = self.emit(
                    InstKind::Binary {
                        op: BinaryOp::LtEq,
                        left: receiver,
                        right: hi,
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.emit(
                    InstKind::Binary {
                        op: BinaryOp::And,
                        left: ge_lo,
                        right: le_hi,
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "is_weekday" | "is_weekend"
                if call.args.is_empty()
                    && matches!(base, zirk_sema::Base::Date | zirk_sema::Base::DateTime) =>
            {
                let receiver = self.lower_expr(&field.object);
                let days = if matches!(base, zirk_sema::Base::DateTime) {
                    self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_datetime_days".to_string(),
                            args: vec![receiver],
                        },
                        i64ty,
                        span,
                    )
                } else {
                    receiver
                };
                let dow = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_date_day_of_week".to_string(),
                        args: vec![days],
                    },
                    i32ty,
                    span,
                );
                let (threshold, op) = if name == "is_weekday" {
                    (5, BinaryOp::LtEq)
                } else {
                    (6, BinaryOp::GtEq)
                };
                let bound = self.emit(InstKind::ConstInt(threshold), i32ty, span);
                Some(self.emit(
                    InstKind::Binary {
                        op,
                        left: dow,
                        right: bound,
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "with_date" | "with_time"
                if call.args.len() == 1 && matches!(base, zirk_sema::Base::DateTime) =>
            {
                let dt = self.lower_expr(&field.object);
                let dt_slot = self.spill(dt, value_ty, span);
                let arg = self.lower_expr(&call.args[0].value);
                let dt = self.emit(InstKind::Load(dt_slot), value_ty, span);
                let (days, nanos) = if name == "with_date" {
                    let nanos = self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_datetime_nanos".to_string(),
                            args: vec![dt],
                        },
                        i64ty,
                        span,
                    );
                    (arg, nanos)
                } else {
                    let days = self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_datetime_days".to_string(),
                            args: vec![dt],
                        },
                        i64ty,
                        span,
                    );
                    (days, arg)
                };
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_datetime_new".to_string(),
                        args: vec![days, nanos],
                    },
                    IrType::Int(IntWidth::I128),
                    span,
                ))
            }
            "with_year" | "with_month" | "with_day" | "with_hour" | "with_minute"
            | "with_second" | "with_nanosecond"
                if call.args.len() == 1 =>
            {
                Some(self.lower_temporal_with(call, field, base, name, span))
            }
            "start_of" | "end_of" if call.args.len() == 1 => {
                let kind = match base {
                    zirk_sema::Base::Date => "date",
                    zirk_sema::Base::Time => "time",
                    _ => "datetime",
                };
                let receiver = self.lower_expr(&field.object);
                let receiver_slot = self.spill(receiver, value_ty, span);
                let unit = self.lower_expr(&call.args[0].value);
                let unit_slot = self.spill(unit, IrType::String, span);
                let unit = self.emit(InstKind::Load(unit_slot), IrType::String, span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: format!("zirk_rt_{kind}_{name}_ok"),
                        args: vec![unit],
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let value = lower.emit(InstKind::Load(receiver_slot), value_ty, span);
                        let unit = lower.emit(InstKind::Load(unit_slot), IrType::String, span);
                        lower.emit(
                            InstKind::Call {
                                callee: format!("zirk_rt_{kind}_{name}_value"),
                                args: vec![value, unit],
                            },
                            value_ty,
                            span,
                        )
                    },
                    "unknown temporal unit",
                ))
            }
            "format" if call.args.len() == 1 => {
                let kind = match base {
                    zirk_sema::Base::Date => "date",
                    zirk_sema::Base::Time => "time",
                    _ => "datetime",
                };
                let receiver = self.lower_expr(&field.object);
                let receiver_slot = self.spill(receiver, value_ty, span);
                let pattern = self.lower_expr(&call.args[0].value);
                let receiver = self.emit(InstKind::Load(receiver_slot), value_ty, span);
                Some(self.emit(
                    InstKind::Call {
                        callee: format!("zirk_rt_{kind}_format"),
                        args: vec![receiver, pattern],
                    },
                    IrType::String,
                    span,
                ))
            }
            "to_iso_string" if call.args.is_empty() => {
                let kind = match base {
                    zirk_sema::Base::Date => "date",
                    zirk_sema::Base::Time => "time",
                    _ => "datetime",
                };
                let receiver = self.lower_expr(&field.object);
                Some(self.emit(
                    InstKind::Call {
                        callee: format!("zirk_rt_{kind}_to_string"),
                        args: vec![receiver],
                    },
                    IrType::String,
                    span,
                ))
            }
            _ => None,
        }
    }

    /// `d.with_month(m)` and friends (`temporal-rich-api`): the components
    /// are projected, the named one is replaced by the argument, and the
    /// triple/quadruple revalidates through the same guarded build the
    /// `Date`/`Time` constructors use — a `with_*` that would land on an
    /// invalid day throws `InvalidDateError`/`InvalidTimeError`, never
    /// clamps.
    fn lower_temporal_with(
        &mut self,
        call: &ast::CallExpr,
        field: &ast::FieldExpr,
        base: zirk_sema::Base,
        name: &str,
        span: Span,
    ) -> Operand {
        let native = self
            .checked
            .native_exceptions
            .expect("a program with `with_*` registered the exception hierarchy");
        let i64ty = IrType::Int(IntWidth::I64);
        let i32ty = IrType::Int(IntWidth::I32);
        let receiver = self.lower_expr(&field.object);
        // The argument may open blocks (`d.with_month(f(x))`), so the
        // receiver crosses through a slot.
        let receiver_slot = self.spill(receiver, self.type_of_operand(receiver), span);
        let arg = self.lower_expr(&call.args[0].value);
        let arg_slot = self.spill(arg, self.type_of_operand(arg), span);
        let arg = self.emit(InstKind::Load(arg_slot), self.type_of_operand(arg), span);
        let arg_ty = self.type_of_operand(arg);
        let receiver = self.emit(
            InstKind::Load(receiver_slot),
            self.type_of_operand(receiver),
            span,
        );

        // `dt.with_month(m)` works on the halves: rebuild the touched half,
        // keep the other.
        let (days_value, nanos_value) = if matches!(base, zirk_sema::Base::DateTime) {
            let days = self.emit(
                InstKind::Call {
                    callee: "zirk_rt_datetime_days".to_string(),
                    args: vec![receiver],
                },
                i64ty,
                span,
            );
            let nanos = self.emit(
                InstKind::Call {
                    callee: "zirk_rt_datetime_nanos".to_string(),
                    args: vec![receiver],
                },
                i64ty,
                span,
            );
            (Some(days), Some(nanos))
        } else {
            (None, None)
        };
        let date_half = matches!(base, zirk_sema::Base::Date) || days_value.is_some();
        let time_half = matches!(base, zirk_sema::Base::Time) || nanos_value.is_some();
        // The half that is *not* rebuilt still feeds the final
        // `datetime_new` — which runs after the rebuild's validity branch.
        // Values never cross blocks (ADR-007), so both halves are held in
        // slots and reloaded where they are combined.
        let days_slot = days_value.map(|v| self.spill(v, i64ty, span));
        let nanos_slot = nanos_value.map(|v| self.spill(v, i64ty, span));

        // `None` means "the untouched half" — it is resolved at the
        // combine, after whichever guard opened blocks (ADR-007).
        let rebuilt_days: Option<Operand> =
            if date_half && matches!(name, "with_year" | "with_month" | "with_day") {
                let days = days_value.unwrap_or(receiver);
                let y = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_date_year".to_string(),
                        args: vec![days],
                    },
                    i32ty,
                    span,
                );
                let mo = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_date_month".to_string(),
                        args: vec![days],
                    },
                    i32ty,
                    span,
                );
                let d = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_date_day".to_string(),
                        args: vec![days],
                    },
                    i32ty,
                    span,
                );
                let arg = self.convert_numeric(arg, arg_ty, i32ty, span);
                let (y, mo, d) = match name {
                    "with_year" => (arg, mo, d),
                    "with_month" => (y, arg, d),
                    _ => (y, mo, arg),
                };
                Some(self.temporal_guarded_build(
                    vec![y, mo, d],
                    "zirk_rt_date_is_valid",
                    "zirk_rt_date_days",
                    native.invalid_date,
                    "invalid date: month or day out of range",
                    i64ty,
                    span,
                ))
            } else {
                None
            };

        let rebuilt_nanos: Option<Operand> = if time_half
            && matches!(
                name,
                "with_hour" | "with_minute" | "with_second" | "with_nanosecond"
            ) {
            let nanos = nanos_value.unwrap_or(receiver);
            let h = self.emit(
                InstKind::Call {
                    callee: "zirk_rt_time_hour".to_string(),
                    args: vec![nanos],
                },
                i32ty,
                span,
            );
            let mi = self.emit(
                InstKind::Call {
                    callee: "zirk_rt_time_minute".to_string(),
                    args: vec![nanos],
                },
                i32ty,
                span,
            );
            let s = self.emit(
                InstKind::Call {
                    callee: "zirk_rt_time_second".to_string(),
                    args: vec![nanos],
                },
                i32ty,
                span,
            );
            let ns = self.emit(
                InstKind::Call {
                    callee: "zirk_rt_time_nanosecond".to_string(),
                    args: vec![nanos],
                },
                i64ty,
                span,
            );
            let (h, mi, s, ns) = if name == "with_nanosecond" {
                let arg = self.convert_numeric(arg, arg_ty, i64ty, span);
                (h, mi, s, arg)
            } else {
                let arg = self.convert_numeric(arg, arg_ty, i32ty, span);
                match name {
                    "with_hour" => (arg, mi, s, ns),
                    "with_minute" => (h, arg, s, ns),
                    _ => (h, mi, arg, ns),
                }
            };
            Some(self.temporal_guarded_build(
                vec![h, mi, s, ns],
                "zirk_rt_time_is_valid",
                "zirk_rt_time_nanos",
                native.invalid_time,
                "invalid time: component out of range",
                i64ty,
                span,
            ))
        } else {
            None
        };

        if matches!(base, zirk_sema::Base::DateTime) {
            // The untouched half reloads from its slot — here, in the
            // post-guard block.
            let days = match rebuilt_days {
                Some(v) => v,
                None => self.emit(
                    InstKind::Load(days_slot.expect("a DateTime has its day half")),
                    i64ty,
                    span,
                ),
            };
            let nanos = match rebuilt_nanos {
                Some(v) => v,
                None => self.emit(
                    InstKind::Load(nanos_slot.expect("a DateTime has its clock half")),
                    i64ty,
                    span,
                ),
            };
            self.emit(
                InstKind::Call {
                    callee: "zirk_rt_datetime_new".to_string(),
                    args: vec![days, nanos],
                },
                IrType::Int(IntWidth::I128),
                span,
            )
        } else if matches!(base, zirk_sema::Base::Time) {
            rebuilt_nanos.expect("a `with_*` on Time always rebuilds the clock")
        } else {
            rebuilt_days.expect("a `with_*` on Date always rebuilds the day")
        }
    }

    /// The guarded component build shared by `with_*` and the temporal
    /// constructors: spill the components, ask the runtime `is_valid`,
    /// throw the native failure on the false branch, build the value on the
    /// true one. Operands never cross blocks (ADR-007), so each side
    /// reloads from its slot.
    #[allow(clippy::too_many_arguments)]
    fn temporal_guarded_build(
        &mut self,
        parts: Vec<Operand>,
        is_valid: &str,
        build: &str,
        failure: u32,
        message: &str,
        returns: IrType,
        span: Span,
    ) -> Operand {
        let slots: Vec<(SlotId, IrType)> = parts
            .iter()
            .map(|&part| {
                let ty = self.type_of_operand(part);
                (self.spill(part, ty, span), ty)
            })
            .collect();
        let load =
            |lower: &mut Self, slot: SlotId, ty: IrType| lower.emit(InstKind::Load(slot), ty, span);
        let args: Vec<Operand> = slots
            .iter()
            .map(|&(slot, ty)| load(self, slot, ty))
            .collect();
        let valid = self.emit(
            InstKind::Call {
                callee: is_valid.to_string(),
                args,
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: valid,
            then_block: cont,
            else_block: fail,
        });

        self.current = fail;
        self.throw_native_failure(failure, message, span);

        self.current = cont;
        let args: Vec<Operand> = slots
            .iter()
            .map(|&(slot, ty)| load(self, slot, ty))
            .collect();
        self.emit(
            InstKind::Call {
                callee: build.to_string(),
                args,
            },
            returns,
            span,
        )
    }

    /// `v.<method>(...)` on an `Int`/`Float` receiver
    /// (`native-type-member-surface`): the documented scalar member surface.
    /// Integers travel to the runtime helpers as `i128` (every width fits),
    /// with `bits`/`signed` constants carrying the receiver's own width
    /// semantics; results come back `i128` and `IntCast` narrows them.
    /// Floats travel as `f64` with `FloatCast` around the call.
    fn lower_scalar_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if field.safe {
            return None;
        }
        let sema = self.checked.expr_types.get(&field.object.span()).copied()?;
        let name = field.name.name.as_str();
        match sema.base {
            zirk_sema::Base::Int(width) => {
                let width = self.ir_int_width(width);
                self.lower_int_method_call(call, field, width, name, span)
            }
            zirk_sema::Base::Float(width) => {
                let width = self.ir_float_width(width);
                self.lower_float_method_call(call, field, width, name, span)
            }
            zirk_sema::Base::Decimal => self.lower_decimal_method_call(call, field, name, span),
            _ => None,
        }
    }

    /// Exact base-ten `Float` receiver methods — see `lower_scalar_method_call`.
    fn lower_decimal_method_call(
        &mut self,
        call: &ast::CallExpr,
        field: &ast::FieldExpr,
        name: &str,
        span: Span,
    ) -> Option<Operand> {
        let dec = IrType::Decimal;
        let receiver = self.lower_expr(&field.object);

        let unary = |this: &mut Self, callee: &str, ret: IrType| {
            this.emit(
                InstKind::Call {
                    callee: callee.to_string(),
                    args: vec![receiver],
                },
                ret,
                span,
            )
        };

        Some(match (name, call.args.len()) {
            ("abs", 0) => unary(self, "zirk_rt_decimal_abs", dec),
            ("floor", 0) => unary(self, "zirk_rt_decimal_floor", dec),
            ("ceil", 0) => unary(self, "zirk_rt_decimal_ceil", dec),
            ("truncate", 0) => unary(self, "zirk_rt_decimal_truncate", dec),
            ("fraction", 0) => unary(self, "zirk_rt_decimal_fraction", dec),
            ("sqrt", 0) => unary(self, "zirk_rt_decimal_sqrt", dec),
            ("sign", 0) => unary(self, "zirk_rt_decimal_sign", IrType::Int(IntWidth::I32)),
            ("scale", 0) => unary(self, "zirk_rt_decimal_scale", IrType::Int(IntWidth::I32)),
            ("is_zero", 0) => unary(self, "zirk_rt_decimal_is_zero", IrType::Boolean),
            ("is_negative", 0) => unary(self, "zirk_rt_decimal_is_negative", IrType::Boolean),
            ("is_integer", 0) => unary(self, "zirk_rt_decimal_is_integer", IrType::Boolean),
            ("round", 0) => {
                let zero = self.const_int_at(0, IrType::Int(IntWidth::I32), span);
                let mode = self.const_int_at(0, IrType::Int(IntWidth::I32), span);
                self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_round".to_string(),
                        args: vec![receiver, zero, mode],
                    },
                    dec,
                    span,
                )
            }
            ("round", 1) => {
                let places = self.lower_decimal_place_arg(&call.args[0].value, span);
                let mode = self.const_int_at(0, IrType::Int(IntWidth::I32), span);
                self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_round".to_string(),
                        args: vec![receiver, places, mode],
                    },
                    dec,
                    span,
                )
            }
            ("pow", 1)
                if self
                    .checked
                    .expr_types
                    .get(&call.args[0].value.span())
                    .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Int(_))) =>
            {
                // An integer exponent takes the exact repeated-multiply path
                // (`zirk_rt_decimal_pow_i`), not the f64-precision
                // `zirk_rt_decimal_pow`. This is what makes `(1.5) ** 2`
                // exactly `2.25` and `2 ** -3` exactly `0.125`.
                //
                // Spill the base first: the exponent may be a `**` whose
                // lowering splits blocks, and values do not cross blocks.
                let base_slot = self.spill(receiver, dec, span);
                let raw = self.lower_expr(&call.args[0].value);
                let raw_ty = self.type_of_operand(raw);
                let n = if raw_ty == IrType::Int(IntWidth::I64) {
                    raw
                } else {
                    self.emit(InstKind::IntCast(raw), IrType::Int(IntWidth::I64), span)
                };
                let base = self.emit(InstKind::Load(base_slot), dec, span);
                self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_pow_i".to_string(),
                        args: vec![base, n],
                    },
                    dec,
                    span,
                )
            }
            ("min" | "max" | "pow", 1) => {
                let other = self.lower_decimal_arg(&call.args[0].value, span);
                let callee = match name {
                    "min" => "zirk_rt_decimal_min",
                    "max" => "zirk_rt_decimal_max",
                    _ => "zirk_rt_decimal_pow",
                };
                self.emit(
                    InstKind::Call {
                        callee: callee.to_string(),
                        args: vec![receiver, other],
                    },
                    dec,
                    span,
                )
            }
            ("clamp", 2) => {
                let low = self.lower_decimal_arg(&call.args[0].value, span);
                let high = self.lower_decimal_arg(&call.args[1].value, span);
                self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_clamp".to_string(),
                        args: vec![receiver, low, high],
                    },
                    dec,
                    span,
                )
            }
            ("div", 1) => {
                let other = self.lower_decimal_arg(&call.args[0].value, span);
                self.decimal_guarded_div(receiver, other, None, span)
            }
            ("div", 2) => {
                let other = self.lower_decimal_arg(&call.args[0].value, span);
                let places = self.lower_decimal_place_arg(&call.args[1].value, span);
                self.decimal_guarded_div(receiver, other, Some(places), span)
            }
            _ => return None,
        })
    }

    /// Lowers a `Float`-typed argument, converting an integer literal/value.
    fn lower_decimal_arg(&mut self, expr: &ast::Expr, span: Span) -> Operand {
        let value = self.lower_expr(expr);
        let actual = self.type_of_operand(value);
        self.convert_numeric(value, actual, IrType::Decimal, span)
    }

    /// Lowers a place-count argument to `Int32`.
    fn lower_decimal_place_arg(&mut self, expr: &ast::Expr, span: Span) -> Operand {
        let value = self.lower_expr(expr);
        let actual = self.type_of_operand(value);
        self.convert_numeric(value, actual, IrType::Int(IntWidth::I32), span)
    }

    /// `a.div(b[, places])` with the same zero-divisor guard `/` gets.
    fn decimal_guarded_div(
        &mut self,
        left: Operand,
        right: Operand,
        places: Option<Operand>,
        span: Span,
    ) -> Operand {
        let dec = IrType::Decimal;
        let left_slot = self.spill(left, dec, span);
        let right_slot = self.spill(right, dec, span);
        let places_slot = places.map(|p| self.spill(p, IrType::Int(IntWidth::I32), span));

        let divisor = self.emit(InstKind::Load(right_slot), dec, span);
        let is_zero = self.emit(
            InstKind::Call {
                callee: "zirk_rt_decimal_is_zero".to_string(),
                args: vec![divisor],
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let ok = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_zero,
            then_block: fail,
            else_block: ok,
        });
        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with division registered the exception hierarchy");
        self.throw_native_failure(native.division_by_zero, "division by zero", span);
        self.current = ok;

        let left = self.emit(InstKind::Load(left_slot), dec, span);
        let right = self.emit(InstKind::Load(right_slot), dec, span);
        match places_slot {
            None => self.emit(
                InstKind::Call {
                    callee: "zirk_rt_decimal_div".to_string(),
                    args: vec![left, right],
                },
                dec,
                span,
            ),
            Some(slot) => {
                let places = self.emit(InstKind::Load(slot), IrType::Int(IntWidth::I32), span);
                let mode = self.const_int_at(0, IrType::Int(IntWidth::I32), span);
                self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_div_ex".to_string(),
                        args: vec![left, right, mode, places],
                    },
                    dec,
                    span,
                )
            }
        }
    }

    /// Whether `call` is a native `Int`/`Float` member — the `Duration`
    /// receiver shares `Int64`'s IR type, so the semantic type answers.
    fn is_scalar_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if field.safe {
            return false;
        }
        const INT0: &[&str] = &[
            "abs",
            "sign",
            "bit_count",
            "leading_zeros",
            "trailing_zeros",
            "is_zero",
            "is_even",
            "is_odd",
        ];
        const INT1: &[&str] = &[
            "min",
            "max",
            "rotate_left",
            "rotate_right",
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "saturating_add",
            "saturating_sub",
            "saturating_mul",
            "checked_add",
            "checked_sub",
            "checked_mul",
            "checked_div",
            "checked_rem",
            "checked_pow",
            "pow",
        ];
        const FLOAT0: &[&str] = &[
            "abs",
            "sign",
            "is_zero",
            "floor",
            "ceil",
            "round",
            "truncate",
            "fraction",
            "is_finite",
            "is_infinite",
            "is_negative",
            "sqrt",
        ];
        const FLOAT1: &[&str] = &["min", "max", "pow"];
        let Some(sema) = self.checked.expr_types.get(&field.object.span()) else {
            return false;
        };
        let name = field.name.name.as_str();
        match sema.base {
            zirk_sema::Base::Int(_) => {
                (INT0.contains(&name) && call.args.is_empty())
                    || (INT1.contains(&name) && call.args.len() == 1)
                    || (name == "clamp" && call.args.len() == 2)
                    || (name == "to_string"
                        && call.args.len() == 1
                        && call.args[0]
                            .name
                            .as_ref()
                            .is_some_and(|n| n.name == "radix"))
            }
            zirk_sema::Base::Float(_) => {
                (FLOAT0.contains(&name) && call.args.is_empty())
                    || (FLOAT1.contains(&name) && call.args.len() == 1)
                    || (name == "clamp" && call.args.len() == 2)
                    || (name == "format" && call.args.len() == 1)
            }
            zirk_sema::Base::Decimal => {
                const DEC0: &[&str] = &[
                    "abs",
                    "floor",
                    "ceil",
                    "truncate",
                    "fraction",
                    "sqrt",
                    "sign",
                    "scale",
                    "is_zero",
                    "is_negative",
                    "is_integer",
                    "round",
                ];
                const DEC1: &[&str] = &["min", "max", "pow", "round", "div"];
                (DEC0.contains(&name) && call.args.is_empty())
                    || (DEC1.contains(&name) && call.args.len() == 1)
                    || (name == "clamp" && call.args.len() == 2)
                    || (name == "div" && call.args.len() == 2)
            }
            _ => false,
        }
    }

    /// Widens `operand` (any `Int` width) to `i128` for the scalar helpers.
    fn int_to_i128(&mut self, operand: Operand, from: IrType, span: Span) -> Operand {
        if from == IrType::Int(IntWidth::I128) {
            operand
        } else {
            self.emit(
                InstKind::IntCast(operand),
                IrType::Int(IntWidth::I128),
                span,
            )
        }
    }

    /// Narrows an `i128` helper result back to the receiver's width.
    fn int_from_i128(&mut self, operand: Operand, to: IntWidth, span: Span) -> Operand {
        if to == IntWidth::I128 {
            operand
        } else {
            self.emit(InstKind::IntCast(operand), IrType::Int(to), span)
        }
    }

    /// `Int32`/`UInt` receiver methods — see `lower_scalar_method_call`.
    fn lower_int_method_call(
        &mut self,
        call: &ast::CallExpr,
        field: &ast::FieldExpr,
        width: IntWidth,
        name: &str,
        span: Span,
    ) -> Option<Operand> {
        let receiver_ir = IrType::Int(width);
        let bits = width.bits() as i32;
        let signed = if width.signed() { 1 } else { 0 };
        let receiver = self.lower_expr(&field.object);
        let wide = self.int_to_i128(receiver, receiver_ir, span);
        let bits_op = self.emit(
            InstKind::ConstInt(bits as i128),
            IrType::Int(IntWidth::I32),
            span,
        );
        let signed_op = self.emit(
            InstKind::ConstInt(signed as i128),
            IrType::Int(IntWidth::I32),
            span,
        );
        let arg_i128 = |lower: &mut Self, arg: &ast::Expr| {
            let raw = lower.lower_expr(arg);
            let raw_ty = lower.type_of_operand(raw);
            lower.int_to_i128(raw, raw_ty, span)
        };
        let call_i128 = |lower: &mut Self, callee: &str, extra: Vec<Operand>| {
            let mut args = vec![wide];
            args.extend(extra);
            args.push(bits_op);
            args.push(signed_op);
            lower.emit(
                InstKind::Call {
                    callee: callee.to_string(),
                    args,
                },
                IrType::Int(IntWidth::I128),
                span,
            )
        };
        match name {
            "abs" if call.args.is_empty() => {
                // `MIN.abs()` traps like `Int64` arithmetic — checked in IR
                // before the helper runs (same shape as `Duration.abs`).
                if width.signed() {
                    let min = self.emit(
                        InstKind::ConstInt(-(1i128 << (bits - 1))),
                        receiver_ir,
                        span,
                    );
                    let at_min = self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Eq,
                            left: receiver,
                            right: min,
                        },
                        IrType::Boolean,
                        span,
                    );
                    let wide_slot = self.spill(wide, IrType::Int(IntWidth::I128), span);
                    let trap = self.new_block();
                    let ok = self.new_block();
                    self.terminate(Terminator::Branch {
                        condition: at_min,
                        then_block: trap,
                        else_block: ok,
                    });
                    self.current = trap;
                    let native = self
                        .checked
                        .native_exceptions
                        .expect("the checker registered the exception hierarchy");
                    self.throw_native_failure(
                        native.arithmetic_overflow,
                        "abs on the minimum representable value",
                        span,
                    );
                    self.current = ok;
                    let wide =
                        self.emit(InstKind::Load(wide_slot), IrType::Int(IntWidth::I128), span);
                    let bits_op = self.emit(
                        InstKind::ConstInt(bits as i128),
                        IrType::Int(IntWidth::I32),
                        span,
                    );
                    let signed_op = self.emit(
                        InstKind::ConstInt(signed as i128),
                        IrType::Int(IntWidth::I32),
                        span,
                    );
                    let result = self.emit(
                        InstKind::Call {
                            callee: "zirk_int_abs".to_string(),
                            args: vec![wide, bits_op, signed_op],
                        },
                        IrType::Int(IntWidth::I128),
                        span,
                    );
                    return Some(self.int_from_i128(result, width, span));
                }
                let result = call_i128(self, "zirk_int_abs", vec![]);
                Some(self.int_from_i128(result, width, span))
            }
            "sign" | "bit_count" | "leading_zeros" | "trailing_zeros" if call.args.is_empty() => {
                let callee = match name {
                    "sign" => "zirk_int_sign",
                    _ => &format!("zirk_int_{name}"),
                };
                // `bit_count`/`leading_zeros`/`trailing_zeros` take no
                // `signed` parameter — the helpers' own signatures differ,
                // so they are emitted directly rather than through
                // `call_i128`.
                let args = if name == "sign" {
                    vec![wide, bits_op, signed_op]
                } else {
                    vec![wide, bits_op]
                };
                Some(self.emit(
                    InstKind::Call {
                        callee: callee.to_string(),
                        args,
                    },
                    IrType::Int(IntWidth::I32),
                    span,
                ))
            }
            "is_zero" | "is_even" | "is_odd" if call.args.is_empty() => Some(self.emit(
                InstKind::Call {
                    callee: format!("zirk_int_{name}"),
                    args: vec![wide],
                },
                IrType::Boolean,
                span,
            )),
            "min" | "max" | "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "saturating_add"
            | "saturating_sub" | "saturating_mul"
                if call.args.len() == 1 =>
            {
                let other = arg_i128(self, &call.args[0].value);
                let result = call_i128(self, &format!("zirk_int_{name}"), vec![other]);
                Some(self.int_from_i128(result, width, span))
            }
            "rotate_left" | "rotate_right" if call.args.len() == 1 => {
                let raw = self.lower_expr(&call.args[0].value);
                let raw_ty = self.type_of_operand(raw);
                let n = if raw_ty == IrType::Int(IntWidth::I64) {
                    raw
                } else {
                    self.emit(InstKind::IntCast(raw), IrType::Int(IntWidth::I64), span)
                };
                let result = self.emit(
                    InstKind::Call {
                        callee: format!("zirk_int_{name}"),
                        args: vec![wide, n, bits_op],
                    },
                    IrType::Int(IntWidth::I128),
                    span,
                );
                Some(self.int_from_i128(result, width, span))
            }
            "clamp" if call.args.len() == 2 => {
                let lo = arg_i128(self, &call.args[0].value);
                let hi = arg_i128(self, &call.args[1].value);
                let result = call_i128(self, "zirk_int_clamp", vec![lo, hi]);
                Some(self.int_from_i128(result, width, span))
            }
            "checked_add" | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem"
            | "checked_pow"
                if call.args.len() == 1 =>
            {
                let other = arg_i128(self, &call.args[0].value);
                let wide_slot = self.spill(wide, IrType::Int(IntWidth::I128), span);
                let other_slot = self.spill(other, IrType::Int(IntWidth::I128), span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: format!("zirk_int_{name}_ok"),
                        args: vec![wide, other, bits_op, signed_op],
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let wide = lower.emit(
                            InstKind::Load(wide_slot),
                            IrType::Int(IntWidth::I128),
                            span,
                        );
                        let other = lower.emit(
                            InstKind::Load(other_slot),
                            IrType::Int(IntWidth::I128),
                            span,
                        );
                        let bits_op = lower.emit(
                            InstKind::ConstInt(bits as i128),
                            IrType::Int(IntWidth::I32),
                            span,
                        );
                        let signed_op = lower.emit(
                            InstKind::ConstInt(signed as i128),
                            IrType::Int(IntWidth::I32),
                            span,
                        );
                        let value = lower.emit(
                            InstKind::Call {
                                callee: format!("zirk_int_{name}_value"),
                                args: vec![wide, other, bits_op, signed_op],
                            },
                            IrType::Int(IntWidth::I128),
                            span,
                        );
                        lower.int_from_i128(value, width, span)
                    },
                    "integer overflow or division by zero",
                ))
            }
            "to_string"
                if call.args.len() == 1
                    && call.args[0]
                        .name
                        .as_ref()
                        .is_some_and(|n| n.name == "radix") =>
            {
                let raw = self.lower_expr(&call.args[0].value);
                let raw_ty = self.type_of_operand(raw);
                let radix = if raw_ty == IrType::Int(IntWidth::I32) {
                    raw
                } else {
                    self.emit(InstKind::IntCast(raw), IrType::Int(IntWidth::I32), span)
                };
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_int_to_string_radix".to_string(),
                        args: vec![wide, radix],
                    },
                    IrType::String,
                    span,
                ))
            }
            // `n.pow(k)` — the desugaring of `n ** k`
            // (`exponentiation-operator`). When the checker widened the result
            // to exact `Float` (a statically negative literal exponent), the
            // base becomes a `Decimal` at scale 0 and the exponent an `i64`,
            // routed to the exact repeated-multiply helper. Otherwise the
            // result stays integer: `zirk_int_checked_pow_ok` gates the
            // operation and an unrepresentable result throws
            // `ArithmeticOverflowError`, like every other integer operation.
            "pow" if call.args.len() == 1 => {
                let widens_to_decimal = self
                    .checked
                    .expr_types
                    .get(&call.span)
                    .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Decimal));

                // Spill the base before lowering the exponent: the exponent
                // may itself be a `**` (`2 ** 3 ** 2`), whose lowering splits
                // blocks, and values do not cross blocks in this IR.
                let base_slot = self.spill(wide, IrType::Int(IntWidth::I128), span);
                let exp_raw = self.lower_expr(&call.args[0].value);
                let exp_ty = self.type_of_operand(exp_raw);
                let exp_i64 = if exp_ty == IrType::Int(IntWidth::I64) {
                    exp_raw
                } else {
                    self.emit(InstKind::IntCast(exp_raw), IrType::Int(IntWidth::I64), span)
                };
                let exp_slot = self.spill(exp_i64, IrType::Int(IntWidth::I64), span);

                let bits_of = |lower: &mut Self| {
                    lower.emit(
                        InstKind::ConstInt(bits as i128),
                        IrType::Int(IntWidth::I32),
                        span,
                    )
                };
                let signed_of = |lower: &mut Self| {
                    lower.emit(
                        InstKind::ConstInt(signed as i128),
                        IrType::Int(IntWidth::I32),
                        span,
                    )
                };

                if widens_to_decimal {
                    let base =
                        self.emit(InstKind::Load(base_slot), IrType::Int(IntWidth::I128), span);
                    let base_dec = self.convert_numeric(
                        base,
                        IrType::Int(IntWidth::I128),
                        IrType::Decimal,
                        span,
                    );
                    let exp = self.emit(InstKind::Load(exp_slot), IrType::Int(IntWidth::I64), span);
                    return Some(self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_decimal_pow_i".to_string(),
                            args: vec![base_dec, exp],
                        },
                        IrType::Decimal,
                        span,
                    ));
                }

                let base = self.emit(InstKind::Load(base_slot), IrType::Int(IntWidth::I128), span);
                let exp64 = self.emit(InstKind::Load(exp_slot), IrType::Int(IntWidth::I64), span);
                let exp = self.int_to_i128(exp64, IrType::Int(IntWidth::I64), span);
                let bits_op = bits_of(self);
                let signed_op = signed_of(self);
                let ok = self.emit(
                    InstKind::Call {
                        callee: "zirk_int_checked_pow_ok".to_string(),
                        args: vec![base, exp, bits_op, signed_op],
                    },
                    IrType::Boolean,
                    span,
                );
                let fail = self.new_block();
                let cont = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: ok,
                    then_block: cont,
                    else_block: fail,
                });
                self.current = fail;
                let native = self
                    .checked
                    .native_exceptions
                    .expect("a program with integer arithmetic registered the exception hierarchy");
                self.throw_native_failure(native.arithmetic_overflow, "arithmetic overflow", span);
                self.current = cont;
                let base = self.emit(InstKind::Load(base_slot), IrType::Int(IntWidth::I128), span);
                let exp64 = self.emit(InstKind::Load(exp_slot), IrType::Int(IntWidth::I64), span);
                let exp = self.int_to_i128(exp64, IrType::Int(IntWidth::I64), span);
                let bits_op = bits_of(self);
                let signed_op = signed_of(self);
                let value = self.emit(
                    InstKind::Call {
                        callee: "zirk_int_checked_pow_value".to_string(),
                        args: vec![base, exp, bits_op, signed_op],
                    },
                    IrType::Int(IntWidth::I128),
                    span,
                );
                Some(self.int_from_i128(value, width, span))
            }
            _ => None,
        }
    }

    /// `Float` receiver methods — `f64` is the helper signature width;
    /// `FloatCast` converts around the call (`native-type-member-surface`).
    fn lower_float_method_call(
        &mut self,
        call: &ast::CallExpr,
        field: &ast::FieldExpr,
        width: FloatWidth,
        name: &str,
        span: Span,
    ) -> Option<Operand> {
        let receiver_ir = IrType::Float(width);
        let f64_ty = IrType::Float(FloatWidth::F64);
        let receiver = self.lower_expr(&field.object);
        let wide = if receiver_ir == f64_ty {
            receiver
        } else {
            self.emit(InstKind::FloatCast(receiver), f64_ty, span)
        };
        let narrow = |lower: &mut Self, value: Operand| {
            if receiver_ir == f64_ty {
                value
            } else {
                lower.emit(InstKind::FloatCast(value), receiver_ir, span)
            }
        };
        let arg_f64 = |lower: &mut Self, arg: &ast::Expr| {
            let raw = lower.lower_expr(arg);
            if lower.type_of_operand(raw) == f64_ty {
                raw
            } else {
                lower.emit(InstKind::FloatCast(raw), f64_ty, span)
            }
        };
        match name {
            "abs" | "floor" | "ceil" | "round" | "truncate" | "fraction"
                if call.args.is_empty() =>
            {
                let result = self.emit(
                    InstKind::Call {
                        callee: format!("zirk_float_{name}"),
                        args: vec![wide],
                    },
                    f64_ty,
                    span,
                );
                Some(narrow(self, result))
            }
            "sign" if call.args.is_empty() => Some(self.emit(
                InstKind::Call {
                    callee: "zirk_float_sign".to_string(),
                    args: vec![wide],
                },
                IrType::Int(IntWidth::I32),
                span,
            )),
            "is_zero" | "is_finite" | "is_infinite" | "is_negative" if call.args.is_empty() => {
                Some(self.emit(
                    InstKind::Call {
                        callee: format!("zirk_float_{name}"),
                        args: vec![wide],
                    },
                    IrType::Boolean,
                    span,
                ))
            }
            "sqrt" if call.args.is_empty() => {
                // A negative argument has no real answer and Zirk's `Float`
                // family has no `NaN` to give back — `FloatNanError`, the
                // same failure an indeterminate operation raises.
                let zero = self.emit(
                    InstKind::ConstFloat(FloatWidth::F64, "0.0".to_string()),
                    f64_ty,
                    span,
                );
                let negative = self.emit(
                    InstKind::Binary {
                        op: BinaryOp::Lt,
                        left: wide,
                        right: zero,
                    },
                    IrType::Boolean,
                    span,
                );
                let wide_slot = self.spill(wide, f64_ty, span);
                let trap = self.new_block();
                let ok = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: negative,
                    then_block: trap,
                    else_block: ok,
                });
                self.current = trap;
                let native = self
                    .checked
                    .native_exceptions
                    .expect("the checker registered the exception hierarchy");
                self.throw_native_failure(native.float_nan, "sqrt of a negative value", span);
                self.current = ok;
                let wide = self.emit(InstKind::Load(wide_slot), f64_ty, span);
                let result = self.emit(
                    InstKind::Call {
                        callee: "zirk_float_sqrt".to_string(),
                        args: vec![wide],
                    },
                    f64_ty,
                    span,
                );
                Some(narrow(self, result))
            }
            "min" | "max" | "pow" if call.args.len() == 1 => {
                // `pow` accepts an integer exponent too (`powi`) — the
                // argument's own type picks the helper.
                let arg_is_int = self
                    .checked
                    .expr_types
                    .get(&call.args[0].value.span())
                    .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Int(_)));
                if name == "pow" && arg_is_int {
                    let raw = self.lower_expr(&call.args[0].value);
                    let raw_ty = self.type_of_operand(raw);
                    let n = if raw_ty == IrType::Int(IntWidth::I64) {
                        raw
                    } else {
                        self.emit(InstKind::IntCast(raw), IrType::Int(IntWidth::I64), span)
                    };
                    let result = self.emit(
                        InstKind::Call {
                            callee: "zirk_float_pow".to_string(),
                            args: vec![wide, n],
                        },
                        f64_ty,
                        span,
                    );
                    return Some(narrow(self, result));
                }
                let other = arg_f64(self, &call.args[0].value);
                let callee = if name == "pow" {
                    "zirk_float_powf".to_string()
                } else {
                    format!("zirk_float_{name}")
                };
                let result = self.emit(
                    InstKind::Call {
                        callee,
                        args: vec![wide, other],
                    },
                    f64_ty,
                    span,
                );
                Some(narrow(self, result))
            }
            "clamp" if call.args.len() == 2 => {
                let lo = arg_f64(self, &call.args[0].value);
                let hi = arg_f64(self, &call.args[1].value);
                let result = self.emit(
                    InstKind::Call {
                        callee: "zirk_float_clamp".to_string(),
                        args: vec![wide, lo, hi],
                    },
                    f64_ty,
                    span,
                );
                Some(narrow(self, result))
            }
            "format" if call.args.len() == 1 => {
                let spec = self.lower_expr(&call.args[0].value);
                Some(self.emit(
                    InstKind::Call {
                        callee: "zirk_float_format".to_string(),
                        args: vec![wide, spec],
                    },
                    IrType::String,
                    span,
                ))
            }
            _ => None,
        }
    }

    /// Builds `Result<T, E>` from a runtime-computed `ok` flag — the shared
    /// shape of `checked_*`, `IntN.parse`, `FloatN.parse`, and
    /// `Regex.parse` (`native-type-member-surface`): the `Ok` arm runs
    /// `produce`, the `Error` arm builds a native failure whose class is
    /// the `E` the checker recorded for this call. `is_overflow` picks the
    /// message for the `checked_*` family.
    fn build_result_from_flag(
        &mut self,
        call: &ast::CallExpr,
        span: Span,
        ok: Operand,
        produce: impl FnOnce(&mut Self) -> Operand,
        error_message: &str,
    ) -> Operand {
        // `Result<T, E>`'s IR type and the `E` class id both come from the
        // checker's own record of this call.
        let result_sema = self
            .checked
            .expr_types
            .get(&call.span)
            .copied()
            .expect("the checker recorded this call's Result type");
        let result_ty = self.ir_type(result_sema);
        let IrType::Enum(result_enum) = result_ty else {
            unreachable!("Result<T,E> always lowers to IrType::Enum")
        };
        let zirk_sema::Base::EnumInstance(instance_id) = result_sema.base else {
            unreachable!("checked_* and parse always type to a Result instance")
        };
        let error_class = match self.checked.enum_instances[instance_id as usize].args[1].base {
            zirk_sema::Base::Class(id) => id,
            _ => unreachable!("the checker interned the error class"),
        };

        let result_slot = self.declare_slot("<result>", result_ty, span);
        let ok_block = self.new_block();
        let error_block = self.new_block();
        let continue_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: ok,
            then_block: ok_block,
            else_block: error_block,
        });

        self.current = ok_block;
        let value = produce(self);
        let ok_value = self.emit(
            InstKind::BuildEnum {
                enum_id: result_enum,
                variant: 0,
                fields: vec![value],
            },
            result_ty,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, ok_value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = error_block;
        let error_object = self.build_native_failure(error_class, error_message, span);
        let error_value = self.emit(
            InstKind::BuildEnum {
                enum_id: result_enum,
                variant: 1,
                fields: vec![error_object],
            },
            result_ty,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, error_value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result_slot), result_ty, span)
    }

    /// `IntN.parse(text)`, `FloatN.parse(text)`, `Regex.parse(pattern)` —
    /// static calls on type names (`native-type-member-surface`). The
    /// checker recorded the callee's span in `scalar_static_accesses` and
    /// the call's `Result<T, E>` in `expr_types`.
    fn lower_native_static_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !self.checked.scalar_static_accesses.contains(&field.span) {
            return None;
        }
        let ast::Expr::Path(base) = &*field.object else {
            return None;
        };
        // `Date.today()` / `Time.now_local()` / `DateTime.now_utc()`
        // (`date-and-time-types`): host-clock constructors lowered to their
        // runtime calls — the base names a type, never a value.
        if let Some((callee, returns)) = match (base.name.as_str(), field.name.name.as_str()) {
            ("Date", "today") => Some(("zirk_rt_date_today", IrType::Int(IntWidth::I64))),
            ("Time", "now_local") => Some(("zirk_rt_time_now_local", IrType::Int(IntWidth::I64))),
            ("Time", "now_utc") => Some(("zirk_rt_time_now_utc", IrType::Int(IntWidth::I64))),
            ("DateTime", "now_local") => {
                Some(("zirk_rt_datetime_now_local", IrType::Int(IntWidth::I128)))
            }
            ("DateTime", "now_utc") => {
                Some(("zirk_rt_datetime_now_utc", IrType::Int(IntWidth::I128)))
            }
            _ => None,
        } {
            return Some(self.emit(
                InstKind::Call {
                    callee: callee.to_string(),
                    args: Vec::new(),
                },
                returns,
                span,
            ));
        }
        if field.name.name != "parse" {
            return None;
        }
        let target = if base.name == "Regex" {
            Type::REGEX
        } else {
            Type::from_name(&base.name)?
        };
        let text = self.lower_expr(&call.args[0].value);
        match target.base {
            // `Date.parse`/`Time.parse`/`DateTime.parse`
            // (`temporal-rich-api`): strict ISO text in, `Result<_, ParseError>`
            // out through the shared `(ok, value)` probe.
            zirk_sema::Base::Date | zirk_sema::Base::Time | zirk_sema::Base::DateTime => {
                let (kind, value_ty) = match target.base {
                    zirk_sema::Base::Date => ("date", IrType::Int(IntWidth::I64)),
                    zirk_sema::Base::Time => ("time", IrType::Int(IntWidth::I64)),
                    _ => ("datetime", IrType::Int(IntWidth::I128)),
                };
                let text_slot = self.spill(text, IrType::String, span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: format!("zirk_rt_{kind}_parse_ok"),
                        args: vec![text],
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let text = lower.emit(InstKind::Load(text_slot), IrType::String, span);
                        lower.emit(
                            InstKind::Call {
                                callee: format!("zirk_rt_{kind}_parse_value"),
                                args: vec![text],
                            },
                            value_ty,
                            span,
                        )
                    },
                    "the text is not a valid ISO 8601 temporal value",
                ))
            }
            zirk_sema::Base::Regex => {
                let text_slot = self.spill(text, IrType::String, span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: "zirk_regex_is_valid_pattern".to_string(),
                        args: vec![text],
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let text = lower.emit(InstKind::Load(text_slot), IrType::String, span);
                        lower.emit(
                            InstKind::Call {
                                callee: "zirk_regex_from_pattern".to_string(),
                                args: vec![text],
                            },
                            IrType::Regex,
                            span,
                        )
                    },
                    "invalid regex pattern",
                ))
            }
            zirk_sema::Base::Int(width) => {
                let width = self.ir_int_width(width);
                let bits_value = width.bits() as i128;
                let signed_flag = if width.signed() { 1 } else { 0 } as i128;
                let bits = self.emit(
                    InstKind::ConstInt(bits_value),
                    IrType::Int(IntWidth::I32),
                    span,
                );
                let signed = self.emit(
                    InstKind::ConstInt(signed_flag),
                    IrType::Int(IntWidth::I32),
                    span,
                );
                // `parse(text, radix: n)` — the radix form on the integer
                // family only.
                let radix = if call.args.len() == 2 {
                    let raw = self.lower_expr(&call.args[1].value);
                    let raw_ty = self.type_of_operand(raw);
                    if raw_ty == IrType::Int(IntWidth::I32) {
                        raw
                    } else {
                        self.emit(InstKind::IntCast(raw), IrType::Int(IntWidth::I32), span)
                    }
                } else {
                    self.emit(InstKind::ConstInt(10), IrType::Int(IntWidth::I32), span)
                };
                let suffix = if call.args.len() == 2 { "_radix" } else { "" };
                let ok_args = if call.args.len() == 2 {
                    vec![text, radix, bits, signed]
                } else {
                    vec![text, bits, signed]
                };
                let text_slot = self.spill(text, IrType::String, span);
                let radix_slot = self.spill(radix, IrType::Int(IntWidth::I32), span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: format!("zirk_int_parse{suffix}_ok"),
                        args: ok_args,
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let text = lower.emit(InstKind::Load(text_slot), IrType::String, span);
                        let radix = lower.emit(
                            InstKind::Load(radix_slot),
                            IrType::Int(IntWidth::I32),
                            span,
                        );
                        let bits = lower.emit(
                            InstKind::ConstInt(bits_value),
                            IrType::Int(IntWidth::I32),
                            span,
                        );
                        let signed = lower.emit(
                            InstKind::ConstInt(signed_flag),
                            IrType::Int(IntWidth::I32),
                            span,
                        );
                        let value = lower.emit(
                            InstKind::Call {
                                callee: format!("zirk_int_parse{suffix}_value"),
                                args: if suffix.is_empty() {
                                    vec![text, bits, signed]
                                } else {
                                    vec![text, radix, bits, signed]
                                },
                            },
                            IrType::Int(IntWidth::I128),
                            span,
                        );
                        lower.int_from_i128(value, width, span)
                    },
                    "invalid integer literal or out of range for the width",
                ))
            }
            zirk_sema::Base::Decimal => {
                let text_slot = self.spill(text, IrType::String, span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_parse_ok".to_string(),
                        args: vec![text],
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let text = lower.emit(InstKind::Load(text_slot), IrType::String, span);
                        lower.emit(
                            InstKind::Call {
                                callee: "zirk_rt_decimal_parse_value".to_string(),
                                args: vec![text],
                            },
                            IrType::Decimal,
                            span,
                        )
                    },
                    "invalid decimal literal",
                ))
            }
            zirk_sema::Base::Float(width) => {
                let width = self.ir_float_width(width);
                let text_slot = self.spill(text, IrType::String, span);
                let ok = self.emit(
                    InstKind::Call {
                        callee: "zirk_float_parse_ok".to_string(),
                        args: vec![text],
                    },
                    IrType::Boolean,
                    span,
                );
                Some(self.build_result_from_flag(
                    call,
                    span,
                    ok,
                    move |lower| {
                        let text = lower.emit(InstKind::Load(text_slot), IrType::String, span);
                        let value = lower.emit(
                            InstKind::Call {
                                callee: "zirk_float_parse_value".to_string(),
                                args: vec![text],
                            },
                            IrType::Float(FloatWidth::F64),
                            span,
                        );
                        if width == FloatWidth::F64 {
                            value
                        } else {
                            lower.emit(InstKind::FloatCast(value), IrType::Float(width), span)
                        }
                    },
                    "invalid float literal",
                ))
            }
            _ => None,
        }
    }

    /// Whether `call` is a `TypeName.parse(...)` static — consulted by
    /// `is_callable_call` so the callee is never type-queried as a field.
    fn is_native_static_call(&self, call: &ast::CallExpr) -> bool {
        matches!(
            &*call.callee,
            ast::Expr::Field(field)
                if self.checked.scalar_static_accesses.contains(&field.span)
        )
    }

    /// `E.keys()`/`E.values()`/`E.from_name`/`E.from_value` and the
    /// `Enums.keys/values/count(E)` helpers (`enum-static-members`). The
    /// checker recorded the callee's span in `enum_static_accesses`;
    /// everything but the lookups is a compile-time constant shape.
    fn lower_enum_static_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !self.checked.enum_static_accesses.contains(&field.span) {
            return None;
        }
        // `E.member` names the enum as the callee's object; `Enums.member(E)`
        // passes it as the only argument — a type reference in argument
        // position, never lowered as a value.
        let ast::Expr::Path(base) = &*field.object else {
            unreachable!("the checker only records a path base")
        };
        let enum_ident = if base.name == "Enums" {
            let ast::Expr::Path(arg) = &call.args[0].value else {
                unreachable!("the checker required an enum type name")
            };
            arg
        } else {
            base
        };
        let enum_id = self.enum_id_of(enum_ident);
        let variant_names: Vec<String> = self.checked.enums[enum_id as usize]
            .variants
            .iter()
            .map(|v| v.name.clone())
            .collect();

        match field.name.name.as_str() {
            "count" => Some(self.emit(
                InstKind::ConstInt(variant_names.len() as i128),
                IrType::Int(IntWidth::I32),
                span,
            )),
            // `E.to_string()` — the call spelling of the `E.to_string`
            // property — renders the enum type name.
            "to_string" => {
                let name = self.checked.enums[enum_id as usize].name.clone();
                Some(self.const_string(&name, span))
            }
            "keys" | "values" => {
                let ty = self
                    .checked
                    .expr_types
                    .get(&call.span)
                    .copied()
                    .expect("the checker recorded this call's List type");
                let Base::List(list_id) = ty.base else {
                    unreachable!("keys/values always type to a List")
                };
                let list_ty = IrType::List(list_id);
                let element_ty = self.module.list_types[list_id as usize];
                let list = self.emit(
                    InstKind::ListNew {
                        element_id: list_id,
                    },
                    list_ty,
                    span,
                );
                if variant_names.is_empty() {
                    return Some(list);
                }
                let list_slot = self.spill(list, list_ty, span);
                for (i, name) in variant_names.iter().enumerate() {
                    let receiver = self.emit(InstKind::Load(list_slot), list_ty, span);
                    // `values()` is a traditional-enum-only member (the
                    // checker rejects it on payloads), so each element is
                    // the case's `Int32` discriminant — its index.
                    let value = if field.name.name == "keys" {
                        let name = self.module.intern_string(name);
                        self.emit(InstKind::ConstString(name), IrType::String, span)
                    } else {
                        self.emit(
                            InstKind::ConstInt(i as i128),
                            IrType::Int(IntWidth::I32),
                            span,
                        )
                    };
                    debug_assert_eq!(element_ty, self.type_of_operand(value));
                    self.emit(InstKind::ListAdd { receiver, value }, IrType::Void, span);
                }
                Some(self.emit(InstKind::Load(list_slot), list_ty, span))
            }
            "from_name" | "from_value" => Some(self.lower_enum_lookup(call, field, enum_id, span)),
            _ => unreachable!("the checker only records the declared static members"),
        }
    }

    /// `E.from_name(name)`/`E.from_value(value)` (`enum-static-members`):
    /// the run-time half of the surface, expanded to the same comparison
    /// chain an exhaustive `match` would take — each block tests the
    /// argument against one case's key, and the miss side falls through to
    /// the `Err(LookupError)` tail.
    ///
    /// ```text
    ///   test₁ ──(no)──▶ test₂ ──(no)──▶ … ──▶ Err(LookupError)
    ///     │(yes)          │(yes)
    ///     ▼               ▼
    ///   Ok(case₁)       Ok(case₂) ──▶ continue ◀── …
    /// ```
    fn lower_enum_lookup(
        &mut self,
        call: &ast::CallExpr,
        field: &ast::FieldExpr,
        enum_id: u32,
        span: Span,
    ) -> Operand {
        let by_name = field.name.name == "from_name";
        let variants = &self.checked.enums[enum_id as usize].variants;
        // The key type is the one `Checker::enum_mapped_value_type` decided:
        // `String` when any `->` mapping is text, `Int32` otherwise.
        let by_text = by_name
            || variants
                .iter()
                .any(|v| matches!(v.mapping, Some(VariantMapping::Text(_))));
        let key_ty = if by_text {
            IrType::String
        } else {
            IrType::Int(IntWidth::I32)
        };
        // The key each case is compared against, resolved now but emitted
        // inside its own test block later — an IR value never crosses a
        // block boundary.
        let keys: Vec<Key> = variants
            .iter()
            .enumerate()
            .map(|(i, variant)| {
                if by_text {
                    let text = match &variant.mapping {
                        Some(VariantMapping::Text(text)) if !by_name => text.clone(),
                        // `from_name` always looks the declared name up, and
                        // a case without a `->` mapping keeps its name as
                        // its value in a string-mapped enum.
                        _ => variant.name.clone(),
                    };
                    Key::Text(text)
                } else {
                    let value = match &variant.mapping {
                        Some(VariantMapping::Int(value)) => *value as i128,
                        // A case without a mapping keeps its discriminant —
                        // the index `EnumType.discriminant` already reports.
                        _ => i as i128,
                    };
                    Key::Int(value)
                }
            })
            .collect();

        // `Result<E, LookupError>`'s IR type and the `LookupError` class id
        // both come from the checker's own record of this call — the same
        // extraction `build_result_from_flag` performs.
        let result_sema = self
            .checked
            .expr_types
            .get(&call.span)
            .copied()
            .expect("the checker recorded this call's Result type");
        let result_ty = self.ir_type(result_sema);
        let IrType::Enum(result_enum) = result_ty else {
            unreachable!("Result<T,E> always lowers to IrType::Enum")
        };
        let Base::EnumInstance(instance_id) = result_sema.base else {
            unreachable!("from_name/from_value always type to a Result instance")
        };
        let error_class = match self.checked.enum_instances[instance_id as usize].args[1].base {
            Base::Class(id) => id,
            _ => unreachable!("the checker interned the error class"),
        };

        let argument = self.lower_expr_as(&call.args[0].value, key_ty);
        let arg_slot = self.spill(argument, key_ty, span);
        let result_slot = self.declare_slot("<result>", result_ty, span);
        let continue_block = self.new_block();

        for (i, key) in keys.into_iter().enumerate() {
            let matched = self.new_block();
            let next = self.new_block();
            let argument = self.emit(InstKind::Load(arg_slot), key_ty, span);
            let key = match key {
                Key::Text(text) => {
                    let id = self.module.intern_string(&text);
                    self.emit(InstKind::ConstString(id), IrType::String, span)
                }
                Key::Int(value) => {
                    self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I32), span)
                }
            };
            let found = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left: argument,
                    right: key,
                },
                IrType::Boolean,
                span,
            );
            self.terminate(Terminator::Branch {
                condition: found,
                then_block: matched,
                else_block: next,
            });

            self.current = matched;
            // `Ok` is declared before `Error`, so its variant is always 0 —
            // the same order `build_result_from_flag` relies on.
            let value = self.emit(
                InstKind::ConstInt(i as i128),
                IrType::Int(IntWidth::I32),
                span,
            );
            let ok = self.emit(
                InstKind::BuildEnum {
                    enum_id: result_enum,
                    variant: 0,
                    fields: vec![value],
                },
                result_ty,
                span,
            );
            self.emit_effect(InstKind::Store(result_slot, ok), span);
            self.terminate(Terminator::Jump(continue_block));

            self.current = next;
        }

        let enum_name = self.checked.enums[enum_id as usize].name.clone();
        let error_object = self.build_native_failure(
            error_class,
            &format!("no `{enum_name}` variant matches the lookup"),
            span,
        );
        let error_value = self.emit(
            InstKind::BuildEnum {
                enum_id: result_enum,
                variant: 1,
                fields: vec![error_object],
            },
            result_ty,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, error_value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result_slot), result_ty, span)
    }

    /// Whether `call` is an `E.keys()`/`E.from_*`/`Enums.*` static —
    /// consulted by `is_callable_call` so the callee is never type-queried
    /// as a field, the same reason `is_native_static_call` exists.
    fn is_enum_static_call(&self, call: &ast::CallExpr) -> bool {
        matches!(
            &*call.callee,
            ast::Expr::Field(field)
                if self.checked.enum_static_accesses.contains(&field.span)
        )
    }

    /// `r.reverse()` on a `Range<T>` (roadmap Phase 7): the same elements
    /// produced last to first — a new `Range`, the receiver untouched.
    fn lower_range_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        if !self.is_range_method_call(call) {
            return None;
        }
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        let receiver = self.lower_expr(&field.object);
        Some(self.emit(
            InstKind::Call {
                callee: "zirk_range_reverse".to_string(),
                args: vec![receiver],
            },
            IrType::Range,
            span,
        ))
    }

    /// Whether `call` is `r.reverse()` on a `Range<T>` receiver.
    fn is_range_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if field.name.name != "reverse" || !call.args.is_empty() || field.safe {
            return false;
        }
        self.type_of(&field.object, field.object.span()) == IrType::Range
    }

    /// `Char` built-in classification and normalization methods.
    fn lower_char_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("is_uppercase", 0)
                | ("is_lowercase", 0)
                | ("is_digit", 0)
                | ("is_letter", 0)
                | ("is_whitespace", 0)
                | ("is_ascii", 0)
                | ("is_alphabetic", 0)
                | ("is_numeric", 0)
                | ("is_alphanumeric", 0)
                | ("ascii_code", 0)
                | ("bytes", 0)
                | ("codepoints", 0)
                | ("to_uppercase", 0)
                | ("to_lowercase", 0)
                | ("normalize", 1)
        ) {
            return None;
        }
        if self.type_of(&field.object, field.object.span()) != IrType::Char {
            return None;
        }
        let receiver = self.lower_expr(&field.object);
        if field.name.name == "normalize" {
            let form = self.lower_expr(&call.args[0].value);
            return Some(self.emit(
                InstKind::Call {
                    callee: "zirk_char_normalize".to_string(),
                    args: vec![receiver, form],
                },
                IrType::String,
                span,
            ));
        }
        // The `List<T>`-returning methods get their concrete element id from
        // the checker so the IR value is type-correct for the verifier.
        let result_ty = || {
            self.checked
                .expr_types
                .get(&call.span)
                .map(|&ty| self.ir_type(ty))
                .unwrap_or(IrType::String)
        };
        if matches!(field.name.name.as_str(), "bytes" | "codepoints") {
            return Some(self.emit(
                InstKind::Call {
                    callee: format!("zirk_char_{}", field.name.name),
                    args: vec![receiver],
                },
                result_ty(),
                span,
            ));
        }
        let (callee, returns) = match field.name.name.as_str() {
            "is_uppercase" => ("zirk_char_is_uppercase", IrType::Boolean),
            "is_lowercase" => ("zirk_char_is_lowercase", IrType::Boolean),
            "is_digit" => ("zirk_char_is_digit", IrType::Boolean),
            "is_letter" => ("zirk_char_is_letter", IrType::Boolean),
            "is_whitespace" => ("zirk_char_is_whitespace", IrType::Boolean),
            "is_ascii" => ("zirk_char_is_ascii", IrType::Boolean),
            "is_alphabetic" => ("zirk_char_is_alphabetic", IrType::Boolean),
            "is_numeric" => ("zirk_char_is_numeric", IrType::Boolean),
            "is_alphanumeric" => ("zirk_char_is_alphanumeric", IrType::Boolean),
            "ascii_code" => ("zirk_char_ascii_code", IrType::Int(IntWidth::I32)),
            "to_uppercase" => ("zirk_char_to_uppercase", IrType::String),
            "to_lowercase" => ("zirk_char_to_lowercase", IrType::String),
            _ => unreachable!("name checked above"),
        };
        Some(self.emit(
            InstKind::Call {
                callee: callee.to_string(),
                args: vec![receiver],
            },
            returns,
            span,
        ))
    }

    /// `list.add(v)`, `list.insert(i, v)` and `list.remove(i)` are lowered to
    /// runtime calls through dedicated instructions; the checker has already
    /// validated the receiver and argument types.
    fn lower_array_list_method_call(
        &mut self,
        call: &ast::CallExpr,
        span: Span,
    ) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if let ast::Expr::Path(ident) = &*field.object
            && self.try_lookup_slot(&ident.name).is_none()
        {
            return None;
        }
        let receiver_ty = self.type_of(&field.object, field.object.span());
        let element_ty = match receiver_ty {
            IrType::Array(_) | IrType::List(_) => self.array_list_element_type(receiver_ty),
            _ => return None,
        };
        let receiver = self.lower_expr(&field.object);
        match (field.name.name.as_str(), call.args.len(), receiver_ty) {
            ("add", 1, IrType::List(_)) => {
                let value = self.lower_expr_as(&call.args[0].value, element_ty);
                Some(self.emit(InstKind::ListAdd { receiver, value }, IrType::Void, span))
            }
            ("insert", 2, IrType::List(_)) => {
                let index = self.lower_expr_as(&call.args[0].value, IrType::Int(IntWidth::I64));
                let value = self.lower_expr_as(&call.args[1].value, element_ty);
                Some(self.emit(
                    InstKind::ListInsert {
                        receiver,
                        index,
                        value,
                    },
                    IrType::Void,
                    span,
                ))
            }
            ("remove", 1, IrType::List(_)) => {
                let receiver_ty = self.value_types[&receiver.0];
                let element_ty = self.array_list_element_type(receiver_ty);
                let arg_ty = self.type_of(&call.args[0].value, call.args[0].value.span());
                if arg_ty == element_ty {
                    let value = self.lower_expr_as(&call.args[0].value, element_ty);
                    Some(self.emit(
                        InstKind::ListRemoveValue { receiver, value },
                        IrType::Boolean,
                        span,
                    ))
                } else if matches!(arg_ty, IrType::Int(_)) {
                    let index = self.lower_expr_as(&call.args[0].value, IrType::Int(IntWidth::I64));
                    Some(self.emit(InstKind::ListRemove { receiver, index }, IrType::Void, span))
                } else {
                    let value = self.lower_expr_as(&call.args[0].value, element_ty);
                    Some(self.emit(
                        InstKind::ListRemoveValue { receiver, value },
                        IrType::Boolean,
                        span,
                    ))
                }
            }
            ("clone", 0, _) => {
                let kind = if matches!(self.value_types[&receiver.0], IrType::Array(_)) {
                    InstKind::ArrayClone { receiver }
                } else {
                    InstKind::ListClone { receiver }
                };
                let ty = self.value_types[&receiver.0];
                Some(self.emit(kind, ty, span))
            }
            _ => None,
        }
    }

    /// `Array<T>(capacity)` and `List<T>()` are lowered to dedicated
    /// allocation instructions; the element type comes from the checker's
    /// recorded semantic type.
    /// `Map<K, V>` and `Set<T>` built-in methods: `length`, `is_empty`,
    /// `set`, `contains_key`, `add`, `contains`.
    fn lower_map_set_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if let ast::Expr::Path(ident) = &*field.object
            && self.try_lookup_slot(&ident.name).is_none()
        {
            return None;
        }
        let receiver_ty = self.type_of(&field.object, field.object.span());
        if !matches!(receiver_ty, IrType::Map(_) | IrType::Set(_)) {
            return None;
        }
        let receiver = self.lower_expr(&field.object);
        let (key_ty, value_ty, element_ty) = match receiver_ty {
            IrType::Map(id) => {
                let (k, v) = self
                    .module
                    .map_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or((IrType::Void, IrType::Void));
                (k, v, IrType::Void)
            }
            IrType::Set(id) => {
                let e = self
                    .module
                    .set_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void);
                (IrType::Void, IrType::Void, e)
            }
            _ => return None,
        };

        match (field.name.name.as_str(), call.args.len(), receiver_ty) {
            ("length", 0, IrType::Map(_)) => Some(self.emit(
                InstKind::MapLength(receiver),
                IrType::Int(IntWidth::U64),
                span,
            )),
            ("length", 0, IrType::Set(_)) => Some(self.emit(
                InstKind::SetLength(receiver),
                IrType::Int(IntWidth::U64),
                span,
            )),
            ("is_empty", 0, IrType::Map(_)) => {
                Some(self.emit(InstKind::MapIsEmpty(receiver), IrType::Boolean, span))
            }
            ("is_empty", 0, IrType::Set(_)) => {
                Some(self.emit(InstKind::SetIsEmpty(receiver), IrType::Boolean, span))
            }
            ("set", 2, IrType::Map(_)) => {
                let key = self.lower_expr_as(&call.args[0].value, key_ty);
                let value = self.lower_expr_as(&call.args[1].value, value_ty);
                Some(self.emit(
                    InstKind::MapSet {
                        receiver,
                        key,
                        value,
                    },
                    IrType::Void,
                    span,
                ))
            }
            ("contains_key", 1, IrType::Map(_)) => {
                let key = self.lower_expr_as(&call.args[0].value, key_ty);
                Some(self.emit(
                    InstKind::MapContainsKey { receiver, key },
                    IrType::Boolean,
                    span,
                ))
            }
            ("add", 1, IrType::Set(_)) => {
                let value = self.lower_expr_as(&call.args[0].value, element_ty);
                Some(self.emit(InstKind::SetAdd { receiver, value }, IrType::Void, span))
            }
            ("contains", 1, IrType::Set(_)) => {
                let value = self.lower_expr_as(&call.args[0].value, element_ty);
                Some(self.emit(
                    InstKind::SetContains { receiver, value },
                    IrType::Boolean,
                    span,
                ))
            }
            ("get_or_null", 1, IrType::Map(_)) => {
                let key = self.lower_expr_as(&call.args[0].value, key_ty);
                let nullable = Nullable::of(value_ty)?;
                Some(self.emit(
                    InstKind::MapGet { receiver, key },
                    IrType::Nullable(nullable),
                    span,
                ))
            }
            ("remove", 1, IrType::Map(_)) => {
                let key = self.lower_expr_as(&call.args[0].value, key_ty);
                Some(self.emit(InstKind::MapRemove { receiver, key }, IrType::Boolean, span))
            }
            ("remove", 1, IrType::Set(_)) => {
                let value = self.lower_expr_as(&call.args[0].value, element_ty);
                Some(self.emit(
                    InstKind::SetRemove { receiver, value },
                    IrType::Boolean,
                    span,
                ))
            }
            _ => None,
        }
    }

    fn lower_array_list_construction(
        &mut self,
        call: &ast::CallExpr,
        span: Span,
    ) -> Option<Operand> {
        let ast::Expr::Path(callee) = &*call.callee else {
            return None;
        };
        if !matches!(callee.name.as_str(), "Array" | "List" | "Map" | "Set") {
            return None;
        }
        let &ty = self.checked.expr_types.get(&call.span)?;
        match (callee.name.as_str(), ty.base) {
            ("Array", Base::Array(id)) if !call.args.is_empty() => {
                if call.args.len() == 1 {
                    // `Array<T>(capacity)` — the single-argument form keeps
                    // its historical meaning as an empty array with room.
                    let capacity =
                        self.lower_expr_as(&call.args[0].value, IrType::Int(IntWidth::U64));
                    return Some(self.emit(
                        InstKind::ArrayNew {
                            element_id: id,
                            capacity,
                        },
                        IrType::Array(id),
                        span,
                    ));
                }
                // `Array(e0, e1, …)` — a literal of `args.len()` elements.
                let len = call.args.len() as u64;
                let capacity = self.emit(
                    InstKind::ConstInt(len as i128),
                    IrType::Int(IntWidth::U64),
                    span,
                );
                let array = self.emit(
                    InstKind::ArrayNew {
                        element_id: id,
                        capacity,
                    },
                    IrType::Array(id),
                    span,
                );
                let array_slot = self.spill(array, IrType::Array(id), span);
                let element_ty = self.module.array_types[id as usize];
                for (i, arg) in call.args.iter().enumerate() {
                    let receiver = self.emit(InstKind::Load(array_slot), IrType::Array(id), span);
                    let index = self.emit(
                        InstKind::ConstInt(i as i128),
                        IrType::Int(IntWidth::I64),
                        span,
                    );
                    let value = self.lower_expr_as(&arg.value, element_ty);
                    self.emit_effect(
                        InstKind::ArrayListStore {
                            receiver,
                            index,
                            value,
                        },
                        span,
                    );
                }
                Some(self.emit(InstKind::Load(array_slot), IrType::Array(id), span))
            }
            ("List", Base::List(id)) => {
                let list = self.emit(InstKind::ListNew { element_id: id }, IrType::List(id), span);
                if call.args.is_empty() {
                    return Some(list);
                }
                let list_slot = self.spill(list, IrType::List(id), span);
                let element_ty = self.module.list_types[id as usize];
                for arg in call.args.iter() {
                    let receiver = self.emit(InstKind::Load(list_slot), IrType::List(id), span);
                    let value = self.lower_expr_as(&arg.value, element_ty);
                    self.emit(InstKind::ListAdd { receiver, value }, IrType::Void, span);
                }
                Some(self.emit(InstKind::Load(list_slot), IrType::List(id), span))
            }
            ("Map", Base::Map(id)) if call.args.is_empty() => {
                Some(self.emit(InstKind::MapNew { map_id: id }, IrType::Map(id), span))
            }
            ("Set", Base::Set(id)) if call.args.is_empty() => {
                Some(self.emit(InstKind::SetNew { set_id: id }, IrType::Set(id), span))
            }
            _ => None,
        }
    }

    /// Lowers a `Throwable` intrinsic call to a runtime call.
    fn lower_exception_intrinsic_call(
        &mut self,
        call: &ast::CallExpr,
        span: Span,
    ) -> Option<Operand> {
        let _ty = self.exception_intrinsic_type(call)?;
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        let receiver = self.lower_expr(&field.object);
        let result = match field.name.name.as_str() {
            "stack_trace" => self.emit(InstKind::StackTrace(receiver), IrType::String, span),
            "suppressed" => self.emit(
                InstKind::Suppressed(receiver),
                IrType::Nullable(Nullable::Object(self.throwable_object_id())),
                span,
            ),
            _ => return None,
        };
        Some(result)
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
    /// this checks the scopes the same way `is_callable_call` does before
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
        matches!(
            target.base,
            Base::Int(_) | Base::Float(_) | Base::Decimal | Base::String
        )
        .then_some(target)
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
                ) ||
                // Structural equality on a `record`
                // (`fase-3-structural-equality`, design D1) opens the same
                // per-field branch-and-join shape `&&`/`||` do — an earlier
                // operand held across one of these must go through a slot
                // exactly like it would across a plain `&&`.
                (matches!(e.op, ast::BinaryOp::Eq | ast::BinaryOp::NotEq)
                    && matches!(
                        self.type_of(&e.left, e.left.span()),
                        IrType::Value(_)
                    ))
                    // Arithmetic on `Int` (`+`, `-`, `*`, `/`, `%`) now throws
                    // catchable `ArithmeticOverflowError`/`DivisionByZeroError`
                    // through a branch, and `Float` arithmetic guards `NaN`
                    // the same way. Shifts also throw `InvalidShiftError`.
                    || (matches!(
                        e.op,
                        ast::BinaryOp::Add
                            | ast::BinaryOp::Sub
                            | ast::BinaryOp::Mul
                            | ast::BinaryOp::Div
                            | ast::BinaryOp::Rem
                            | ast::BinaryOp::Shl
                            | ast::BinaryOp::Shr
                    ) && matches!(
                        self.type_of(&ast::Expr::Binary(e.clone()), e.span),
                        IrType::Int(_) | IrType::Float(_)
                    ))
                    // `String * Int` routes through `checked_repeat`, whose
                    // negative-count guard emits a `fail`/`cont` split exactly
                    // like the integer overflow checks above — a sibling
                    // operand emitted first has to cross that branch through a
                    // slot, not a plain value.
                    || (matches!(e.op, ast::BinaryOp::Mul)
                        && self.type_of(&ast::Expr::Binary(e.clone()), e.span)
                            == IrType::String)
                    || self.opens_blocks(&e.left)
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
            // `receiver[index]` (roadmap Phase 4e, `fase-4e-native-slice`,
            // design D5) always opens a `fail`/`cont` split for its own
            // bounds check (`Self::lower_native_slice_bounds_check`),
            // regardless of the receiver or index shape — an earlier
            // operand held across one (a call argument, the other side of
            // a binary operator, …) must go through a slot the same way it
            // would across an `if`/`match`/`?.`.
            ast::Expr::Index(_) => true,
            // A slice's own bounds handling lives in the runtime, but its
            // receiver and bounds are still expressions that can open
            // blocks of their own — spilling them is mandatory.
            ast::Expr::Slice(_) => true,
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
                } else if target == IrType::Decimal {
                    self.lower_decimal_binary(binary_op(b.op), left, right, span)
                } else {
                    let op = binary_op(b.op);
                    self.emit_checked_binary(op, left, right, target, span)
                }
            }
            ast::Expr::Unary(u) if numeric && u.op == ast::UnaryOp::Neg => {
                let operand = self.lower_context_tree(target, &u.operand);
                if matches!(target, IrType::Int(_)) {
                    let zero = self.const_int_at(0, target, span);
                    self.emit_checked_binary(BinaryOp::Sub, zero, operand, target, span)
                } else if target == IrType::Decimal {
                    self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_decimal_neg".to_string(),
                            args: vec![operand],
                        },
                        IrType::Decimal,
                        span,
                    )
                } else {
                    self.emit(
                        InstKind::Unary {
                            op: UnaryOp::Neg,
                            operand,
                        },
                        target,
                        span,
                    )
                }
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
            (_, IrType::Decimal) | (IrType::Decimal, _) => {
                self.convert_numeric(operand, actual, target, span)
            }
            (_, IrType::String) => self.lower_to_string_expr(expr, operand, span),
            _ => unreachable!("the checker validated this conversion"),
        }
    }

    /// A generic class's own bare name (`Box`) cannot tell `Box<Int32>` from
    /// `Box<String>` apart the way `class_id` resolves an ordinary class —
    /// the checker records which instantiation each call site inferred
    /// (`Self::checked`'s `generic_constructions`, roadmap task 11.1), and
    /// that takes priority whenever this call's span is one.
    fn construction_class_id(&self, call: &ast::CallExpr) -> Option<u32> {
        // The checker records the class every construction resolved to —
        // nested and local classes carry registered names the written callee
        // does not (`Outer.Nested`, `Local$local$0`).
        // A generic construction resolves to the specialized copy, not the
        // template `check_construction` recorded.
        if let Some(&instance) = self.checked.generic_constructions.get(&call.span) {
            return Some(self.instance_base + instance);
        }
        if let Some(&id) = self.checked.resolved_constructions.get(&call.span) {
            return Some(id);
        }
        self.class_id(&self.callee_name(call))
    }

    /// Lowers `User(...)` into an allocation plus a call to its constructor.
    ///
    /// The two steps are what separates identity from initialization: the
    /// object exists — and has its address, which is its identity — before its
    /// constructor runs on it.
    fn lower_construction(&mut self, call: &ast::CallExpr, id: u32, span: Span) -> Operand {
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
            return self.emit(InstKind::Alloc(id), IrType::Object(id), span);
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
            let object = self.emit(InstKind::Alloc(id), IrType::Object(id), span);
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

        // Arguments are lowered first, so any branch introduced by them (e.g.
        // integer arithmetic overflow checks) is finished before the object is
        // allocated. The object and the `Call` to the constructor are then
        // emitted in the same block, and the object never crosses a branch.
        let mut args: Vec<Operand> = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        let object = self.emit(InstKind::Alloc(id), IrType::Object(id), span);
        let mut call_args = vec![object];

        // An `inner class`'s constructor takes the enclosing instance as a
        // hidden argument after `this`: the receiver of `o.Inner(...)`, or
        // `this` itself for a bare `Inner(...)` inside the enclosing class.
        if self.checked.inner_constructions.contains_key(&call.span) {
            let parent = self.checked.classes[id as usize]
                .enclosing
                .expect("an inner construction is marked only for inner classes");
            let outer = if let ast::Expr::Field(field) = &*call.callee {
                self.lower_expr_as(&field.object, IrType::Object(parent))
            } else {
                let slot = self.lookup_slot("this");
                self.emit(InstKind::Load(slot), IrType::Object(parent), span)
            };
            call_args.push(outer);
        }
        call_args.extend(args);

        let name = self.module.objects[id as usize].name.clone();
        self.emit_effect(
            InstKind::Call {
                callee: constructor_symbol(&name, index),
                args: call_args,
            },
            span,
        );

        object
    }

    /// Lowers `Point(x: 1, y: 2)`: a record's implicit
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
    /// with associated data (`Shape.Circle(radius: 5)` or the unqualified
    /// `Ok(...)`, `Error(...)` forms of `Result<T,E>`) — the checker marks
    /// such a call's callee span the same way it marks a bare variant
    /// reference's (`self.checked.variant_accesses`), so this is the call
    /// equivalent of that check.
    fn variant_construction(&self, call: &ast::CallExpr) -> Option<(u32, u32)> {
        let (variant_name, enum_name) = match &*call.callee {
            ast::Expr::Field(field) => {
                if !self.checked.variant_accesses.contains(&field.span) {
                    return None;
                }
                let ast::Expr::Path(enum_name) = &*field.object else {
                    return None;
                };
                (field.name.clone(), enum_name.clone())
            }
            ast::Expr::Path(callee) => {
                if !self.checked.variant_accesses.contains(&callee.span) {
                    return None;
                }
                if callee.name != "Ok" && callee.name != "Error" {
                    return None;
                }
                (callee.clone(), ast::Ident::new("Result", callee.span))
            }
            _ => return None,
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
        let variant = self.checked.enums[enum_id as usize].discriminant(&variant_name.name)?;
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
        // `Decimal` <-> integer / `Float` `as` casts route through
        // the `convert_numeric` conversion instructions.
        if matches!(
            (actual_ty, target_ty),
            (IrType::Decimal, _) | (_, IrType::Decimal)
        ) && (Self::is_numeric_ir_type(actual_ty) && Self::is_numeric_ir_type(target_ty))
        {
            return self.convert_numeric(value, actual_ty, target_ty, span);
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

        // `x as? T` produces `T?`: `null` on a mismatch, the retyped value
        // wrapped on success.
        if expr.optional {
            let nullable = Nullable::Object(target_class);
            let result_ty = IrType::Nullable(nullable);
            let result = self.declare_slot("<optional cast>", result_ty, span);

            let value_slot = self.spill(value, actual_ty, span);
            let value_loaded = self.emit(InstKind::Load(value_slot), actual_ty, span);
            let is_instance = self.emit(
                InstKind::IsInstance {
                    object: value_loaded,
                    target_class,
                },
                IrType::Boolean,
                span,
            );

            let fail = self.new_block();
            let success = self.new_block();
            let cont = self.new_block();
            self.terminate(Terminator::Branch {
                condition: is_instance,
                then_block: success,
                else_block: fail,
            });

            self.current = fail;
            let absent = self.emit(InstKind::NullValue(nullable), result_ty, span);
            self.emit_effect(InstKind::Store(result, absent), span);
            self.terminate(Terminator::Jump(cont));

            self.current = success;
            let value_loaded = self.emit(InstKind::Load(value_slot), actual_ty, span);
            let retyped = self.emit(InstKind::Retype(value_loaded), target_ty, span);
            let wrapped = self.emit(
                InstKind::Wrap {
                    base: nullable,
                    value: retyped,
                },
                result_ty,
                span,
            );
            self.emit_effect(InstKind::Store(result, wrapped), span);
            self.terminate(Terminator::Jump(cont));

            self.current = cont;
            return self.emit(InstKind::Load(result), result_ty, span);
        }

        // `as` / `<T>` casts over class types now throw `InvalidCastError`
        // instead of aborting: `IsInstance` answers the same question
        // `CheckedCast` used to assert, and a failing branch builds the
        // exception and dispatches it like an explicit `throw`.
        let value_slot = self.spill(value, actual_ty, span);
        let value_loaded = self.emit(InstKind::Load(value_slot), actual_ty, span);
        let is_instance = self.emit(
            InstKind::IsInstance {
                object: value_loaded,
                target_class,
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_instance,
            then_block: cont,
            else_block: fail,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with a class cast registered the exception hierarchy");
        self.throw_native_failure(native.invalid_cast, "invalid cast", span);

        self.current = cont;
        let value_loaded = self.emit(InstKind::Load(value_slot), actual_ty, span);
        self.emit(InstKind::Retype(value_loaded), target_ty, span)
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
        // Universal `.type` member: materialise the type's name as a `String`.
        if expr.name.name == "type" {
            let ty = self
                .checked
                .expr_types
                .get(&expr.object.span())
                .copied()
                .unwrap_or(Type::UNKNOWN);
            let name = self.type_name(ty);
            let id = self.module.intern_string(&name);
            return self.emit(InstKind::ConstString(id), IrType::String, span);
        }

        if expr.name.name == "is_null"
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Pointer(_)
            )
        {
            let pointer = self.lower_expr(&expr.object);
            return self.emit(InstKind::PointerIsNull(pointer), IrType::Boolean, span);
        }

        // `.is_alive` (roadmap Phase 4e, `fase-4e-weak`, design D4): the same
        // null-check `.upgrade()` does, without producing a new strong
        // reference.
        if expr.name.name == "is_alive"
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Weak(_)
            )
        {
            let weak = self.lower_expr(&expr.object);
            return self.emit(InstKind::WeakIsAlive(weak), IrType::Boolean, span);
        }

        // `.length`/`.is_empty` (roadmap Phase 4e, `fase-4e-native-slice`) —
        // usable in ordinary safe code on an already-constructed
        // `NativeSlice<T>`/`NativeSliceMut<T>` (proposal, task 1.3). The
        // member name is checked *first*, short-circuiting before
        // `Self::type_of` ever runs on `expr.object` — the same order
        // `.is_null`/`.is_alive` just above use, and for the same reason:
        // `expr.object` may name an enum (`Direction.North`), which
        // `Self::type_of` cannot resolve as a variable or function at all.
        if matches!(expr.name.name.as_str(), "length" | "is_empty")
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::NativeSlice(_) | IrType::NativeSliceMut(_)
            )
        {
            let receiver = self.lower_expr(&expr.object);
            match expr.name.name.as_str() {
                "length" => {
                    return self.emit(
                        InstKind::NativeSliceLength(receiver),
                        IrType::Int(IntWidth::U64),
                        span,
                    );
                }
                "is_empty" => {
                    let length = self.emit(
                        InstKind::NativeSliceLength(receiver),
                        IrType::Int(IntWidth::U64),
                        span,
                    );
                    let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
                    let zero = self.emit(InstKind::IntCast(zero), IrType::Int(IntWidth::U64), span);
                    return self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Eq,
                            left: length,
                            right: zero,
                        },
                        IrType::Boolean,
                        span,
                    );
                }
                _ => unreachable!("the checker only types `.length`/`.is_empty` on a native view"),
            }
        }

        // `Direction.count`/`Direction.to_string` (`enum-static-members`):
        // a static member on the enum's own type name — the path names a
        // type, not a value to read a field from, so this answers before
        // any `type_of` on `expr.object`, the same ordering the
        // `variant_accesses` branch below keeps for `Direction.North`.
        if self.checked.enum_static_accesses.contains(&expr.span) {
            let ast::Expr::Path(enum_name) = &*expr.object else {
                unreachable!("an enum static access names its enum")
            };
            let enum_id = self.enum_id_of(enum_name);
            return match expr.name.name.as_str() {
                "count" => {
                    let count = self.checked.enums[enum_id as usize].variants.len();
                    self.emit(
                        InstKind::ConstInt(count as i128),
                        IrType::Int(IntWidth::I32),
                        span,
                    )
                }
                // `E.to_string` renders the enum type name — a constant
                // string known since the declaration resolved.
                "to_string" => {
                    let name = self.checked.enums[enum_id as usize].name.clone();
                    self.const_string(&name, span)
                }
                _ => unreachable!("the checker only records `count`/`to_string` here"),
            };
        }

        // A `ClassName.field` that names a `static` field of a user class.
        if let ast::Expr::Path(base) = &*expr.object
            && let Some(class_id) = self
                .checked
                .classes
                .iter()
                .position(|c| c.name == base.name)
        {
            let class = self.checked.classes[class_id].clone();
            if let Some(field) = class.field(&expr.name.name).cloned()
                && field.is_static
            {
                let callee = static_field_symbol(&class.name, &expr.name.name);
                let returns = self.ir_type(field.ty);
                return self.emit(
                    InstKind::Call {
                        callee,
                        args: Vec::new(),
                    },
                    returns,
                    span,
                );
            }
        }

        // `native-type-member-surface` field/property lowering.
        //
        // Static scalar constants, `String`/`Char` metadata, `Tuple.length`,
        // and `Enum` `.name`/`.value`/`.to_string` all have no in-memory
        // layout to read; they are produced at compile time or by runtime
        // calls.  The order mirrors `field_type_of`.
        if self.checked.scalar_static_accesses.contains(&expr.span) {
            return self.lower_scalar_static(&expr.object, &expr.name.name, span);
        }

        // A bare enum variant access (`Shape.Circle` or `Direction.North`)
        // names the enum itself as `object`; it has no runtime value to
        // take a property from, so it must be handled before any `type_of`
        // call on `object`.
        if self.checked.variant_accesses.contains(&expr.span) {
            let ast::Expr::Path(enum_name) = &*expr.object else {
                unreachable!("a variant access names its enum")
            };
            let enum_id = self.enum_id_of(enum_name);
            let discriminant = self.discriminant(enum_name, &expr.name.name);
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
                    InstKind::ConstInt(discriminant as i128),
                    IrType::Int(IntWidth::I32),
                    span,
                )
            };
        }

        if self.type_of(&expr.object, expr.object.span()) == IrType::String
            && let Some(operand) = self.lower_string_property(&expr.object, &expr.name.name, span)
        {
            return operand;
        }
        if self.type_of(&expr.object, expr.object.span()) == IrType::Char
            && let Some(operand) = self.lower_char_property(&expr.object, &expr.name.name, span)
        {
            return operand;
        }
        if let Some(operand) = self.lower_duration_property(&expr.object, &expr.name.name, span) {
            return operand;
        }
        if let Some(operand) = self.lower_temporal_property(&expr.object, &expr.name.name, span) {
            return operand;
        }
        if self
            .checked
            .expr_types
            .get(&expr.object.span())
            .is_some_and(|ty| matches!(ty.base, Base::Tuple(_)))
            && expr.name.name == "length"
        {
            return self.lower_tuple_length(&expr.object, span);
        }
        if let Some(operand) = self.lower_enum_property(expr, span) {
            return operand;
        }

        // `r.start`/`r.end`/`r.step` on a `Range<T>` (roadmap Phase 7): the
        // runtime stores every part as `i64`, so the getter's result narrows
        // back to the element's own width for an `Int32` range.
        if matches!(expr.name.name.as_str(), "start" | "end" | "step")
            && self.type_of(&expr.object, expr.object.span()) == IrType::Range
        {
            let receiver = self.lower_expr(&expr.object);
            let callee = match expr.name.name.as_str() {
                "start" => "zirk_range_start",
                "end" => "zirk_range_end",
                "step" => "zirk_range_step",
                _ => unreachable!("checked just above"),
            };
            let value = self.emit(
                InstKind::Call {
                    callee: callee.to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I64),
                span,
            );
            let element = self
                .checked
                .expr_types
                .get(&expr.object.span())
                .and_then(|ty| match ty.base {
                    Base::Range(id) => Some(self.checked.range_types[id as usize]),
                    _ => None,
                })
                .map(|element| self.ir_type(element))
                .unwrap_or(IrType::Int(IntWidth::I64));
            return if element == IrType::Int(IntWidth::I64) {
                value
            } else {
                self.emit(InstKind::IntCast(value), element, span)
            };
        }

        // `.length`/`.is_empty` on `Array<T>`/`List<T>`.
        if matches!(expr.name.name.as_str(), "length" | "is_empty")
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Array(_) | IrType::List(_)
            )
        {
            let receiver = self.lower_expr(&expr.object);
            let is_list = matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::List(_)
            );
            let length = self.emit(
                if is_list {
                    InstKind::ListLength(receiver)
                } else {
                    InstKind::ArrayLength(receiver)
                },
                IrType::Int(IntWidth::U64),
                span,
            );
            match expr.name.name.as_str() {
                "length" => return length,
                "is_empty" => {
                    let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
                    let zero = self.emit(InstKind::IntCast(zero), IrType::Int(IntWidth::U64), span);
                    return self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Eq,
                            left: length,
                            right: zero,
                        },
                        IrType::Boolean,
                        span,
                    );
                }
                _ => unreachable!(),
            }
        }

        // `.length`/`.is_empty` on `Map<K, V>`/`Set<T>`.
        if matches!(expr.name.name.as_str(), "length" | "is_empty")
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Map(_) | IrType::Set(_)
            )
        {
            let receiver = self.lower_expr(&expr.object);
            let is_set = matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Set(_)
            );
            let length = self.emit(
                if is_set {
                    InstKind::SetLength(receiver)
                } else {
                    InstKind::MapLength(receiver)
                },
                IrType::Int(IntWidth::U64),
                span,
            );
            match expr.name.name.as_str() {
                "length" => return length,
                "is_empty" => {
                    let zero = self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span);
                    let zero = self.emit(InstKind::IntCast(zero), IrType::Int(IntWidth::U64), span);
                    return self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Eq,
                            left: length,
                            right: zero,
                        },
                        IrType::Boolean,
                        span,
                    );
                }
                _ => unreachable!(),
            }
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
                    InstKind::ConstInt(discriminant as i128),
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

    /// Builds a `String` from a literal, interning it once.
    fn const_string(&mut self, text: &str, span: Span) -> Operand {
        let id = self.module.intern_string(text);
        self.emit(InstKind::ConstString(id), IrType::String, span)
    }

    /// `Int32.MAX`, `Float64.EPSILON`, etc. — the value is a constant the
    /// checker already validated (`native-type-member-surface`).
    fn lower_scalar_static(&mut self, object: &ast::Expr, member: &str, span: Span) -> Operand {
        let ast::Expr::Path(base) = object else {
            unreachable!("a scalar static access names a type")
        };
        let Some(ty) = Type::from_name(&base.name) else {
            unreachable!("the checker only records existing scalar types")
        };
        match ty.base {
            zirk_sema::Base::Int(width) => {
                self.lower_int_constant(self.ir_int_width(width), member, span)
            }
            zirk_sema::Base::Float(width) => {
                self.lower_float_constant(self.ir_float_width(width), member, span)
            }
            _ => unreachable!("the checker only records scalar types"),
        }
    }

    fn lower_int_constant(&mut self, width: IntWidth, member: &str, span: Span) -> Operand {
        let bits = width.bits();
        let value = match member {
            "MIN" => {
                if width.signed() {
                    -(1i128 << (bits - 1))
                } else {
                    0
                }
            }
            "MAX" => {
                if width.signed() {
                    (1i128 << (bits - 1)) - 1
                } else if bits == 128 {
                    u128::MAX as i128
                } else {
                    (1i128 << bits) - 1
                }
            }
            "BITS" => bits as i128,
            _ => unreachable!("the checker only records MIN/MAX/BITS"),
        };
        let ir = IrType::Int(width);
        if member == "BITS" {
            let raw = self.emit(InstKind::ConstInt(value), IrType::Int(IntWidth::I32), span);
            if ir == IrType::Int(IntWidth::I32) {
                raw
            } else {
                self.emit(InstKind::IntCast(raw), ir, span)
            }
        } else {
            self.emit(InstKind::ConstInt(value), ir, span)
        }
    }

    fn lower_float_constant(&mut self, width: FloatWidth, member: &str, span: Span) -> Operand {
        use zirk_sema::FloatWidth as W;
        // The literal strings are widened via `FloatCast` below for `F16`/
        // `F32`/`F128`; `f64` values here are the widest currently used.
        let f64_str = match member {
            "MIN" => match self.sema_float_width(width) {
                W::F16 => "6.103515625e-5",
                W::F32 => "1.1754943508222875e-38",
                W::F64 => "2.2250738585072014e-308",
                W::F128 => "3.3621031431120935062626778173217526e-4932",
            },
            "MAX" => match self.sema_float_width(width) {
                W::F16 => "65504",
                W::F32 => "3.4028234663852886e38",
                W::F64 => "1.7976931348623157e308",
                W::F128 => "1.189731495357231765085759326628007e4932",
            },
            "LOWEST" => match self.sema_float_width(width) {
                W::F16 => "-65504",
                W::F32 => "-3.4028234663852886e38",
                W::F64 => "-1.7976931348623157e308",
                W::F128 => "-1.189731495357231765085759326628007e4932",
            },
            "EPSILON" => match self.sema_float_width(width) {
                W::F16 => "0.0009765625",
                W::F32 => "1.1920928955078125e-7",
                W::F64 => "2.220446049250313e-16",
                W::F128 => "1.925929944387235853055977942584927e-34",
            },
            "POSITIVE_INFINITY" => "inf",
            "NEGATIVE_INFINITY" => "-inf",
            _ => unreachable!("the checker only records the documented float constants"),
        };
        let value = self.emit(
            InstKind::ConstFloat(FloatWidth::F64, f64_str.to_string()),
            IrType::Float(FloatWidth::F64),
            span,
        );
        if width == FloatWidth::F64 {
            value
        } else {
            self.emit(InstKind::FloatCast(value), IrType::Float(width), span)
        }
    }

    fn sema_float_width(&self, width: FloatWidth) -> zirk_sema::FloatWidth {
        use FloatWidth as I;
        use zirk_sema::FloatWidth as S;
        match width {
            I::F16 => S::F16,
            I::F32 => S::F32,
            I::F64 => S::F64,
            I::F128 => S::F128,
        }
    }

    /// `s.length`, `s.byte_length`, `s.is_empty` and `c` metadata fields
    /// (`native-type-member-surface`).
    fn lower_string_property(
        &mut self,
        object: &ast::Expr,
        member: &str,
        span: Span,
    ) -> Option<Operand> {
        let receiver = self.lower_expr(object);
        Some(match member {
            "length" => self.emit(
                InstKind::Call {
                    callee: "zirk_str_length".to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I32),
                span,
            ),
            "byte_length" => self.emit(
                InstKind::Call {
                    callee: "zirk_str_byte_length".to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I32),
                span,
            ),
            "is_empty" => self.emit(
                InstKind::Call {
                    callee: "zirk_str_is_empty".to_string(),
                    args: vec![receiver],
                },
                IrType::Boolean,
                span,
            ),
            _ => return None,
        })
    }

    fn lower_char_property(
        &mut self,
        object: &ast::Expr,
        member: &str,
        span: Span,
    ) -> Option<Operand> {
        let receiver = self.lower_expr(object);
        Some(match member {
            "byte_length" => self.emit(
                InstKind::Call {
                    callee: "zirk_char_byte_length".to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I32),
                span,
            ),
            "codepoint_count" => self.emit(
                InstKind::Call {
                    callee: "zirk_char_codepoint_count".to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I32),
                span,
            ),
            "ascii_code" => self.emit(
                InstKind::Call {
                    callee: "zirk_char_ascii_code".to_string(),
                    args: vec![receiver],
                },
                IrType::Int(IntWidth::I32),
                span,
            ),
            "is_ascii" | "is_alphabetic" | "is_numeric" | "is_alphanumeric" | "is_uppercase"
            | "is_lowercase" | "is_digit" | "is_letter" | "is_whitespace" => self.emit(
                InstKind::Call {
                    callee: format!("zirk_char_{member}"),
                    args: vec![receiver],
                },
                IrType::Boolean,
                span,
            ),
            _ => return None,
        })
    }

    fn lower_duration_property(
        &mut self,
        object: &ast::Expr,
        member: &str,
        span: Span,
    ) -> Option<Operand> {
        let receiver = self.lower_expr(object);
        let (callee, returns) = match member {
            "days" => ("zirk_duration_days", IrType::Int(IntWidth::I64)),
            "hours" => ("zirk_duration_hours", IrType::Int(IntWidth::I32)),
            "minutes" => ("zirk_duration_minutes", IrType::Int(IntWidth::I32)),
            "seconds" => ("zirk_duration_seconds", IrType::Int(IntWidth::I32)),
            "milliseconds" => ("zirk_duration_milliseconds", IrType::Int(IntWidth::I32)),
            "microseconds" => ("zirk_duration_microseconds", IrType::Int(IntWidth::I32)),
            "nanoseconds" => ("zirk_duration_nanoseconds", IrType::Int(IntWidth::I32)),
            _ => return None,
        };
        Some(self.emit(
            InstKind::Call {
                callee: callee.to_string(),
                args: vec![receiver],
            },
            returns,
            span,
        ))
    }

    /// `d.year` / `t.hour` / `dt.date` (`date-and-time-types`): the read-only
    /// component projections of the civil temporal types. `DateTime`'s own
    /// components compose the two primitives — its `year` is the date
    /// half's `year`, its `hour` the time half's.
    fn lower_temporal_property(
        &mut self,
        object: &ast::Expr,
        member: &str,
        span: Span,
    ) -> Option<Operand> {
        let ty = *self.checked.expr_types.get(&object.span())?;
        let receiver = self.lower_expr(object);
        match ty.base {
            Base::Date => {
                let i32_call = |this: &mut Self, callee: &str| {
                    this.emit(
                        InstKind::Call {
                            callee: callee.to_string(),
                            args: vec![receiver],
                        },
                        IrType::Int(IntWidth::I32),
                        span,
                    )
                };
                Some(match member {
                    "year" => i32_call(self, "zirk_rt_date_year"),
                    "month" => i32_call(self, "zirk_rt_date_month"),
                    "day" => i32_call(self, "zirk_rt_date_day"),
                    // `temporal-rich-api` calendrical properties.
                    "day_of_week" => i32_call(self, "zirk_rt_date_day_of_week"),
                    "day_of_year" => i32_call(self, "zirk_rt_date_day_of_year"),
                    "week_of_year" => i32_call(self, "zirk_rt_date_week_of_year"),
                    "quarter" => i32_call(self, "zirk_rt_date_quarter"),
                    "days_in_month" => i32_call(self, "zirk_rt_date_days_in_month"),
                    "days_in_year" => i32_call(self, "zirk_rt_date_days_in_year"),
                    "is_leap_year" => self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_date_is_leap".to_string(),
                            args: vec![receiver],
                        },
                        IrType::Boolean,
                        span,
                    ),
                    _ => return None,
                })
            }
            Base::Time => {
                // `millisecond`/`microsecond` are pure arithmetic on the
                // sub-second nanosecond field — no runtime call.
                if member == "millisecond" || member == "microsecond" {
                    let i64ty = IrType::Int(IntWidth::I64);
                    let one_billion = self.const_int_at(1_000_000_000, i64ty, span);
                    let frac = self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Rem,
                            left: receiver,
                            right: one_billion,
                        },
                        i64ty,
                        span,
                    );
                    let divisor = if member == "millisecond" {
                        1_000_000
                    } else {
                        1_000
                    };
                    let unit = self.const_int_at(divisor, i64ty, span);
                    let total = self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Div,
                            left: frac,
                            right: unit,
                        },
                        i64ty,
                        span,
                    );
                    // `microsecond` is the µs within the millisecond
                    // (0–999): take it mod 1000.
                    let thousand = self.const_int_at(1_000, i64ty, span);
                    let bounded = self.emit(
                        InstKind::Binary {
                            op: BinaryOp::Rem,
                            left: total,
                            right: thousand,
                        },
                        i64ty,
                        span,
                    );
                    return Some(self.emit(
                        InstKind::IntCast(bounded),
                        IrType::Int(IntWidth::I32),
                        span,
                    ));
                }
                let (callee, returns) = match member {
                    "hour" => ("zirk_rt_time_hour", IrType::Int(IntWidth::I32)),
                    "minute" => ("zirk_rt_time_minute", IrType::Int(IntWidth::I32)),
                    "second" => ("zirk_rt_time_second", IrType::Int(IntWidth::I32)),
                    "nanosecond" => ("zirk_rt_time_nanosecond", IrType::Int(IntWidth::I64)),
                    _ => return None,
                };
                Some(self.emit(
                    InstKind::Call {
                        callee: callee.to_string(),
                        args: vec![receiver],
                    },
                    returns,
                    span,
                ))
            }
            Base::DateTime => {
                let days = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_datetime_days".to_string(),
                        args: vec![receiver],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                );
                let nanos = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_datetime_nanos".to_string(),
                        args: vec![receiver],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                );
                // Date-part members delegate to the `zirk_rt_date_*`
                // projections over the day count; time-part members to the
                // `zirk_rt_time_*` ones over the day's nanoseconds.
                let i32_of = |this: &mut Self, callee: &str, arg: Operand| {
                    this.emit(
                        InstKind::Call {
                            callee: callee.to_string(),
                            args: vec![arg],
                        },
                        IrType::Int(IntWidth::I32),
                        span,
                    )
                };
                Some(match member {
                    "date" => days,
                    "time" => nanos,
                    "year" => i32_of(self, "zirk_rt_date_year", days),
                    "month" => i32_of(self, "zirk_rt_date_month", days),
                    "day" => i32_of(self, "zirk_rt_date_day", days),
                    "day_of_week" => i32_of(self, "zirk_rt_date_day_of_week", days),
                    "day_of_year" => i32_of(self, "zirk_rt_date_day_of_year", days),
                    "week_of_year" => i32_of(self, "zirk_rt_date_week_of_year", days),
                    "quarter" => i32_of(self, "zirk_rt_date_quarter", days),
                    "days_in_month" => i32_of(self, "zirk_rt_date_days_in_month", days),
                    "days_in_year" => i32_of(self, "zirk_rt_date_days_in_year", days),
                    "is_leap_year" => self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_date_is_leap".to_string(),
                            args: vec![days],
                        },
                        IrType::Boolean,
                        span,
                    ),
                    "hour" => i32_of(self, "zirk_rt_time_hour", nanos),
                    "minute" => i32_of(self, "zirk_rt_time_minute", nanos),
                    "second" => i32_of(self, "zirk_rt_time_second", nanos),
                    "millisecond" | "microsecond" => {
                        // Same pure-IR derivation as the `Time` arm.
                        let i64ty = IrType::Int(IntWidth::I64);
                        let billion = self.const_int_at(1_000_000_000, i64ty, span);
                        let frac = self.emit(
                            InstKind::Binary {
                                op: BinaryOp::Rem,
                                left: nanos,
                                right: billion,
                            },
                            i64ty,
                            span,
                        );
                        let divisor = if member == "millisecond" {
                            1_000_000
                        } else {
                            1_000
                        };
                        let unit = self.const_int_at(divisor, i64ty, span);
                        let total = self.emit(
                            InstKind::Binary {
                                op: BinaryOp::Div,
                                left: frac,
                                right: unit,
                            },
                            i64ty,
                            span,
                        );
                        let thousand = self.const_int_at(1_000, i64ty, span);
                        let bounded = self.emit(
                            InstKind::Binary {
                                op: BinaryOp::Rem,
                                left: total,
                                right: thousand,
                            },
                            i64ty,
                            span,
                        );
                        self.emit(InstKind::IntCast(bounded), IrType::Int(IntWidth::I32), span)
                    }
                    "nanosecond" => self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_time_nanosecond".to_string(),
                            args: vec![nanos],
                        },
                        IrType::Int(IntWidth::I64),
                        span,
                    ),
                    _ => return None,
                })
            }
            _ => None,
        }
    }

    fn lower_tuple_length(&mut self, object: &ast::Expr, span: Span) -> Operand {
        let count = if let ast::Expr::Tuple(t) = object {
            t.elements.len()
        } else {
            let &ty = self
                .checked
                .expr_types
                .get(&object.span())
                .expect("a typed tuple expression has a semantic type");
            let Base::Tuple(id) = ty.base else {
                unreachable!("the checker only types `.length` on a tuple expression")
            };
            self.checked.tuple_types[id as usize].elements.len()
        };
        self.emit(
            InstKind::ConstInt(count as i128),
            IrType::Int(IntWidth::I32),
            span,
        )
    }

    fn lower_enum_property(&mut self, expr: &ast::FieldExpr, span: Span) -> Option<Operand> {
        let object = &expr.object;
        let name = expr.name.name.as_str();
        if !matches!(name, "name" | "value" | "to_string") {
            return None;
        }
        let sema = self.checked.expr_types.get(&object.span())?;
        if !matches!(sema.base, Base::Enum(_) | Base::EnumInstance(_)) {
            return None;
        }

        // For a variant access (`Color.Red`) the case is known now.
        if let ast::Expr::Field(variant_expr) = &**object
            && self.checked.variant_accesses.contains(&variant_expr.span)
        {
            let ast::Expr::Path(enum_name) = &*variant_expr.object else {
                unreachable!("a variant access names its enum")
            };
            let enum_id = self.enum_id_of(enum_name);
            let discriminant = self.discriminant(enum_name, &variant_expr.name.name) as usize;
            let variant_name = self.checked.enums[enum_id as usize].variants[discriminant]
                .name
                .clone();
            return if name == "value" {
                Some(self.emit(
                    InstKind::ConstInt(discriminant as i128),
                    IrType::Int(IntWidth::I32),
                    span,
                ))
            } else {
                Some(self.const_string(&variant_name, span))
            };
        }

        // For a runtime `Enum` value, emit a switch over the discriminant.
        // A traditional enum *is* its `Int32` discriminant once lowered —
        // `value` answers the operand itself and `name`/`to_string` still
        // switch over it; only an algebraic value needs `Discriminant`.
        let receiver = self.lower_expr(object);
        let receiver_ty = self.type_of(object, object.span());
        let (enum_id, disc) = match receiver_ty {
            IrType::Enum(enum_id) => (
                enum_id,
                self.emit(
                    InstKind::Discriminant(receiver),
                    IrType::Int(IntWidth::I32),
                    span,
                ),
            ),
            IrType::Int(_) => {
                let Base::Enum(enum_id) = sema.base else {
                    unreachable!("a non-payload enum value lowers to its Int32 discriminant")
                };
                if name == "value" {
                    return Some(receiver);
                }
                (enum_id, receiver)
            }
            _ => unreachable!("an enum value lowers to IrType::Enum or its Int32 discriminant"),
        };
        let result = self.declare_slot(
            "<enum_prop>",
            if name == "value" {
                IrType::Int(IntWidth::I32)
            } else {
                IrType::String
            },
            span,
        );
        // Each arm's test runs in a block of its own, and an IR value never
        // crosses a block boundary — the discriminant goes through a slot
        // and is reloaded per test.
        let disc_slot = self.spill(disc, IrType::Int(IntWidth::I32), span);

        let variants = self.checked.enums[enum_id as usize].variants.clone();
        let continue_block = self.new_block();
        let mut current = self.current;
        for (i, variant) in variants.iter().enumerate() {
            let arm = self.new_block();
            self.current = current;
            let disc = self.emit(InstKind::Load(disc_slot), IrType::Int(IntWidth::I32), span);
            let index = self.emit(
                InstKind::ConstInt(i as i128),
                IrType::Int(IntWidth::I32),
                span,
            );
            let test = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left: disc,
                    right: index,
                },
                IrType::Boolean,
                span,
            );
            let next = self.new_block();
            self.terminate(Terminator::Branch {
                condition: test,
                then_block: arm,
                else_block: next,
            });
            self.current = arm;
            let value = if name == "value" {
                self.emit(
                    InstKind::ConstInt(i as i128),
                    IrType::Int(IntWidth::I32),
                    span,
                )
            } else {
                self.const_string(&variant.name, span)
            };
            self.emit_effect(InstKind::Store(result, value), span);
            self.terminate(Terminator::Jump(continue_block));
            current = next;
        }
        self.current = current;
        let fallback = if name == "value" {
            self.emit(InstKind::ConstInt(0), IrType::Int(IntWidth::I32), span)
        } else {
            self.const_string("", span)
        };
        self.emit_effect(InstKind::Store(result, fallback), span);
        self.terminate(Terminator::Jump(continue_block));
        self.current = continue_block;
        Some(self.emit(
            InstKind::Load(result),
            if name == "value" {
                IrType::Int(IntWidth::I32)
            } else {
                IrType::String
            },
            span,
        ))
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
        // `ClassName.field` that names a `static` field of a user class.
        if let ast::Expr::Path(base) = &*expr.object
            && let Some(class_id) = self
                .checked
                .classes
                .iter()
                .position(|c| c.name == base.name)
            && let Some(field) = self.checked.classes[class_id].field(&expr.name.name)
            && field.is_static
        {
            return self.ir_type(field.ty);
        }

        // Universal `.type` member: a `String` with the value's type name.
        if expr.name.name == "type" {
            return IrType::String;
        }

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
        // `.is_alive` (roadmap Phase 4e, `fase-4e-weak`) — see
        // `Self::lower_field`'s matching branch.
        if expr.name.name == "is_alive"
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::Weak(_)
            )
        {
            return IrType::Boolean;
        }
        // `regex.matches` / `regex.replace` are not real fields, but the
        // type-of pass for `Expr::Call` asks for their field type to decide the
        // call shape.  Check the name first: `expr.object` may be a type name
        // for a variant construction like `Wrapper.Inner(...)`.
        if matches!(
            expr.name.name.as_str(),
            "matches" | "replace" | "find" | "split" | "find_all"
        ) && self.type_of(&expr.object, expr.object.span()) == IrType::Regex
        {
            return match expr.name.name.as_str() {
                "matches" => IrType::Boolean,
                "replace" => IrType::String,
                "find" => IrType::Nullable(Nullable::Object(self.checked.regex_match_class)),
                "split" => {
                    let id = self
                        .checked
                        .list_types
                        .iter()
                        .position(|&t| t == Type::STRING)
                        .expect("the checker interned `List<String>` for `Regex.split`")
                        as u32;
                    IrType::List(id)
                }
                "find_all" => IrType::List(self.regex_match_list_id()),
                _ => unreachable!("name checked above"),
            };
        }
        // `.length`/`.is_empty` (roadmap Phase 4e, `fase-4e-native-slice`) —
        // see `Self::lower_field`'s matching branch, member name checked
        // first for the same reason.
        if matches!(expr.name.name.as_str(), "length" | "is_empty")
            && matches!(
                self.type_of(&expr.object, expr.object.span()),
                IrType::NativeSlice(_)
                    | IrType::NativeSliceMut(_)
                    | IrType::Array(_)
                    | IrType::List(_)
                    | IrType::Map(_)
                    | IrType::Set(_)
            )
        {
            match expr.name.name.as_str() {
                "length" => return IrType::Int(IntWidth::U64),
                "is_empty" => return IrType::Boolean,
                _ => unreachable!("the checker only types `.length`/`.is_empty` on a collection"),
            }
        }
        // `s.split` named as a callee: `is_string_method_call` classifies
        // the call, so this type is only ever asked whether it is
        // `Callable` — `List<String>` answers that without
        // `field_position`'s layout walk, which a string has no part in
        // (same reasoning as `r.reverse` below).
        if expr.name.name == "split"
            && self.type_of(&expr.object, expr.object.span()) == IrType::String
        {
            return IrType::List(self.string_list_id());
        }
        // `d.abs`/`d.sign`/`d.is_*` named as callees (`native-type-member-
        // surface`): a `Duration` lowers to `Int64`, which has no field
        // layout — answer with the method's own return type instead.
        if matches!(
            expr.name.name.as_str(),
            "abs" | "sign" | "is_zero" | "is_positive" | "is_negative"
        ) && self
            .checked
            .expr_types
            .get(&expr.object.span())
            .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Duration))
        {
            return match expr.name.name.as_str() {
                "abs" => IrType::Int(IntWidth::I64),
                "sign" => IrType::Int(IntWidth::I32),
                _ => IrType::Boolean,
            };
        }
        // `native-type-member-surface`: static members of scalar type
        // names (`Int32.MAX`, `Float64.EPSILON`) — the checker already
        // resolved the base as a type, not a value.
        if self.checked.scalar_static_accesses.contains(&expr.span) {
            let ast::Expr::Path(base) = &*expr.object else {
                unreachable!("a scalar static access names a type")
            };
            let Some(ty) = Type::from_name(&base.name) else {
                unreachable!("the checker only records existing scalar types")
            };
            return self.ir_type(ty);
        }

        // `s.length` / `s.byte_length` / `c.byte_length` /
        // `c.codepoint_count` / `c.ascii_code` — the runtime-provided
        // property surface.
        if matches!(expr.name.name.as_str(), "length" | "byte_length")
            && self.type_of(&expr.object, expr.object.span()) == IrType::String
        {
            return IrType::Int(IntWidth::I32);
        }
        if matches!(
            expr.name.name.as_str(),
            "byte_length" | "codepoint_count" | "ascii_code"
        ) && self.type_of(&expr.object, expr.object.span()) == IrType::Char
        {
            return IrType::Int(IntWidth::I32);
        }
        if matches!(
            expr.name.name.as_str(),
            "is_ascii"
                | "is_alphabetic"
                | "is_numeric"
                | "is_alphanumeric"
                | "is_uppercase"
                | "is_lowercase"
                | "is_digit"
                | "is_letter"
                | "is_whitespace"
        ) && self.type_of(&expr.object, expr.object.span()) == IrType::Char
        {
            return IrType::Boolean;
        }
        if expr.name.name == "is_empty"
            && self.type_of(&expr.object, expr.object.span()) == IrType::String
        {
            return IrType::Boolean;
        }

        // `t.length` on a `Tuple` (the doc surface) and `.name`/`.value` /
        // `.to_string` on an `Enum` value or variant access.
        if expr.name.name == "length"
            && self
                .checked
                .expr_types
                .get(&expr.object.span())
                .is_some_and(|ty| matches!(ty.base, Base::Tuple(_)))
        {
            return IrType::Int(IntWidth::I32);
        }
        if matches!(expr.name.name.as_str(), "name" | "value" | "to_string")
            && self
                .checked
                .expr_types
                .get(&expr.object.span())
                .is_some_and(|ty| matches!(ty.base, Base::Enum(_) | Base::EnumInstance(_)))
        {
            return match expr.name.name.as_str() {
                "name" | "to_string" => IrType::String,
                "value" => IrType::Int(IntWidth::I32),
                _ => unreachable!(),
            };
        }

        // `r.start`/`r.end`/`r.step` on a `Range<T>` (roadmap Phase 7) —
        // the member's type is the range's element type; see
        // `Self::lower_field`'s matching branch.
        if matches!(expr.name.name.as_str(), "start" | "end" | "step")
            && self.type_of(&expr.object, expr.object.span()) == IrType::Range
        {
            return self
                .checked
                .expr_types
                .get(&expr.object.span())
                .and_then(|ty| match ty.base {
                    Base::Range(id) => Some(self.checked.range_types[id as usize]),
                    _ => None,
                })
                .map(|element| self.ir_type(element))
                .unwrap_or(IrType::Int(IntWidth::I64));
        }
        // `r.reverse` named as a callee (roadmap Phase 7): `is_callable_call`
        // and `is_range_method_call` classify the call, so this type is
        // only ever asked whether it is `Callable` — `Range` answers that
        // without `field_position`'s layout walk, which a range has no
        // part in.
        if expr.name.name == "reverse"
            && self.type_of(&expr.object, expr.object.span()) == IrType::Range
        {
            return IrType::Range;
        }
        // Duration component properties (`days`, `hours`, ...).
        if matches!(
            expr.name.name.as_str(),
            "days"
                | "hours"
                | "minutes"
                | "seconds"
                | "milliseconds"
                | "microseconds"
                | "nanoseconds"
        ) && self
            .checked
            .expr_types
            .get(&expr.object.span())
            .is_some_and(|ty| matches!(ty.base, Base::Duration))
        {
            return if expr.name.name == "days" {
                IrType::Int(IntWidth::I64)
            } else {
                IrType::Int(IntWidth::I32)
            };
        }
        // Civil temporal components (`date-and-time-types`): `d.year`,
        // `dt.date`, … — no layout slot exists on an `i64`/`i128`, so the
        // checker's recorded member type answers (`expr_types` holds the
        // `Field` expression's own type). Method callees (`d.start_of`,
        // `d.is_before`) get no `expr_types` entry of their own — like
        // `Duration`'s, they only ever need to answer "is this a
        // `Callable`?", which the member name's own return type settles.
        if self
            .checked
            .expr_types
            .get(&expr.object.span())
            .is_some_and(|ty| matches!(ty.base, Base::Date | Base::Time | Base::DateTime))
        {
            if let Some(&ty) = self.checked.expr_types.get(&expr.span)
                && !ty.is_unknown()
            {
                return self.ir_type(ty);
            }
            return match expr.name.name.as_str() {
                "is_before" | "is_after" | "is_same" | "is_same_or_before" | "is_same_or_after"
                | "is_between" | "is_weekday" | "is_weekend" => IrType::Boolean,
                "format" | "to_iso_string" | "to_string" => IrType::String,
                "start_of" | "end_of" | "with_year" | "with_month" | "with_day" | "with_hour"
                | "with_minute" | "with_second" | "with_nanosecond" | "with_date" | "with_time" => {
                    self.type_of(&expr.object, expr.object.span())
                }
                _ => IrType::Int(IntWidth::I32),
            };
        }
        // `Direction.count` and an `E.*`/`Enums.*` static callee
        // (`enum-static-members`): no layout slot exists to point at — the
        // checker recorded the member's type for `count`, and a call's
        // callee is only ever asked here whether it is `Callable` (it is
        // not). This precedes the `variant_accesses` branch below: the
        // callee spans share that set, but `Enums` names no enum —
        // `enum_id_of` would not resolve it.
        if self.checked.enum_static_accesses.contains(&expr.span) {
            return self
                .checked
                .expr_types
                .get(&expr.span)
                .copied()
                .filter(|ty| !ty.is_unknown())
                .map(|ty| self.ir_type(ty))
                .unwrap_or(IrType::Int(IntWidth::I32));
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
            "read" | "write" | "offset" | "offset_bytes" | "as_slice" | "as_slice_mut"
        ) {
            return false;
        }
        matches!(
            self.type_of(&field.object, field.object.span()),
            IrType::Pointer(_)
        )
    }

    /// Returns the id of `Pointer<pointee>` in `module.pointer_types`,
    /// allocating a new entry if the pointee was not interned by the checker
    /// (this happens for intermediate containers of a nested field access).
    fn intern_pointer_type(&mut self, pointee: IrType) -> u32 {
        if let Some(index) = self.module.pointer_types.iter().position(|&t| t == pointee) {
            index as u32
        } else {
            let id = self.module.pointer_types.len() as u32;
            self.module.pointer_types.push(pointee);
            id
        }
    }

    /// `Pointer.from(place)` (design D8): the address already computed for
    /// `place`'s own storage — no allocation, just exposing it.
    ///
    /// For a `record` field, the chain is built from the
    /// innermost lvalue out: the root slot gives a `Pointer<Root>`, each
    /// intermediate field `PointerFromField`s to the next value, and the
    /// final field is the `Pointer<T>` requested by the user.
    fn lower_pointer_from(&mut self, place: &ast::Expr, span: Span) -> Operand {
        let pointee = self.type_of(place, place.span());
        let id = self.intern_pointer_type(pointee);
        let ty = IrType::Pointer(id);

        match place {
            ast::Expr::Path(ident) => {
                let slot = self.lookup_slot(&ident.name);
                self.emit(InstKind::PointerFromSlot(slot), ty, span)
            }
            ast::Expr::Field(field) => {
                let (index, _) = self.field_position(&field.object, &field.name.name);
                let object = self.lower_pointer_from(&field.object, span);
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
        let pointer_ty = IrType::Pointer(id);

        // The pointer is held in a slot so it survives an argument that
        // opens a branch (`write(v + 1)`, `offset(n * 2)`, etc.).
        let pointer_slot = self.spill(pointer, pointer_ty, span);

        match field.name.name.as_str() {
            "read" => {
                let pointer = self.emit(InstKind::Load(pointer_slot), pointer_ty, span);
                let t = self.module.pointer_types[id as usize];
                self.emit(InstKind::PointerRead(pointer), t, span)
            }
            "write" => {
                let t = self.module.pointer_types[id as usize];
                let value = self.lower_expr_as(&e.args[0].value, t);
                let pointer = self.emit(InstKind::Load(pointer_slot), pointer_ty, span);
                self.emit(
                    InstKind::PointerWrite { pointer, value },
                    IrType::Void,
                    span,
                )
            }
            "offset" => {
                let amount = self.lower_expr_as(&e.args[0].value, IrType::Int(IntWidth::I32));
                let pointer = self.emit(InstKind::Load(pointer_slot), pointer_ty, span);
                self.emit(
                    InstKind::PointerOffset { pointer, amount },
                    pointer_ty,
                    span,
                )
            }
            "offset_bytes" => {
                let amount = self.lower_expr_as(&e.args[0].value, IrType::Int(IntWidth::I32));
                let pointer = self.emit(InstKind::Load(pointer_slot), pointer_ty, span);
                self.emit(
                    InstKind::PointerOffsetBytes { pointer, amount },
                    pointer_ty,
                    span,
                )
            }
            // `.as_slice(length)`/`.as_slice_mut(length)` (roadmap Phase
            // 4e, `fase-4e-native-slice`, design D1/D3/D5): validated
            // construction, producing `Result<..., NativeError>`.
            "as_slice" | "as_slice_mut" => {
                let mutable = field.name.name == "as_slice_mut";
                let element = self.module.pointer_types[id as usize];
                // `pointer` was already spilled above before the length
                // argument below gets a chance to open a block of its own.

                let length = self.lower_expr_as(&e.args[0].value, IrType::Int(IntWidth::U64));
                let known_length = self.known_pointer_extent(&field.object);
                self.lower_native_slice_construction(
                    NativeSliceConstruction {
                        pointer_slot,
                        length,
                        pointer_type_id: id,
                        element,
                        mutable,
                        known_length,
                    },
                    span,
                )
            }
            _ => unreachable!("checked by `Self::is_pointer_method_call`"),
        }
    }

    /// This pass's own scope decision on design D3's "where the underlying
    /// allocation's own size is knowable" (see `InstKind::NativeSliceValidate`'s
    /// own doc comment): recognizes only the syntactically direct
    /// `Pointer.from(place).as_slice(n)` shape, where `place`'s own storage
    /// holds exactly one element — not general provenance/dataflow
    /// tracking. `None` (opaque provenance) for everything else, including
    /// a `Pointer<T>` read out of a variable.
    fn known_pointer_extent(&self, receiver: &ast::Expr) -> Option<u64> {
        let ast::Expr::Call(call) = receiver else {
            return None;
        };
        if self.is_pointer_from_call(call) {
            Some(1)
        } else {
            None
        }
    }

    /// `pointer.as_slice(length)`/`.as_slice_mut(length)` (design D1/D3/D5):
    /// validates via the runtime, then builds
    /// `Result<NativeSlice<T>|NativeSliceMut<T>, NativeError>` — the same
    /// branch/store/join shape `Self::checked_int_division` and
    /// `Self::lower_if_expr` already use for a conditionally produced
    /// value, since a single `InstKind` cannot itself branch (control flow
    /// is expressed as IR blocks, never hidden inside one instruction's own
    /// codegen — `ADR-007`).
    ///
    /// `pointer_slot` is already spilled by the caller (`Self::lower_pointer_method_call`'s
    /// own `as_slice`/`as_slice_mut` arm), immediately after `pointer` was
    /// lowered and before the length argument had a chance to open a block
    /// of its own — `length` itself is spilled here, right on entry, before
    /// anything else in this function gets that same chance.
    fn lower_native_slice_construction(
        &mut self,
        construction: NativeSliceConstruction,
        span: Span,
    ) -> Operand {
        let NativeSliceConstruction {
            pointer_slot,
            length,
            pointer_type_id,
            element,
            mutable,
            known_length,
        } = construction;
        let length_slot = self.spill(length, IrType::Int(IntWidth::U64), span);

        let view_ty = if mutable {
            let id = self.module.intern_native_slice_mut_type(element);
            IrType::NativeSliceMut(id)
        } else {
            let id = self.module.intern_native_slice_type(element);
            IrType::NativeSlice(id)
        };
        let result_ty = self.native_slice_result_type(pointer_type_id, mutable);
        let IrType::Enum(result_enum) = result_ty else {
            unreachable!("Result<T,E> always lowers to IrType::Enum")
        };

        let result_slot = self.declare_slot("<native_slice_result>", result_ty, span);

        let pointer_for_check = self.emit(
            InstKind::Load(pointer_slot),
            IrType::Pointer(pointer_type_id),
            span,
        );
        let length_for_check = self.emit(
            InstKind::Load(length_slot),
            IrType::Int(IntWidth::U64),
            span,
        );
        let is_valid = self.emit(
            InstKind::NativeSliceValidate {
                pointer: pointer_for_check,
                length: length_for_check,
                element,
                known_length,
            },
            IrType::Boolean,
            span,
        );

        let ok_block = self.new_block();
        let error_block = self.new_block();
        let continue_block = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_valid,
            then_block: ok_block,
            else_block: error_block,
        });

        self.current = ok_block;
        let pointer_ok = self.emit(
            InstKind::Load(pointer_slot),
            IrType::Pointer(pointer_type_id),
            span,
        );
        let length_ok = self.emit(
            InstKind::Load(length_slot),
            IrType::Int(IntWidth::U64),
            span,
        );
        let view = self.emit(
            InstKind::NativeSliceValue {
                pointer: pointer_ok,
                length: length_ok,
            },
            view_ty,
            span,
        );
        let ok_value = self.emit(
            InstKind::BuildEnum {
                enum_id: result_enum,
                variant: 0,
                fields: vec![view],
            },
            result_ty,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, ok_value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = error_block;
        let native = self
            .checked
            .native_exceptions
            .expect("a program calling as_slice/as_slice_mut registered the exception hierarchy");
        let error_object = self.build_native_failure(
            native.native_error,
            "invalid NativeSlice/NativeSliceMut construction: null, misaligned, unrepresentable extent, or length exceeds the known allocation",
            span,
        );
        let error_value = self.emit(
            InstKind::BuildEnum {
                enum_id: result_enum,
                variant: 1,
                fields: vec![error_object],
            },
            result_ty,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, error_value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result_slot), result_ty, span)
    }

    /// The `Result<NativeSlice<T>|NativeSliceMut<T>, NativeError>` `IrType`
    /// `.as_slice`/`.as_slice_mut` produce — re-finds the exact
    /// `checked.enum_instances` entry `Checker::native_result_type` already
    /// interned for this `(view, NativeError)` pair, the same
    /// re-derivation `Self::type_of`'s `Pointer<T>`/`Weak<T>` arms already
    /// do for their own tables.
    fn native_slice_result_type(&self, pointer_type_id: u32, mutable: bool) -> IrType {
        let pointee = self.checked.pointer_types[pointer_type_id as usize];
        let view = if mutable {
            let id = self
                .checked
                .native_slice_mut_types
                .iter()
                .position(|&t| t == pointee)
                .expect("the checker interned this NativeSliceMut<T>") as u32;
            Type::of(Base::NativeSliceMut(id))
        } else {
            let id = self
                .checked
                .native_slice_types
                .iter()
                .position(|&t| t == pointee)
                .expect("the checker interned this NativeSlice<T>") as u32;
            Type::of(Base::NativeSlice(id))
        };
        let native = self
            .checked
            .native_exceptions
            .expect("a program calling as_slice/as_slice_mut registered the exception hierarchy");
        let native_result = self
            .checked
            .native_result
            .expect("register_native_result_enum runs before any type is checked");
        let args = vec![view, Type::of(Base::Class(native.native_error))];
        let id = self
            .checked
            .enum_instances
            .iter()
            .position(|inst| inst.enum_id == native_result && inst.args == args)
            .expect("the checker interned this Result<view, NativeError> instantiation")
            as u32;
        self.ir_type(Type::of(Base::EnumInstance(id)))
    }

    /// The element type a `NativeSlice<T>`/`NativeSliceMut<T>` receiver
    /// carries (roadmap Phase 4e, `fase-4e-native-slice`, design D5) —
    /// used by indexing and assignment-target typing.
    fn native_slice_element_type(&self, receiver_ty: IrType) -> IrType {
        match receiver_ty {
            IrType::NativeSlice(id) => self.module.native_slice_types[id as usize],
            IrType::NativeSliceMut(id) => self.module.native_slice_mut_types[id as usize],
            _ => unreachable!(
                "the checker only accepts a NativeSlice/NativeSliceMut receiver for indexing"
            ),
        }
    }

    /// The element type of an `Array<T>` or `List<T>` receiver.
    fn array_list_element_type(&self, receiver_ty: IrType) -> IrType {
        match receiver_ty {
            IrType::Array(id) => self.module.array_types[id as usize],
            IrType::List(id) => self.module.list_types[id as usize],
            _ => unreachable!("the checker only accepts an Array/List receiver here"),
        }
    }

    /// `receiver[index]` (design D5): bounds-checks `index` against the
    /// receiver's own carried length, throwing `IndexOutOfBoundsError` on
    /// failure (design D3's "controlled bounds error", spec scenario "View
    /// indexing stays bounds-checked") — the same guard shape
    /// `Self::checked_int_division` uses, reloading both operands on the
    /// `cont` block for the caller to use in the actual load/store.
    ///
    /// `receiver_slot`/`index_slot` are already spilled by the caller
    /// (`Self::lower_index_read`/`Self::lower_assign`'s own `Index` arm),
    /// since this always opens a `fail`/`cont` split regardless of whether
    /// anything the caller lowered before it does.
    fn lower_native_slice_bounds_check(
        &mut self,
        receiver_slot: SlotId,
        receiver_ty: IrType,
        index_slot: SlotId,
        span: Span,
    ) -> (Operand, Operand) {
        let receiver_check = self.emit(InstKind::Load(receiver_slot), receiver_ty, span);
        let length = self.emit(
            InstKind::NativeSliceLength(receiver_check),
            IrType::Int(IntWidth::U64),
            span,
        );
        let index_check = self.emit(InstKind::Load(index_slot), IrType::Int(IntWidth::U64), span);
        let out_of_bounds = self.emit(
            InstKind::Binary {
                op: BinaryOp::GtEq,
                left: index_check,
                right: length,
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: out_of_bounds,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program indexing a native view registered the exception hierarchy");
        self.throw_native_failure(
            native.index_out_of_bounds,
            "native view index out of bounds",
            span,
        );

        self.current = cont;
        let receiver = self.emit(InstKind::Load(receiver_slot), receiver_ty, span);
        let index = self.emit(InstKind::Load(index_slot), IrType::Int(IntWidth::U64), span);
        (receiver, index)
    }

    /// `receiver[index]` read (design D5).
    ///
    /// `receiver` is spilled immediately after it is lowered, before
    /// `expr.index` gets a chance to open a block of its own and strand it
    /// (ADR-007 D7) — `a[cond ? 0 : 1]` is exactly this shape.
    fn lower_index_read(&mut self, expr: &ast::IndexExpr, span: Span) -> Operand {
        let receiver = self.lower_expr(&expr.receiver);
        let receiver_ty = self.type_of_operand(receiver);
        let receiver_slot = self.spill(receiver, receiver_ty, span);

        if receiver_ty == IrType::String {
            return self.lower_string_index_read(receiver_slot, &expr.index, span);
        }

        if matches!(receiver_ty, IrType::Array(_) | IrType::List(_)) {
            return self.lower_array_list_index_read(receiver_slot, &expr.index, receiver_ty, span);
        }

        if let IrType::Value(layout_id) = receiver_ty {
            let ast::Expr::Int(lit) = &*expr.index else {
                unreachable!("a tuple index is a constant integer literal")
            };
            let index = lit.value as u32;
            let field_ty = self.module.values[layout_id as usize].fields[index as usize].ty;
            let object = self.emit(InstKind::Load(receiver_slot), receiver_ty, span);
            return self.emit(InstKind::LoadField { object, index }, field_ty, span);
        }

        let index_operand = self.lower_expr_as(&expr.index, IrType::Int(IntWidth::U64));
        let index_slot = self.spill(index_operand, IrType::Int(IntWidth::U64), span);

        let (receiver, index) =
            self.lower_native_slice_bounds_check(receiver_slot, receiver_ty, index_slot, span);
        let element_ty = self.native_slice_element_type(receiver_ty);
        self.emit(
            InstKind::NativeSliceLoad { receiver, index },
            element_ty,
            span,
        )
    }

    /// `array[i]` / `list[i]` read for `Array<T>` and `List<T>`.
    fn lower_array_list_index_read(
        &mut self,
        receiver_slot: SlotId,
        index_expr: &ast::Expr,
        receiver_ty: IrType,
        span: Span,
    ) -> Operand {
        let element_ty = self.array_list_element_type(receiver_ty);
        let receiver = self.emit(InstKind::Load(receiver_slot), receiver_ty, span);
        let index = self.lower_expr_as(index_expr, IrType::Int(IntWidth::I64));
        self.emit(
            InstKind::ArrayListLoad { receiver, index },
            element_ty,
            span,
        )
    }

    /// `s[start:end:step]` for `String` (roadmap Phase 7): every part is an
    /// `Int64` at the ABI, `i64::MIN` marking a part the source left out —
    /// the runtime resolves defaults against the actual grapheme count,
    /// which only it can see. Receiver and each bound are spilled as they
    /// lower so a bound that opens a block of its own cannot strand an
    /// earlier operand (ADR-007).
    fn lower_slice(&mut self, expr: &ast::SliceExpr, span: Span) -> Operand {
        let i64_ty = IrType::Int(IntWidth::I64);
        // `r[lo:hi:st]` (roadmap Phase 7, `Range<T>`): the runtime resolves
        // the bounds against the range's own element sequence and returns a
        // new `Range` — the receiver is never mutated.
        if self.type_of(&expr.receiver, expr.receiver.span()) == IrType::Range {
            let receiver = self.lower_expr(&expr.receiver);
            let receiver_slot = self.spill(receiver, IrType::Range, span);
            let mut lower_part = |part: &Option<Box<ast::Expr>>| {
                let operand = match part {
                    Some(e) => self.lower_expr_as(e, i64_ty),
                    None => self.const_i64(i64::MIN as i128, span),
                };
                self.spill(operand, i64_ty, span)
            };
            let start = lower_part(&expr.start);
            let end = lower_part(&expr.end);
            let step = lower_part(&expr.step);
            let range = self.emit(InstKind::Load(receiver_slot), IrType::Range, span);
            let start = self.emit(InstKind::Load(start), i64_ty, span);
            let end = self.emit(InstKind::Load(end), i64_ty, span);
            let step = self.emit(InstKind::Load(step), i64_ty, span);
            return self.emit(
                InstKind::Call {
                    callee: "zirk_range_slice".to_string(),
                    args: vec![range, start, end, step],
                },
                IrType::Range,
                span,
            );
        }
        let receiver = self.lower_expr(&expr.receiver);
        let receiver_ty = self.type_of_operand(receiver);
        let receiver_slot = self.spill(receiver, receiver_ty, span);

        let mut lower_part = |part: &Option<Box<ast::Expr>>| {
            let operand = match part {
                Some(e) => self.lower_expr_as(e, i64_ty),
                None => self.const_i64(i64::MIN as i128, span),
            };
            self.spill(operand, i64_ty, span)
        };
        let start = lower_part(&expr.start);
        let end = lower_part(&expr.end);
        let step = lower_part(&expr.step);

        if matches!(receiver_ty, IrType::Array(_) | IrType::List(_)) {
            let receiver = self.emit(InstKind::Load(receiver_slot), receiver_ty, span);
            let start = self.emit(InstKind::Load(start), i64_ty, span);
            let end = self.emit(InstKind::Load(end), i64_ty, span);
            let step = self.emit(InstKind::Load(step), i64_ty, span);
            return self.emit(
                InstKind::ArraySlice {
                    receiver,
                    start,
                    end,
                    step,
                },
                receiver_ty,
                span,
            );
        }

        let string = self.emit(InstKind::Load(receiver_slot), IrType::String, span);
        let start = self.emit(InstKind::Load(start), i64_ty, span);
        let end = self.emit(InstKind::Load(end), i64_ty, span);
        let step = self.emit(InstKind::Load(step), i64_ty, span);
        self.emit(
            InstKind::Call {
                callee: "zirk_str_slice".to_string(),
                args: vec![string, start, end, step],
            },
            IrType::String,
            span,
        )
    }

    /// `s[i]` for `String`, returning a `Char` grapheme by copy.
    fn lower_string_index_read(
        &mut self,
        receiver_slot: SlotId,
        index_expr: &ast::Expr,
        span: Span,
    ) -> Operand {
        let index = self.lower_expr_as(index_expr, IrType::Int(IntWidth::I64));
        let index_slot = self.spill(index, IrType::Int(IntWidth::I64), span);

        let string = self.emit(InstKind::Load(receiver_slot), IrType::String, span);
        let index = self.emit(InstKind::Load(index_slot), IrType::Int(IntWidth::I64), span);
        let offset = self.emit(
            InstKind::StringGraphemeOffset { string, index },
            IrType::Int(IntWidth::I64),
            span,
        );
        let offset_slot = self.spill(offset, IrType::Int(IntWidth::I64), span);

        let minus_one = self.const_i64(-1, span);
        let is_out = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: offset,
                right: minus_one,
            },
            IrType::Boolean,
            span,
        );

        let fail = self.new_block();
        let cont = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_out,
            then_block: fail,
            else_block: cont,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program indexing a string registered the exception hierarchy");
        self.throw_native_failure(
            native.index_out_of_bounds,
            "string index out of bounds",
            span,
        );

        self.current = cont;
        let string = self.emit(InstKind::Load(receiver_slot), IrType::String, span);
        let offset = self.emit(
            InstKind::Load(offset_slot),
            IrType::Int(IntWidth::I64),
            span,
        );
        let length = self.emit(
            InstKind::GraphemeLenAt { string, offset },
            IrType::Int(IntWidth::I64),
            span,
        );
        self.emit(
            InstKind::GraphemeSlice {
                string,
                offset,
                len: length,
            },
            IrType::Char,
            span,
        )
    }

    /// Whether `e` is `Weak.from(value)` (roadmap Phase 4e, `fase-4e-weak`,
    /// design D1) — the compiler-built-in static call
    /// `Checker::check_weak_from` already recognized the same way.
    fn is_weak_from_call(&self, e: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*e.callee else {
            return false;
        };
        let ast::Expr::Path(base) = &*field.object else {
            return false;
        };
        base.name == "Weak"
            && field.name.name == "from"
            && self.try_lookup_slot(&base.name).is_none()
    }

    /// Whether `e` is `.upgrade()` on a `Weak<T>` receiver (design D4).
    /// `.is_alive` is not here — it is a member read
    /// (`Self::field_type_of`/`Self::lower_field`), never a call.
    fn is_weak_method_call(&self, e: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*e.callee else {
            return false;
        };
        if field.name.name != "upgrade" {
            return false;
        }
        matches!(
            self.type_of(&field.object, field.object.span()),
            IrType::Weak(_)
        )
    }

    /// Whether `e` is `Pin(value)` (roadmap Phase 4e, `phase-4e-memory`,
    /// design D1) — the compiler-built-in constructor `Checker::check_pin_construction`
    /// already recognized the same way.
    fn is_pin_call(&self, e: &ast::CallExpr) -> bool {
        let ast::Expr::Path(callee) = &*e.callee else {
            return false;
        };
        callee.name == "Pin" && self.try_lookup_slot(&callee.name).is_none()
    }

    /// `Weak.from(value)` (design D1): allocates a fresh WeakCell and stores
    /// `value`'s own address into it — `value` is already a managed
    /// reference, so unlike `Pointer.from` this lowers the value itself
    /// rather than taking the address of an addressable place.
    fn lower_weak_from(&mut self, value: &ast::Expr, span: Span) -> Operand {
        let referent = self.type_of(value, value.span());
        let id = self
            .module
            .weak_types
            .iter()
            .position(|&t| t == referent)
            .expect("the checker interned every Weak<T> it type-checked") as u32;
        let ty = IrType::Weak(id);
        let target = self.lower_expr(value);
        self.emit(InstKind::WeakFrom(target), ty, span)
    }

    /// `.upgrade()` (design D4).
    fn lower_weak_method_call(&mut self, e: &ast::CallExpr, span: Span) -> Operand {
        let ast::Expr::Field(field) = &*e.callee else {
            unreachable!("checked by `Self::is_weak_method_call`")
        };
        let weak = self.lower_expr(&field.object);
        let IrType::Weak(id) = self.type_of_operand(weak) else {
            unreachable!("checked by `Self::is_weak_method_call`")
        };
        let referent = self.module.weak_types[id as usize];
        let result_ty = IrType::Nullable(
            Nullable::of(referent).expect("a Weak<T> referent has a nullable form"),
        );
        self.emit(InstKind::WeakUpgrade(weak), result_ty, span)
    }

    /// `Dependent.from(base, ptr)` (roadmap Phase 4e, `phase-4e-memory`,
    /// design D1): surface-only stub. Emits `DependentFrom` with the base
    /// object and the already-computed field pointer.
    #[allow(dead_code)]
    fn lower_dependent_from(
        &mut self,
        base: Operand,
        field_ptr: Operand,
        element: IrType,
        span: Span,
    ) -> Operand {
        let id = self
            .module
            .dependent_types
            .iter()
            .position(|&t| t == element)
            .expect("the checker interned every Dependent<T> it type-checked")
            as u32;
        let ty = IrType::Dependent(id);
        self.emit(InstKind::DependentFrom { base, field_ptr }, ty, span)
    }

    /// `Pin(obj)` (roadmap Phase 4e, `phase-4e-memory`, design D1): pins the
    /// object at runtime and returns the same pointer as a `Pin<T>` value.
    fn lower_pin_call(&mut self, expr: &ast::CallExpr, span: Span) -> Operand {
        let value = &expr.args[0].value;
        let object = self.lower_expr(value);
        let referent = self.type_of(value, value.span());
        let id = self
            .module
            .pin_types
            .iter()
            .position(|&t| t == referent)
            .expect("the checker interned every Pin<T> it type-checked") as u32;
        let ty = IrType::Pin(id);
        self.emit(InstKind::PinObject { object }, ty, span)
    }

    /// Releases a pin created by `Self::lower_pin_call` (roadmap Phase 4e,
    /// `phase-4e-memory`, design D1): surface-only stub.
    #[allow(dead_code)]
    fn lower_unpin_object(&mut self, object: Operand, span: Span) -> Operand {
        self.emit(InstKind::UnpinObject { object }, IrType::Void, span)
    }

    /// Whether `e` is the compiler-derived `.clone()` (roadmap Phase 4e,
    /// `fase-4e-clone`, design D1/D4) — a bare, no-argument `.clone()` on an
    /// `Object`-typed receiver whose class declares no `clone` method of
    /// its own. `Checker::check_method_call_on` already reached exactly the
    /// same conclusion at the same gate (a manual implementation, when one
    /// exists, is dispatched through the ordinary method-call path,
    /// `Self::method_of`, instead) — mirrored here rather than threaded
    /// through from the checker, the same way `Self::is_weak_method_call`
    /// re-derives its own answer from the receiver's IR type rather than
    /// consulting a sema-provided call-resolution table.
    fn is_derived_clone_call(&self, e: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*e.callee else {
            return false;
        };
        if field.name.name != "clone" || !e.args.is_empty() {
            return false;
        }
        if self.checked.variant_accesses.contains(&field.span) {
            return false;
        }
        // A `record`/`enum` `.clone()` (roadmap Phase 7) has no `Object`
        // receiver to recognize it by — the checker's own recorded
        // expression type says which it is (a `record` resolves to
        // `IrType::Value`, an enum to `Int32` or `IrType::Enum`).
        match self
            .checked
            .expr_types
            .get(&field.object.span())
            .map(|t| t.base)
        {
            Some(Base::Enum(_) | Base::EnumInstance(_)) => return true,
            Some(Base::Class(id)) => {
                return self.checked.classes[id as usize].method("clone").is_none();
            }
            _ => {}
        }
        match self.type_of(&field.object, field.object.span()) {
            IrType::Object(id) => self.checked.classes[id as usize].method("clone").is_none(),
            IrType::Callable(_) => true,
            _ => false,
        }
    }

    /// The compiler-derived `.clone()` (design D1/D2): an object or callable
    /// lowers to a single `InstKind::Clone`, typed as the receiver's own
    /// type — see that variant's own doc comment for why the whole graph
    /// traversal lives in the runtime rather than being unrolled into
    /// several IR instructions here. A `record`/`enum` (roadmap Phase 7)
    /// clones field-wise instead: `Self::lower_clone_value` rebuilds the
    /// value with each `Clone` field's own copy.
    fn lower_derived_clone_call(&mut self, e: &ast::CallExpr, span: Span) -> Operand {
        let ast::Expr::Field(field) = &*e.callee else {
            unreachable!("checked by `Self::is_derived_clone_call`")
        };
        let ty = self.type_of(&field.object, field.object.span());
        let value = self.lower_expr(&field.object);
        self.lower_clone_value(value, ty, span)
    }

    /// The `Clone` copy of one value: scalars copy as they are; an object or
    /// callable goes through `zirk_rt_clone`'s graph traversal
    /// (`InstKind::Clone`); a `record` (`IrType::Value`) rebuilds itself
    /// field by field, recursively; an algebraic enum dispatches on its
    /// discriminant and rebuilds the active variant (roadmap Phase 7 — a
    /// whole-value copy would read an *inactive* variant's undefined
    /// payload, the hazard `Checker::is_clone_type`'s own comment names).
    fn lower_clone_value(&mut self, value: Operand, ty: IrType, span: Span) -> Operand {
        match ty {
            IrType::Nullable(inner) => {
                let inner_op = self.emit(InstKind::Unwrap(value), inner.inner(), span);
                let cloned = self.lower_clone_value(inner_op, inner.inner(), span);
                self.emit(
                    InstKind::Wrap {
                        base: inner,
                        value: cloned,
                    },
                    ty,
                    span,
                )
            }
            IrType::Object(_) | IrType::Callable(_) => self.emit(InstKind::Clone(value), ty, span),
            IrType::Value(id) => {
                let field_tys: Vec<IrType> = self.module.values[id as usize]
                    .fields
                    .iter()
                    .map(|f| f.ty)
                    .collect();
                let mut fields = Vec::with_capacity(field_tys.len());
                let slot = self.spill(value, ty, span);
                for (index, field_ty) in field_tys.iter().copied().enumerate() {
                    let whole = self.emit(InstKind::Load(slot), ty, span);
                    let field = self.emit(
                        InstKind::LoadField {
                            object: whole,
                            index: index as u32,
                        },
                        field_ty,
                        span,
                    );
                    fields.push(self.lower_clone_value(field, field_ty, span));
                }
                self.emit(InstKind::BuildValue { class: id, fields }, ty, span)
            }
            IrType::Enum(id) => self.lower_clone_enum(value, id, ty, span),
            _ => value,
        }
    }

    /// The `Clone` copy of an algebraic enum: dispatches on the
    /// discriminant so only the *active* variant's payload is read — an
    /// inactive variant's fields are undefined bits (`BuildEnum`'s own
    /// contract), which is why this cannot be a whole-value copy.
    fn lower_clone_enum(
        &mut self,
        value: Operand,
        enum_id: u32,
        ty: IrType,
        span: Span,
    ) -> Operand {
        let source = self.spill(value, ty, span);
        let result = self.declare_slot("<clone>", ty, span);
        let variants: Vec<(Vec<u32>, Vec<IrType>)> = self.module.enums[enum_id as usize]
            .variants
            .iter()
            .map(|indices| {
                let tys = indices
                    .iter()
                    .map(|&i| self.module.enums[enum_id as usize].fields[i as usize].ty)
                    .collect();
                (indices.clone(), tys)
            })
            .collect();
        let done = self.new_block();

        for (variant_index, (indices, field_tys)) in variants.iter().enumerate() {
            let whole = self.emit(InstKind::Load(source), ty, span);
            let disc = self.emit(
                InstKind::Discriminant(whole),
                IrType::Int(IntWidth::I32),
                span,
            );
            let expected = self.emit(
                InstKind::ConstInt(variant_index as i128),
                IrType::Int(IntWidth::I32),
                span,
            );
            let is_variant = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left: disc,
                    right: expected,
                },
                IrType::Boolean,
                span,
            );
            let body = self.new_block();
            let next = self.new_block();
            self.terminate(Terminator::Branch {
                condition: is_variant,
                then_block: body,
                else_block: next,
            });

            self.current = body;
            let mut fields = Vec::with_capacity(indices.len());
            for (i, &index) in indices.iter().enumerate() {
                let whole = self.emit(InstKind::Load(source), ty, span);
                let field = self.emit(
                    InstKind::LoadField {
                        object: whole,
                        index,
                    },
                    field_tys[i],
                    span,
                );
                fields.push(self.lower_clone_value(field, field_tys[i], span));
            }
            let rebuilt = self.emit(
                InstKind::BuildEnum {
                    enum_id,
                    variant: variant_index as u32,
                    fields,
                },
                ty,
                span,
            );
            self.emit_effect(InstKind::Store(result, rebuilt), span);
            self.terminate(Terminator::Jump(done));

            self.current = next;
        }

        // The verifier's own `match`-exhaustiveness guarantee means one arm
        // always matched; reaching `done` without a `result` would be an
        // uninitialized read, so fall through into `done` only after the
        // last variant was tested — its `next` is dead by construction, but
        // a jump keeps the terminator-correctness the IR requires.
        self.terminate(Terminator::Jump(done));
        self.current = done;
        self.emit(InstKind::Load(result), ty, span)
    }

    fn field_position(&self, object: &ast::Expr, name: &str) -> (u32, IrType) {
        fn layout_of<'a>(
            module: &'a Module,
            target: IrType,
            field: &str,
        ) -> Option<(&'a [ObjectField], usize)> {
            match target {
                IrType::Object(id) => {
                    let layout = &module.objects[id as usize];
                    let index = layout.field_index(field)?;
                    Some((&layout.fields, index))
                }
                IrType::Value(id) => {
                    let layout = &module.values[id as usize];
                    let index = layout.field_index(field)?;
                    Some((&layout.fields, index))
                }
                IrType::Pin(id) => layout_of(module, module.pin_types[id as usize], field),
                _ => None,
            }
        }

        let ty = self.type_of(object, object.span()).unwrapped();
        let (fields, index) = layout_of(self.module, ty, name)
            .expect("a verified field access reads an object, a value, or a Pin<T> of one");
        (index as u32, fields[index].ty)
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
        let one = if let IrType::Float(w) = ty {
            self.emit(InstKind::ConstFloat(w, "1".to_string()), ty, span)
        } else {
            self.const_int_at(1, ty, span)
        };
        let updated =
            self.emit_checked_binary(binary_op(expr.op.as_binary()), previous, one, ty, span);
        // `emit_checked_binary` may have split into a continuation block. The
        // slot has not been updated yet, so re-reading it here gives the same
        // pre-increment value `previous` had in the original block.
        let previous = self.emit(InstKind::Load(slot), ty, span);
        self.journal_writes_to_slot(slot, span);
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
            ast::Stmt::Unsafe(nested) => self.lower_unsafe_block_value(&nested.body),
            ast::Stmt::Commit(nested) => self.lower_commit_block_value(&nested.body),
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
            // `array.add(...)`, `list.remove(...)` and other built-in collection
            // methods are lowered to collection instructions/runtime calls.
            ast::Expr::Call(e) if self.is_array_list_method_call(e) => {
                if let Some(result) = self.lower_array_list_method_call(e, expr.span()) {
                    let _ = result;
                }
            }
            // `map.set(...)`, `set.add(...)`, etc.
            ast::Expr::Call(e) if self.is_map_set_method_call(e) => {
                if let Some(result) = self.lower_map_set_method_call(e, expr.span()) {
                    let _ = result;
                }
            }
            // A closure call goes through the value and has its own arm in
            // `lower_expr`; only a direct call is special-cased here.
            ast::Expr::Call(e)
                if !self.is_callable_call(e) && {
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
    /// class/record with its own `to_string()` method is called
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

        if matches!(ty, IrType::Array(_) | IrType::List(_)) {
            return self.lower_collection_to_string(operand, ty, span);
        }

        if let IrType::Enum(enum_id) = ty {
            return self.lower_enum_to_string(operand, enum_id, span);
        }

        if let IrType::Value(id) = ty {
            let has_method = (id as usize) < self.checked.classes.len()
                && self.checked.classes[id as usize]
                    .method("to_string")
                    .is_some();
            if has_method {
                let method = self.checked.classes[id as usize]
                    .method("to_string")
                    .unwrap();
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

            // Default rendering for tuples and records that do
            // not declare their own `to_string`.
            let layout_name = self.module.values[id as usize].name.clone();
            let is_tuple = layout_name.starts_with("Tuple(");
            let prefix = if is_tuple {
                "("
            } else {
                &format!("{layout_name}(")
            };
            let fields: Vec<_> = self.module.values[id as usize]
                .fields
                .iter()
                .map(|f| (f.name.clone(), f.ty))
                .collect();
            return self.lower_default_render_to_string(operand, prefix, &fields, is_tuple, span);
        }

        if let IrType::Object(id) = ty {
            if let Some(method) = self.checked.classes[id as usize].method("to_string") {
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

            let layout_name = self.module.objects[id as usize].name.clone();
            let fields: Vec<_> = self.module.objects[id as usize]
                .fields
                .iter()
                .map(|f| (f.name.clone(), f.ty))
                .collect();
            return self.lower_default_render_to_string(
                operand,
                &format!("{layout_name}("),
                &fields,
                false,
                span,
            );
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

        if matches!(ty, IrType::Weak(_)) {
            return self.const_string("<Weak>", span);
        }

        if matches!(ty, IrType::Callable(_)) {
            return self.const_string("<Callable>", span);
        }

        self.emit(InstKind::ToString(operand), IrType::String, span)
    }

    /// Renders a tuple, record, or class as `Name(f0: v0, f1: v1)` or, for
    /// tuples, `(v0, v1)`. Each field is read with `LoadField` and converted
    /// recursively.
    fn lower_default_render_to_string(
        &mut self,
        operand: Operand,
        prefix: &str,
        fields: &[(String, IrType)],
        is_tuple: bool,
        span: Span,
    ) -> Operand {
        let mut result = self.const_string(prefix, span);
        for (i, (field_name, field_ty)) in fields.iter().enumerate() {
            if i > 0 {
                let sep = self.const_string(", ", span);
                result = self.emit(
                    InstKind::Concat {
                        left: result,
                        right: sep,
                    },
                    IrType::String,
                    span,
                );
            }
            if !is_tuple {
                let label = self.const_string(&format!("{field_name}: "), span);
                result = self.emit(
                    InstKind::Concat {
                        left: result,
                        right: label,
                    },
                    IrType::String,
                    span,
                );
            }
            let field = self.emit(
                InstKind::LoadField {
                    object: operand,
                    index: i as u32,
                },
                *field_ty,
                span,
            );
            let rendered = self.lower_to_string(field, *field_ty, span);
            result = self.emit(
                InstKind::Concat {
                    left: result,
                    right: rendered,
                },
                IrType::String,
                span,
            );
        }
        let close = self.const_string(")", span);
        self.emit(
            InstKind::Concat {
                left: result,
                right: close,
            },
            IrType::String,
            span,
        )
    }

    /// Default `to_string` for an algebraic enum: switch on the discriminant
    /// and return the matching variant's name.
    fn lower_enum_to_string(&mut self, operand: Operand, enum_id: u32, span: Span) -> Operand {
        let result_slot = self.declare_slot("<enum_to_string>", IrType::String, span);

        let disc_ty = IrType::Int(IntWidth::I32);
        let disc = if enum_has_payload(self.checked, enum_id) {
            self.emit(InstKind::Discriminant(operand), disc_ty, span)
        } else {
            operand
        };
        let disc_slot = self.declare_slot("<enum_disc>", disc_ty, span);
        self.emit_effect(InstKind::Store(disc_slot, disc), span);

        let continue_block = self.new_block();
        let variants = self.checked.enums[enum_id as usize].variants.clone();
        let mut current = self.current;

        for (i, variant) in variants.iter().enumerate() {
            let arm = self.new_block();
            self.current = current;
            let disc = self.emit(InstKind::Load(disc_slot), disc_ty, span);
            let index = self.emit(InstKind::ConstInt(i as i128), disc_ty, span);
            let test = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Eq,
                    left: disc,
                    right: index,
                },
                IrType::Boolean,
                span,
            );
            let next = self.new_block();
            self.terminate(Terminator::Branch {
                condition: test,
                then_block: arm,
                else_block: next,
            });
            self.current = arm;
            let name = self.const_string(&variant.name, span);
            self.emit_effect(InstKind::Store(result_slot, name), span);
            self.terminate(Terminator::Jump(continue_block));
            current = next;
        }

        self.current = current;
        let fallback = self.const_string("", span);
        self.emit_effect(InstKind::Store(result_slot, fallback), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result_slot), IrType::String, span)
    }

    /// `Array<T>` / `List<T>` `to_string()`: builds `[e0, e1, ...]` by
    /// looping over the collection, converting each element with
    /// [`Self::lower_to_string`], and concatenating with `", ".
    fn lower_collection_to_string(
        &mut self,
        receiver: Operand,
        receiver_ty: IrType,
        span: Span,
    ) -> Operand {
        let u64_ty = IrType::Int(IntWidth::U64);
        let zero = self.const_int_at(0, u64_ty, span);

        // The collection itself, its length and the accumulating result string
        // live in slots so the loop blocks can reload them.
        let receiver_slot = self.spill(receiver, receiver_ty, span);
        let length = if matches!(receiver_ty, IrType::List(_)) {
            self.emit(InstKind::ListLength(receiver), u64_ty, span)
        } else {
            self.emit(InstKind::ArrayLength(receiver), u64_ty, span)
        };
        let length_slot = self.spill(length, u64_ty, span);

        let open = self.const_string("[", span);
        let result_slot = self.spill(open, IrType::String, span);
        let i_slot = self.spill(zero, u64_ty, span);

        let header = self.new_block();
        let body = self.new_block();
        let comma = self.new_block();
        let after_comma = self.new_block();
        let end = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let i = self.emit(InstKind::Load(i_slot), u64_ty, span);
        let length = self.emit(InstKind::Load(length_slot), u64_ty, span);
        let cond = self.emit(
            InstKind::Binary {
                op: BinaryOp::Lt,
                left: i,
                right: length,
            },
            IrType::Boolean,
            span,
        );
        self.terminate(Terminator::Branch {
            condition: cond,
            then_block: body,
            else_block: end,
        });

        self.current = body;
        let i = self.emit(InstKind::Load(i_slot), u64_ty, span);
        let zero = self.const_int_at(0, u64_ty, span);
        let is_first = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: i,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        self.terminate(Terminator::Branch {
            condition: is_first,
            then_block: after_comma,
            else_block: comma,
        });

        self.current = comma;
        let result = self.emit(InstKind::Load(result_slot), IrType::String, span);
        let sep = self.const_string(", ", span);
        let result = self.emit(
            InstKind::Concat {
                left: result,
                right: sep,
            },
            IrType::String,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, result), span);
        self.terminate(Terminator::Jump(after_comma));

        self.current = after_comma;
        let receiver = self.emit(InstKind::Load(receiver_slot), receiver_ty, span);
        let i = self.emit(InstKind::Load(i_slot), u64_ty, span);
        let element_ty = self.array_list_element_type(receiver_ty);
        let element = self.emit(
            InstKind::ArrayListLoad { receiver, index: i },
            element_ty,
            span,
        );
        let element_str = self.lower_to_string(element, element_ty, span);
        let result = self.emit(InstKind::Load(result_slot), IrType::String, span);
        let result = self.emit(
            InstKind::Concat {
                left: result,
                right: element_str,
            },
            IrType::String,
            span,
        );
        self.emit_effect(InstKind::Store(result_slot, result), span);
        let i = self.emit(InstKind::Load(i_slot), u64_ty, span);
        let one = self.const_int_at(1, u64_ty, span);
        let i = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: i,
                right: one,
            },
            u64_ty,
            span,
        );
        self.emit_effect(InstKind::Store(i_slot, i), span);
        self.terminate(Terminator::Jump(header));

        self.current = end;
        let result = self.emit(InstKind::Load(result_slot), IrType::String, span);
        let close = self.const_string("]", span);
        self.emit(
            InstKind::Concat {
                left: result,
                right: close,
            },
            IrType::String,
            span,
        )
    }

    /// [`Self::lower_to_string`]'s expression-aware form: `Duration` is an
    /// `Int64` at the IR level, which `ToString` would render as a raw
    /// nanosecond count, so it goes through the runtime's own duration
    /// formatter instead. A bare enum variant access (`Color.Red`) is known
    /// at compile time, so its name is returned directly.
    fn lower_to_string_expr(&mut self, expr: &ast::Expr, operand: Operand, span: Span) -> Operand {
        // `date-and-time-types`: the civil temporal types render through
        // their ISO 8601 formatters in the runtime.
        if let Some(callee) = match self.temporal_base(expr) {
            Some(zirk_sema::Base::Date) => Some("zirk_rt_date_to_string"),
            Some(zirk_sema::Base::Time) => Some("zirk_rt_time_to_string"),
            Some(zirk_sema::Base::DateTime) => Some("zirk_rt_datetime_to_string"),
            _ => None,
        } {
            return self.emit(
                InstKind::Call {
                    callee: callee.to_string(),
                    args: vec![operand],
                },
                IrType::String,
                span,
            );
        }

        if self.is_duration(expr) {
            return self.emit(
                InstKind::Call {
                    callee: "zirk_rt_duration_to_string".to_string(),
                    args: vec![operand],
                },
                IrType::String,
                span,
            );
        }

        if let ast::Expr::Field(field) = expr
            && self.checked.variant_accesses.contains(&field.span)
            && let Some(&ty) = self.checked.expr_types.get(&expr.span())
            && matches!(ty.base, Base::Enum(_) | Base::EnumInstance(_))
        {
            return self.const_string(&field.name.name, span);
        }

        if let Some(&ty) = self.checked.expr_types.get(&expr.span())
            && let Base::Enum(enum_id) = ty.base
        {
            return self.lower_enum_to_string(operand, enum_id, span);
        }

        let ty = self.type_of(expr, expr.span());
        self.lower_to_string(operand, ty, span)
    }

    /// Lowers the argument of a `println`, converting it via `to_string()`
    /// when it is not already a `String`.
    ///
    /// `ZIRK_STDLIB_SPEC.md` section 3: every printable value goes through
    /// `to_string()`. Without this the runtime would read an `Int32` as if it
    /// were a pointer.
    fn lower_println_argument(&mut self, expr: &ast::PrintlnExpr, span: Span) -> Operand {
        let operand = self.lower_expr(&expr.arg);
        self.lower_to_string_expr(&expr.arg, operand, span)
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
                    self.lower_to_string_expr(inner, operand, span)
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
        if field.name.name != "to_string"
            || !call.args.is_empty()
            || self.checked.variant_accesses.contains(&field.span)
        {
            return false;
        }
        match self.type_of(&field.object, field.object.span()) {
            IrType::Object(id) => self.checked.classes[id as usize]
                .method("to_string")
                .is_none(),
            IrType::Value(id) => {
                (id as usize) >= self.checked.classes.len()
                    || self.checked.classes[id as usize]
                        .method("to_string")
                        .is_none()
            }
            IrType::Contract(_) => false,
            _ => true,
        }
    }

    /// Whether `call` is `regex.matches(text)` or `regex.replace(text, repl)`.
    fn is_regex_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("matches", 1) | ("replace", 2) | ("find", 1) | ("split", 1) | ("find_all", 1)
        ) {
            return false;
        }
        self.type_of(&field.object, field.object.span()) == IrType::Regex
    }

    /// Whether `call` is `Regex.Match.group(n)` / `Regex.Match.group(name)`.
    fn is_regex_match_group_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if field.name.name != "group" || call.args.len() != 1 {
            return false;
        }
        let ty = self.type_of(&field.object, field.object.span()).unwrapped();
        matches!(ty, IrType::Object(id) if id == self.checked.regex_match_class)
    }

    /// Whether `call` is a built-in `String` method.
    fn is_string_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("trim", 0)
                | ("trim_start", 0)
                | ("trim_end", 0)
                | ("to_lowercase", 0)
                | ("to_uppercase", 0)
                | ("clone", 0)
                | ("is_empty", 0)
                | ("split_whitespace", 0)
                | ("lines", 0)
                | ("bytes", 0)
                | ("codepoints", 0)
                | ("chars", 0)
                | ("contains", 1)
                | ("starts_with", 1)
                | ("ends_with", 1)
                | ("substring", 2)
                | ("search", 1)
                | ("find", 1)
                | ("replace", 2)
                | ("normalize", 1)
                | ("split", 1)
        ) {
            return false;
        }
        self.type_of(&field.object, field.object.span()) == IrType::String
    }

    /// Whether `call` is a built-in `Char` method.
    fn is_char_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if !matches!(
            (field.name.name.as_str(), call.args.len()),
            ("is_uppercase", 0)
                | ("is_lowercase", 0)
                | ("is_digit", 0)
                | ("is_letter", 0)
                | ("is_whitespace", 0)
                | ("is_ascii", 0)
                | ("is_alphabetic", 0)
                | ("is_numeric", 0)
                | ("is_alphanumeric", 0)
                | ("ascii_code", 0)
                | ("bytes", 0)
                | ("codepoints", 0)
                | ("to_uppercase", 0)
                | ("to_lowercase", 0)
                | ("normalize", 1)
        ) {
            return false;
        }
        self.type_of(&field.object, field.object.span()) == IrType::Char
    }

    /// Whether `call` is a built-in `Array<T>`/`List<T>` method call.
    fn is_array_list_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if matches!(field.name.name.as_str(), "length" | "is_empty") {
            return false;
        }
        if let ast::Expr::Path(ident) = &*field.object
            && self.try_lookup_slot(&ident.name).is_none()
        {
            return false;
        }
        matches!(
            self.type_of(&field.object, field.object.span()),
            IrType::Array(_) | IrType::List(_)
        )
    }

    /// Whether `call` is a built-in `Map<K, V>`/`Set<T>` method call.
    fn is_map_set_method_call(&self, call: &ast::CallExpr) -> bool {
        let ast::Expr::Field(field) = &*call.callee else {
            return false;
        };
        if let ast::Expr::Path(ident) = &*field.object
            && self.try_lookup_slot(&ident.name).is_none()
        {
            return false;
        }
        matches!(
            self.type_of(&field.object, field.object.span()),
            IrType::Map(_) | IrType::Set(_)
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

    fn is_callable_call(&self, call: &ast::CallExpr) -> bool {
        // `Outer.Nested(...)`/`o.Inner(...)`: the callee's base names a type
        // (or is the hidden `outer` receiver), so every probe below would
        // ask for the type of something that is not one — the same failure
        // mode the `is_enum_static_call`/`is_class_static_call` guards name.
        if self.checked.resolved_constructions.contains_key(&call.span) {
            return false;
        }

        // `enum-static-members`: `Direction.keys()`/`Enums.values(E)` — the
        // callee's base names a type, not a value, so every probe below
        // (`exception_intrinsic_type`, `method_of`, the `type_of`
        // fallthrough on the callee itself) would ask for the type of
        // something that is not one — exactly the failure mode the
        // `Pointer.from` guard below names.
        if self.is_enum_static_call(call) {
            return false;
        }
        // `ClassName.method(...)` `static fn`: the base names a type, not a
        // value, and the static-call path lowers it.
        if self.is_class_static_call(call) {
            return false;
        }
        // A method or `super` call is not a callable call, and asking for the
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
        // Neither is `Weak.from(value)`/a `Weak<T>` method (roadmap Phase
        // 4e, `fase-4e-weak`) — same reasoning as `Pointer.from` above.
        if self.is_weak_from_call(call) || self.is_weak_method_call(call) {
            return false;
        }
        // Neither is the compiler-derived `.clone()` (roadmap Phase 4e,
        // `fase-4e-clone`) — same reasoning again: its own receiver is an
        // ordinary `Object`, but `field.name.name` names no method, so
        // `Self::method_of` below would find nothing and this call would
        // otherwise be mistaken for a closure-typed value.
        if self.is_derived_clone_call(call) {
            return false;
        }

        // `date-and-time-types`: a `Date(...)`/`Time(...)`/`DateTime(...)`
        // construction or a `Date.today()`/`Time.now_local()` host-clock
        // static is not a callable call either — its callee names a type,
        // and the probes below (`exception_intrinsic_type`, `method_of`,
        // the `type_of` fallthrough) would ask for the type of something
        // that is not a value, exactly the failure mode `Pointer.from`
        // names above. The temporal statics live in
        // `scalar_static_accesses`, which `is_native_static_call` below
        // already covers; this clause is for the `Path` callee of the
        // constructions.
        if self.checked.temporal_constructions.contains(&call.span) {
            return false;
        }
        // `is_native_static_call` only consults `scalar_static_accesses` —
        // no `type_of` probe — so it is safe to run before
        // `exception_intrinsic_type` for `Date.today()`/`Time.now_local()`,
        // whose `Field` callee would otherwise make `type_of` try to
        // resolve `Date` as a variable.
        if self.is_native_static_call(call) {
            return false;
        }

        // `Throwable.stack_trace()` and `Throwable.suppressed()` are runtime
        // intrinsics, not callable values or ordinary method symbols.
        if self.exception_intrinsic_type(call).is_some() {
            return false;
        }

        // `native-type-member-surface`: static calls on type names
        // (`Int32.parse(...)`) and built-in method calls whose receiver is a
        // type name or a native scalar (`Int32.MAX.to_string()`,
        // `s.find("x")`) must be recognized before `method_of`/`contract`
        // resolution tries to look the base up as a variable/function.
        if self.is_native_static_call(call)
            || self.is_native_to_string_call(call)
            || self.is_scalar_method_call(call)
            || self.is_string_method_call(call)
            || self.is_char_method_call(call)
            || self.is_duration_method_call(call)
            || self.is_regex_method_call(call)
            || self.is_regex_match_group_call(call)
            || self.is_array_list_method_call(call)
            || self.is_map_set_method_call(call)
            || self.result_method(call).is_some()
        {
            return false;
        }

        if self.method_of(call).is_some()
            || self.contract_method_of(call).is_some()
            || self.safe_method_call_info(call).is_some()
            || self.variant_construction(call).is_some()
            || self.is_native_to_string_call(call)
            || self.is_regex_method_call(call)
            || self.is_regex_match_group_call(call)
            || self.is_duration_method_call(call)
            || self.is_string_method_call(call)
            || self.is_char_method_call(call)
            || self.is_scalar_method_call(call)
            || self.is_native_static_call(call)
            || self.is_array_list_method_call(call)
            || self.is_map_set_method_call(call)
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
                .is_some_and(|slot| matches!(self.slot_type(*slot), IrType::Callable(_))),
            other => matches!(self.type_of(other, other.span()), IrType::Callable(_)),
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
        // The checker records the contextual width for numeric literals and the
        // result type for binary operations; use it when available so lowering
        // matches the semantic type. Other expressions keep their IR-derived type
        // (slot, layout, etc.).
        if matches!(
            expr,
            ast::Expr::Int(_) | ast::Expr::Float(_) | ast::Expr::Duration(_) | ast::Expr::Binary(_)
        ) && let Some(&ty) = self.checked.expr_types.get(&expr.span())
            && !ty.is_unknown()
        {
            return self.ir_type(ty);
        }

        match expr {
            ast::Expr::Int(_) => IrType::Int(IntWidth::I32),
            ast::Expr::Duration(_) => IrType::Int(IntWidth::I64),
            ast::Expr::Float(lit) => IrType::Float(float_literal_width(lit)),
            ast::Expr::Bool(_) => IrType::Boolean,
            ast::Expr::Str(_) => IrType::String,
            ast::Expr::Regex(_) => IrType::Regex,
            ast::Expr::Char(_) => IrType::Char,
            ast::Expr::Path(ident) => match self.try_lookup_slot(&ident.name) {
                Some(slot) => self.slot_type(slot),
                None => {
                    // `outer` inside an `inner class` names the hidden field.
                    if ident.name == "outer"
                        && let Some(class) = self.current_class
                        && let Some(parent) = self.checked.classes[class as usize].enclosing
                    {
                        return IrType::Object(parent);
                    }
                    let (id, _) = self
                        .named_function_value(&ident.name, ident.span)
                        .unwrap_or_else(|| {
                            panic!(
                                "a verified program only names declared variables or functions (`{}`)",
                                ident.name
                            )
                        });
                    IrType::Callable(id)
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
                if matches!(
                    e.op,
                    ast::BinaryOp::Eq
                        | ast::BinaryOp::NotEq
                        | ast::BinaryOp::Lt
                        | ast::BinaryOp::LtEq
                        | ast::BinaryOp::Gt
                        | ast::BinaryOp::GtEq
                ) && self.operator_method_of(e).is_some() =>
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
            ast::Expr::Call(e) if self.is_callable_call(e) => {
                let IrType::Callable(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_callable_call`")
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
                    "as_slice" => self.native_slice_result_type(id, false),
                    "as_slice_mut" => self.native_slice_result_type(id, true),
                    _ => unreachable!("checked by `Self::is_pointer_method_call`"),
                }
            }
            ast::Expr::Call(e) if self.is_weak_from_call(e) => {
                let value = &e.args[0].value;
                let referent = self.type_of(value, value.span());
                let id = self
                    .module
                    .weak_types
                    .iter()
                    .position(|&t| t == referent)
                    .expect("the checker interned every Weak<T> it type-checked")
                    as u32;
                IrType::Weak(id)
            }
            ast::Expr::Call(e) if self.is_weak_method_call(e) => {
                let ast::Expr::Field(field) = &*e.callee else {
                    unreachable!("checked by `Self::is_weak_method_call`")
                };
                let IrType::Weak(id) = self.type_of(&field.object, field.object.span()) else {
                    unreachable!("checked by `Self::is_weak_method_call`")
                };
                let referent = self.module.weak_types[id as usize];
                IrType::Nullable(
                    Nullable::of(referent).expect("a Weak<T> referent has a nullable form"),
                )
            }
            ast::Expr::Call(e) if self.is_pin_call(e) => {
                let value = &e.args[0].value;
                let referent = self.type_of(value, value.span());
                let id = self
                    .module
                    .pin_types
                    .iter()
                    .position(|&t| t == referent)
                    .expect("the checker interned every Pin<T> it type-checked")
                    as u32;
                IrType::Pin(id)
            }
            ast::Expr::Call(e) if self.is_derived_clone_call(e) => {
                let ast::Expr::Field(field) = &*e.callee else {
                    unreachable!("checked by `Self::is_derived_clone_call`")
                };
                self.type_of(&field.object, field.object.span())
            }
            ast::Expr::Call(e) => {
                // `ClassName.method(...)` `static fn`: the base names a type,
                // not a value; `method_of`/`contract_method_of` would panic.
                if let Some(method) = self.class_static_method_of(e) {
                    return self.ir_type(method.returns);
                }

                // `native-type-member-surface`: the checker already resolved
                // the concrete type of built-in method/static calls and of
                // every other well-typed call — use it before any name-based
                // fallback that cannot handle a `Field` callee.
                if let Some(&ty) = self.checked.expr_types.get(&expr.span())
                    && !ty.is_unknown()
                {
                    return self.ir_type(ty);
                }
                if let Some(ty) = self.exception_intrinsic_type(e) {
                    return ty;
                }
                if let Some(method) = self.contract_method_of(e) {
                    // A generic contract member may name the contract's own
                    // type parameter (`Iterator<T>.next(): Iteration<T>`):
                    // `method.returns` is the unsubstituted signature, whose
                    // layout is the placeholder `module.enums` keeps for a
                    // still-generic instance. The checker recorded this
                    // call's substituted type (`Iteration<Int32>`) — use it
                    // when it exists (roadmap Phase 7, task 12.3).
                    if let Some(&ty) = self.checked.expr_types.get(&expr.span())
                        && !ty.is_unknown()
                    {
                        return self.ir_type(ty);
                    }
                    return self.ir_type(method.returns);
                }
                if let Some(method) = self.method_of(e) {
                    if let Some(&ty) = self.checked.expr_types.get(&expr.span())
                        && !ty.is_unknown()
                    {
                        return self.ir_type(ty);
                    }
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
                if self.is_regex_method_call(e)
                    && let ast::Expr::Field(field) = &*e.callee
                {
                    return match field.name.name.as_str() {
                        "matches" => IrType::Boolean,
                        "replace" => IrType::String,
                        "find" => {
                            IrType::Nullable(Nullable::Object(self.checked.regex_match_class))
                        }
                        "split" => {
                            let id = self
                                .checked
                                .list_types
                                .iter()
                                .position(|&t| t == Type::STRING)
                                .expect("the checker interned `List<String>` for `Regex.split`")
                                as u32;
                            IrType::List(id)
                        }
                        "find_all" => IrType::List(self.regex_match_list_id()),
                        _ => unreachable!("checked by `is_regex_method_call`"),
                    };
                }
                if self.is_regex_match_group_call(e) {
                    return IrType::String;
                }
                if self.is_range_method_call(e) {
                    return IrType::Range;
                }
                if self.is_string_method_call(e)
                    && let ast::Expr::Field(field) = &*e.callee
                {
                    return match field.name.name.as_str() {
                        "trim" | "substring" => IrType::String,
                        "contains" | "starts_with" | "ends_with" => IrType::Boolean,
                        "search" => IrType::Int(IntWidth::I64),
                        _ => unreachable!("checked by `is_string_method_call`"),
                    };
                }
                if self.is_char_method_call(e)
                    && let ast::Expr::Field(field) = &*e.callee
                {
                    return match field.name.name.as_str() {
                        "is_uppercase" | "is_lowercase" | "is_digit" | "is_letter"
                        | "is_whitespace" => IrType::Boolean,
                        "to_uppercase" | "to_lowercase" => IrType::String,
                        _ => unreachable!("checked by `is_char_method_call`"),
                    };
                }
                if self.is_duration_method_call(e)
                    && let ast::Expr::Field(field) = &*e.callee
                {
                    return match field.name.name.as_str() {
                        "abs" => IrType::Int(IntWidth::I64),
                        "sign" => IrType::Int(IntWidth::I32),
                        _ => IrType::Boolean,
                    };
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
                if let Some(&ty) = self.checked.expr_types.get(&expr.span())
                    && matches!(ty.base, Base::Array(_) | Base::List(_))
                {
                    return self.ir_type(ty);
                }
                // A call through a contract instantiation (`it.next()` on an
                // `Iterator<Int32>`-typed value): the method's declared
                // signature may name the contract's own `T`, so the
                // checker's substituted record of this call's type is the
                // answer — `ir_type` of `method.returns` would point at the
                // unsubstituted `Iteration<T>` layout (roadmap Phase 7).
                if let ast::Expr::Field(field) = &*e.callee
                    && matches!(
                        self.type_of(&field.object, field.object.span()),
                        IrType::Contract(_)
                    )
                    && let Some(&ty) = self.checked.expr_types.get(&expr.span())
                    && !ty.is_unknown()
                {
                    return self.ir_type(ty);
                }
                // `native-type-member-surface`: the checker already knows
                // the final type for built-in method/static calls; use it
                // before the name-based resolution below, which cannot
                // handle a `Field` callee (`Int32.MAX.to_string()`,
                // `s.find("x")`, etc.).
                if let Some(&ty) = self.checked.expr_types.get(&expr.span())
                    && !ty.is_unknown()
                    && (self.is_native_static_call(e)
                        || self.is_scalar_method_call(e)
                        || self.is_string_method_call(e)
                        || self.is_char_method_call(e)
                        || self.is_native_to_string_call(e)
                        || self.is_array_list_method_call(e)
                        || self.is_map_set_method_call(e))
                {
                    return self.ir_type(ty);
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
            // `receiver[index]` (roadmap Phase 4e, `fase-4e-native-slice`,
            // design D5): the element type its indexing entry carries.
            ast::Expr::Index(e) => {
                let receiver_ty = self.type_of(&e.receiver, e.receiver.span());
                if receiver_ty == IrType::String {
                    IrType::Char
                } else if let IrType::Value(layout_id) = receiver_ty {
                    let ast::Expr::Int(lit) = &*e.index else {
                        unreachable!("a tuple index is a constant integer literal")
                    };
                    let index = lit.value as usize;
                    self.module.values[layout_id as usize]
                        .fields
                        .get(index)
                        .map(|f| f.ty)
                        .unwrap_or(IrType::Never)
                } else if matches!(receiver_ty, IrType::Array(_) | IrType::List(_)) {
                    self.array_list_element_type(receiver_ty)
                } else {
                    self.native_slice_element_type(receiver_ty)
                }
            }
            // `receiver[start:end:step]` (roadmap Phase 7) — the checker only
            // accepts a `String` receiver today.
            // `r[...]` slices to a `Range` of the same element type
            // (roadmap Phase 7); a `String` slice stays `String`.
            ast::Expr::Slice(e) => self.type_of(&e.receiver, e.receiver.span()),
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
            ast::Expr::Tuple(_) => {
                let Some(&ty) = self.checked.expr_types.get(&expr.span()) else {
                    unreachable!("a tuple expression has a recorded semantic type")
                };
                self.ir_type(ty)
            }
            ast::Expr::Range(_) => IrType::Range,
            ast::Expr::Lambda(_) | ast::Expr::Null(_) => {
                unreachable!("the type of this expression comes from the value it produced")
            }
            ast::Expr::Transfer(e) => self.type_of(&e.expr, e.expr.span()),
            ast::Expr::Cast(e) => {
                let base = self.ir_type_from_ref(&e.target);
                if e.optional {
                    IrType::Nullable(Nullable::of(base).expect("only nullable targets reach `as?`"))
                } else {
                    base
                }
            }
            ast::Expr::Interpolated(_) => IrType::String,
            ast::Expr::Unsafe(u) => self.block_value_type(&u.body),
            ast::Expr::Commit(c) => self.block_value_type(&c.body),
        }
    }

    /// The semantic type the checker recorded for an expression, if any.
    fn semantic_type(&self, expr: &ast::Expr) -> Option<zirk_sema::Type> {
        self.checked.expr_types.get(&expr.span()).copied()
    }

    fn is_duration(&self, expr: &ast::Expr) -> bool {
        self.semantic_type(expr)
            .is_some_and(|ty| matches!(ty.base, zirk_sema::Base::Duration))
    }

    /// Whether an expression's semantic type is one of the civil temporal
    /// types (`date-and-time-types`) — the values `lower_temporal_binary`
    /// and `lower_to_string_expr` route on.
    fn is_temporal(&self, expr: &ast::Expr) -> bool {
        self.semantic_type(expr).is_some_and(|ty| {
            matches!(
                ty.base,
                zirk_sema::Base::Date | zirk_sema::Base::Time | zirk_sema::Base::DateTime
            )
        })
    }

    /// The semantic `Base` of a civil temporal operand (`Date`, `Time` or
    /// `DateTime`), or `None` when it is not one of them.
    fn temporal_base(&self, expr: &ast::Expr) -> Option<zirk_sema::Base> {
        self.semantic_type(expr).and_then(|ty| match ty.base {
            b @ (zirk_sema::Base::Date | zirk_sema::Base::Time | zirk_sema::Base::DateTime) => {
                Some(b)
            }
            _ => None,
        })
    }

    /// Lowers civil temporal arithmetic and comparison
    /// (`date-and-time-types`). A `Date` is an `i64` day count and a `Time`
    /// an `i64` nanosecond count, so same-type comparisons and
    /// `Date`(+|-)`Int` / `Date - Date` are exactly the checked integer
    /// operations. What an integer instruction cannot express goes through
    /// the runtime: `Time`'s wrap-around-midnight `Duration` arithmetic,
    /// `Time - Time`, and `Date + Time` composing into an `i128` `DateTime`.
    fn lower_temporal_binary(
        &mut self,
        expr: &ast::BinaryExpr,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Operand {
        use ast::BinaryOp::*;

        let left_base = self.temporal_base(&expr.left);
        let right_base = self.temporal_base(&expr.right);

        // `Date + Time` / `Time + Date` composes a `DateTime`: the runtime
        // packs the day count and the day's nanoseconds into the `i128`.
        if expr.op == Add
            && matches!(
                (left_base, right_base),
                (Some(zirk_sema::Base::Date), Some(zirk_sema::Base::Time))
                    | (Some(zirk_sema::Base::Time), Some(zirk_sema::Base::Date))
            )
        {
            let (days, nanos) = if left_base == Some(zirk_sema::Base::Date) {
                (left, right)
            } else {
                (right, left)
            };
            return self.emit(
                InstKind::Call {
                    callee: "zirk_rt_datetime_new".to_string(),
                    args: vec![days, nanos],
                },
                IrType::Int(IntWidth::I128),
                span,
            );
        }

        // `Date`/`DateTime` (+|-) `Duration` (`temporal-rich-api`): a
        // duration is nanoseconds, so both operands move onto the `i128`
        // epoch-nanosecond timeline — a `Date` is read at local midnight
        // (`days * NANOS_PER_DAY`), a `DateTime` is already there. The
        // result is always a `DateTime`; `Duration` may only appear on the
        // left of `+` (the checker never admits `Duration - Date`).
        let temporal_base = left_base.or(right_base);
        let mixes_duration = matches!(
            temporal_base,
            Some(zirk_sema::Base::Date | zirk_sema::Base::DateTime)
        ) && (self.is_duration(&expr.left) || self.is_duration(&expr.right));
        if mixes_duration && matches!(expr.op, Add | Sub) {
            let i128ty = IrType::Int(IntWidth::I128);
            let i64ty = IrType::Int(IntWidth::I64);
            let (value, base, delta, negate) = if let Some(left_base) = left_base {
                (left, left_base, right, expr.op == Sub)
            } else {
                (right, right_base.unwrap(), left, false)
            };
            // Plain `Binary` ops, not `emit_checked_binary`: every operand
            // range keeps the result inside `i128` by a wide margin
            // (`i64` days × `8.64e16` ≈ 2^120, plus an `i64` delta), and a
            // checked helper would open a block these operands would have
            // to cross.
            let epoch = if base == zirk_sema::Base::Date {
                let value = self.convert_numeric(value, i64ty, i128ty, span);
                let per_day = self.const_int_at(86_400_000_000_000, i128ty, span);
                self.emit(
                    InstKind::Binary {
                        op: BinaryOp::Mul,
                        left: value,
                        right: per_day,
                    },
                    i128ty,
                    span,
                )
            } else {
                value
            };
            let delta = self.convert_numeric(delta, i64ty, i128ty, span);
            let delta = if negate {
                let zero = self.const_int_at(0, i128ty, span);
                self.emit(
                    InstKind::Binary {
                        op: BinaryOp::Sub,
                        left: zero,
                        right: delta,
                    },
                    i128ty,
                    span,
                )
            } else {
                delta
            };
            return self.emit(
                InstKind::Binary {
                    op: BinaryOp::Add,
                    left: epoch,
                    right: delta,
                },
                i128ty,
                span,
            );
        }

        // `DateTime - DateTime` → `Duration` (`temporal-rich-api`): the
        // exact epoch-nanosecond difference. It is computed at `i128` and
        // narrowed to the `i64` `Duration` — a span past ±292 years cannot
        // be a `Duration` and throws `ArithmeticOverflowError`.
        if expr.op == Sub
            && left_base == Some(zirk_sema::Base::DateTime)
            && right_base == Some(zirk_sema::Base::DateTime)
        {
            let i128ty = IrType::Int(IntWidth::I128);
            let i64ty = IrType::Int(IntWidth::I64);
            let diff = self.emit_checked_binary(binary_op(Sub), left, right, i128ty, span);
            let diff_slot = self.spill(diff, i128ty, span);
            let lo = self.const_int_at(i64::MIN as i128, i128ty, span);
            let hi = self.const_int_at(i64::MAX as i128, i128ty, span);
            let below = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Lt,
                    left: diff,
                    right: lo,
                },
                IrType::Boolean,
                span,
            );
            let above = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Gt,
                    left: diff,
                    right: hi,
                },
                IrType::Boolean,
                span,
            );
            let overflow = self.emit(
                InstKind::Binary {
                    op: BinaryOp::Or,
                    left: below,
                    right: above,
                },
                IrType::Boolean,
                span,
            );
            let fail = self.new_block();
            let cont = self.new_block();
            self.terminate(Terminator::Branch {
                condition: overflow,
                then_block: fail,
                else_block: cont,
            });
            self.current = fail;
            let native = self
                .checked
                .native_exceptions
                .expect("a program with DateTime arithmetic registered the exception hierarchy");
            self.throw_native_failure(
                native.arithmetic_overflow,
                "the DateTime difference exceeds the Duration range",
                span,
            );
            self.current = cont;
            let diff = self.emit(InstKind::Load(diff_slot), i128ty, span);
            return self.emit(InstKind::IntCast(diff), i64ty, span);
        }

        // `Time`(+|-)`Duration` wraps modulo one day; `Time - Time` is the
        // `Duration` between them. Both are `i64` runtime calls.
        let time = [left_base, right_base]
            .iter()
            .flatten()
            .any(|b| *b == zirk_sema::Base::Time);
        if time {
            match expr.op {
                Add | Sub
                    if self.is_duration(if left_base == Some(zirk_sema::Base::Time) {
                        &expr.right
                    } else {
                        &expr.left
                    }) =>
                {
                    let (time_op, duration_op, negate) = if left_base == Some(zirk_sema::Base::Time)
                    {
                        (left, right, expr.op == Sub)
                    } else {
                        (right, left, false)
                    };
                    let nanos = if negate {
                        // `0 - delta` plain: the negated count is only ever
                        // read modulo one day by `zirk_rt_time_add_nanos`,
                        // so the wrap on `i64::MIN` is unobservable — and a
                        // plain op opens no block `time_op` would cross.
                        let zero = self.const_int_at(0, IrType::Int(IntWidth::I64), span);
                        self.emit(
                            InstKind::Binary {
                                op: BinaryOp::Sub,
                                left: zero,
                                right: duration_op,
                            },
                            IrType::Int(IntWidth::I64),
                            span,
                        )
                    } else {
                        duration_op
                    };
                    return self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_time_add_nanos".to_string(),
                            args: vec![time_op, nanos],
                        },
                        IrType::Int(IntWidth::I64),
                        span,
                    );
                }
                Sub if left_base == Some(zirk_sema::Base::Time)
                    && right_base == Some(zirk_sema::Base::Time) =>
                {
                    return self.emit(
                        InstKind::Call {
                            callee: "zirk_rt_time_diff".to_string(),
                            args: vec![left, right],
                        },
                        IrType::Int(IntWidth::I64),
                        span,
                    );
                }
                _ => {}
            }
        }

        // Everything else is the plain integer operation over the
        // representation: `Date`(+|-)`Int`, `Date - Date`, and every
        // same-type comparison (`i64` for `Date`/`Time`, `i128` for
        // `DateTime`). The operand width is whichever side is temporal —
        // for `Date + Int` the other side is widened to `i64`.
        let temporal_ty = self.type_of(
            if left_base.is_some() {
                &expr.left
            } else {
                &expr.right
            },
            span,
        );
        let left_actual = self.type_of_operand(left);
        let right_actual = self.type_of_operand(right);
        let left = self.convert_numeric(left, left_actual, temporal_ty, expr.left.span());
        let right = self.convert_numeric(right, right_actual, temporal_ty, expr.right.span());
        self.emit_checked_binary(binary_op(expr.op), left, right, temporal_ty, span)
    }

    /// Lowers `Duration` arithmetic and comparison. A `Duration` is an `i64`
    /// nanosecond count, so `+`/`-`, comparisons and `Duration * Int` /
    /// `Duration / Int` are exactly the checked `i64` operations the same
    /// shapes already are for `Int64`. The cases an `i64` instruction cannot
    /// express — a `Float` scalar and the `Duration / Duration` ratio — go
    /// through the runtime.
    fn lower_duration_binary(
        &mut self,
        expr: &ast::BinaryExpr,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Operand {
        use ast::BinaryOp::*;

        let left_actual = self.type_of_operand(left);
        let right_actual = self.type_of_operand(right);
        let left_is_duration = self.is_duration(&expr.left);
        let right_is_duration = self.is_duration(&expr.right);

        // `Duration` (+|-) `Duration` and every comparison run directly on
        // the nanosecond count, with the same overflow check an `Int64`
        // operation gets.
        if matches!(expr.op, Add | Sub | Eq | NotEq | Lt | LtEq | Gt | GtEq) {
            let left = self.convert_numeric(
                left,
                left_actual,
                IrType::Int(IntWidth::I64),
                expr.left.span(),
            );
            let right = self.convert_numeric(
                right,
                right_actual,
                IrType::Int(IntWidth::I64),
                expr.right.span(),
            );
            return self.emit_checked_binary(
                binary_op(expr.op),
                left,
                right,
                IrType::Int(IntWidth::I64),
                span,
            );
        }

        // `Duration / Duration` answers a `Float64` ratio, not a duration.
        // The divisor's zero check is the same `DivisionByZeroError` integer
        // division gets.
        if expr.op == Div && left_is_duration && right_is_duration {
            let left = self.convert_numeric(
                left,
                left_actual,
                IrType::Int(IntWidth::I64),
                expr.left.span(),
            );
            let right = self.convert_numeric(
                right,
                right_actual,
                IrType::Int(IntWidth::I64),
                expr.right.span(),
            );
            return self.duration_ratio(left, right, span);
        }

        // `Duration (*|/) scalar`. The duration is whichever side carries it:
        // `*` is commutative in the checker too, so `2 * 1s` reaches here
        // with the scalar on the left.
        debug_assert!(
            !(expr.op == Mul && left_is_duration && right_is_duration),
            "the checker rejects `Duration * Duration`",
        );
        let (nanos, nanos_actual, scalar, scalar_actual, scalar_span) = if left_is_duration {
            (left, left_actual, right, right_actual, expr.right.span())
        } else {
            (right, right_actual, left, left_actual, expr.left.span())
        };
        let nanos = self.convert_numeric(nanos, nanos_actual, IrType::Int(IntWidth::I64), span);

        match scalar_actual {
            // An integer scalar keeps exact `Int64` semantics — `*` overflows
            // and `/` guards its divisor the same way.
            IrType::Int(_) => {
                let scalar = self.convert_numeric(
                    scalar,
                    scalar_actual,
                    IrType::Int(IntWidth::I64),
                    scalar_span,
                );
                self.emit_checked_binary(
                    binary_op(expr.op),
                    nanos,
                    scalar,
                    IrType::Int(IntWidth::I64),
                    span,
                )
            }
            // A `Float`/exact-`Float` scalar needs the runtime: the result is
            // a nanosecond count again, which no float instruction answers.
            // The exact `Float` is converted to `f64` first (`Duration`
            // arithmetic with a fractional scalar is already approximate).
            IrType::Float(_) | IrType::Decimal => {
                let scalar = self.convert_numeric(
                    scalar,
                    scalar_actual,
                    IrType::Float(FloatWidth::F64),
                    scalar_span,
                );
                if expr.op == Div {
                    return self.duration_float_div(nanos, scalar, span);
                }
                self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_duration_mul_f64".to_string(),
                        args: vec![nanos, scalar],
                    },
                    IrType::Int(IntWidth::I64),
                    span,
                )
            }
            _ => unreachable!("the checker only admits a numeric scalar here"),
        }
    }

    /// `Duration / Duration`: a `Float64` ratio, guarded against a zero
    /// divisor exactly like `checked_int_division` guards integer division.
    fn duration_ratio(&mut self, left: Operand, right: Operand, span: Span) -> Operand {
        let i64_ty = IrType::Int(IntWidth::I64);
        let left_slot = self.spill(left, i64_ty, span);
        let right_slot = self.spill(right, i64_ty, span);

        let divisor = self.emit(InstKind::Load(right_slot), i64_ty, span);
        let zero = self.const_int_at(0, i64_ty, span);
        let is_zero = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: divisor,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let ok = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_zero,
            then_block: fail,
            else_block: ok,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with division registered the exception hierarchy");
        self.throw_native_failure(native.division_by_zero, "division by zero", span);

        self.current = ok;
        let left = self.emit(InstKind::Load(left_slot), i64_ty, span);
        let right = self.emit(InstKind::Load(right_slot), i64_ty, span);
        self.emit(
            InstKind::Call {
                callee: "zirk_rt_duration_div_duration".to_string(),
                args: vec![left, right],
            },
            IrType::Float(FloatWidth::F64),
            span,
        )
    }

    /// `Duration / Float` scalar: a zero scalar throws the same
    /// `DivisionByZeroError` an integer divisor does — a `Duration` cannot
    /// hold the infinity `nanos / 0.0` would otherwise produce.
    fn duration_float_div(&mut self, nanos: Operand, scalar: Operand, span: Span) -> Operand {
        let i64_ty = IrType::Int(IntWidth::I64);
        let f64_ty = IrType::Float(FloatWidth::F64);
        let nanos_slot = self.spill(nanos, i64_ty, span);
        let scalar_slot = self.spill(scalar, f64_ty, span);

        let divisor = self.emit(InstKind::Load(scalar_slot), f64_ty, span);
        let zero = self.emit(
            InstKind::ConstFloat(FloatWidth::F64, "0".to_string()),
            f64_ty,
            span,
        );
        let is_zero = self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left: divisor,
                right: zero,
            },
            IrType::Boolean,
            span,
        );
        let fail = self.new_block();
        let ok = self.new_block();
        self.terminate(Terminator::Branch {
            condition: is_zero,
            then_block: fail,
            else_block: ok,
        });

        self.current = fail;
        let native = self
            .checked
            .native_exceptions
            .expect("a program with division registered the exception hierarchy");
        self.throw_native_failure(native.division_by_zero, "division by zero", span);

        self.current = ok;
        let nanos = self.emit(InstKind::Load(nanos_slot), i64_ty, span);
        let scalar = self.emit(InstKind::Load(scalar_slot), f64_ty, span);
        self.emit(
            InstKind::Call {
                callee: "zirk_rt_duration_div_f64".to_string(),
                args: vec![nanos, scalar],
            },
            i64_ty,
            span,
        )
    }

    /// Lowers a binary operator whose operands are both exact base-ten
    /// `Float`. Arithmetic goes to `zirk_rt_decimal_*` calls; `/` and `%`
    /// first branch to `DivisionByZeroError` when the divisor is zero, the
    /// same guard integer and `Duration` division get. A comparison lowers
    /// to `zirk_rt_decimal_cmp` and then the plain integer comparison of its
    /// `-1`/`0`/`1` result against zero.
    fn lower_decimal_binary(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Operand {
        let dec = IrType::Decimal;

        let callee = match op {
            BinaryOp::Add => Some("zirk_rt_decimal_add"),
            BinaryOp::Sub => Some("zirk_rt_decimal_sub"),
            BinaryOp::Mul => Some("zirk_rt_decimal_mul"),
            BinaryOp::Div => Some("zirk_rt_decimal_div"),
            BinaryOp::Rem => Some("zirk_rt_decimal_rem"),
            _ => None,
        };

        if let Some(callee) = callee {
            let (left, right) = if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
                let left_slot = self.spill(left, dec, span);
                let right_slot = self.spill(right, dec, span);
                let divisor = self.emit(InstKind::Load(right_slot), dec, span);
                let is_zero = self.emit(
                    InstKind::Call {
                        callee: "zirk_rt_decimal_is_zero".to_string(),
                        args: vec![divisor],
                    },
                    IrType::Boolean,
                    span,
                );
                let fail = self.new_block();
                let ok = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: is_zero,
                    then_block: fail,
                    else_block: ok,
                });
                self.current = fail;
                let native = self
                    .checked
                    .native_exceptions
                    .expect("a program with division registered the exception hierarchy");
                self.throw_native_failure(native.division_by_zero, "division by zero", span);
                self.current = ok;
                (
                    self.emit(InstKind::Load(left_slot), dec, span),
                    self.emit(InstKind::Load(right_slot), dec, span),
                )
            } else {
                (left, right)
            };
            return self.emit(
                InstKind::Call {
                    callee: callee.to_string(),
                    args: vec![left, right],
                },
                dec,
                span,
            );
        }

        // A comparison: `cmp(left, right) <op> 0`.
        let ordering = self.emit(
            InstKind::Call {
                callee: "zirk_rt_decimal_cmp".to_string(),
                args: vec![left, right],
            },
            IrType::Int(IntWidth::I32),
            span,
        );
        let zero = self.const_int_at(0, IrType::Int(IntWidth::I32), span);
        self.emit(
            InstKind::Binary {
                op,
                left: ordering,
                right: zero,
            },
            IrType::Boolean,
            span,
        )
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
        Lt => "_less",
        LtEq => "_less_equal",
        Gt => "_greater",
        GtEq => "_greater_equal",
        _ => return None,
    })
}

/// Whether an operator on these operands is one of `String`'s.
fn is_string_operator(left: IrType, right: IrType, op: ast::BinaryOp) -> bool {
    matches!(
        (left, right, op),
        (IrType::String, IrType::String, ast::BinaryOp::Add)
            | (IrType::String, IrType::Int(_), ast::BinaryOp::Mul)
            | (IrType::Int(_), IrType::String, ast::BinaryOp::Mul)
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

/// The arguments `FunctionLowering::lower_native_slice_construction` needs
/// (roadmap Phase 4e, `fase-4e-native-slice`, design D1/D3/D5), bundled into
/// one struct rather than passed positionally — clippy's own
/// `too_many_arguments` threshold, and each field already has a name worth
/// reading at the one call site that builds it.
struct NativeSliceConstruction {
    /// Already spilled by the caller, immediately after `pointer` was
    /// lowered (see the function's own doc comment on why).
    pointer_slot: SlotId,
    length: Operand,
    pointer_type_id: u32,
    element: IrType,
    mutable: bool,
    known_length: Option<u64>,
}

/// The key one `from_name`/`from_value` comparison tests the argument
/// against (`enum-static-members`) — resolved ahead of the block chain it
/// is emitted into, since an IR value cannot cross blocks.
enum Key {
    /// A declared name or a `-> "text"` mapping.
    Text(String),
    /// A `-> n` mapping or the case's own discriminant.
    Int(i128),
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
