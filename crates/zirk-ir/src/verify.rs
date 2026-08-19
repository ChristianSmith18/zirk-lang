//! Well-formedness verifier for the IR.
//!
//! It exists so a lowering bug surfaces as a precise message here instead of as
//! an unreadable LLVM failure — or, worse, as a binary that miscompiles
//! silently. It is the IR equivalent of `Module::verify()` in LLVM.
//!
//! It is not a type checker for Zirk: by the time we reach the IR the program
//! has already been verified by `zirk-sema`. What is checked here are the
//! invariants of the *representation*.

use crate::ir::*;
use std::collections::HashMap;

/// A violation of the IR invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrError {
    pub function: String,
    pub detail: String,
}

impl std::fmt::Display for IrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "in `{}`: {}", self.function, self.detail)
    }
}

/// Verifies an entire module.
pub fn verify(module: &Module) -> Result<(), Vec<IrError>> {
    let mut errors = Vec::new();

    for function in &module.functions {
        verify_function(module, function, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_function(module: &Module, function: &Function, errors: &mut Vec<IrError>) {
    let mut report = |detail: String| {
        errors.push(IrError {
            function: function.name.clone(),
            detail,
        })
    };

    if function.block(function.entry).is_none() {
        report(format!(
            "the entry block {:?} does not exist",
            function.entry
        ));
        return;
    }

    let block_ids: Vec<BlockId> = function.blocks.iter().map(|b| b.id).collect();

    // Every definition in the function, collected up front so the verifier can
    // tell a value that does not exist from one defined further down: they are
    // different defects and deserve different messages.
    let mut declared: HashMap<ValueId, BlockId> = HashMap::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            if let Some(result) = inst.result {
                declared.entry(result).or_insert(block.id);
            }
        }
    }

    // Value types, and the block each was defined in. In this design values do
    // not cross blocks: locals travel through slots, which is what allows
    // skipping SSA and phi nodes.
    let mut values: HashMap<ValueId, (BlockId, IrType)> = HashMap::new();

    for block in &function.blocks {
        let mut defined_here: Vec<ValueId> = Vec::new();

        for (index, inst) in block.instructions.iter().enumerate() {
            let position = format!("block {:?}, instruction {index}", block.id);

            for operand in operands_of(&inst.kind) {
                if defined_here.contains(&operand.0) {
                    continue;
                }

                match declared.get(&operand.0) {
                    None => report(format!("{position}: uses undefined value {:?}", operand.0)),
                    Some(origin) if *origin != block.id => report(format!(
                        "{position}: uses value {:?} defined in another block ({origin:?}); \
                         values do not cross blocks in this IR",
                        operand.0
                    )),
                    Some(_) => report(format!(
                        "{position}: uses value {:?} before it is defined",
                        operand.0
                    )),
                }
            }

            verify_instruction(module, function, inst, &values, &position, &mut report);

            if let Some(result) = inst.result {
                if values.contains_key(&result) {
                    report(format!("{position}: value {result:?} is defined twice"));
                }
                values.insert(result, (block.id, inst.ty));
                defined_here.push(result);
            }
        }

        // Every block ends in exactly one terminator (design D2).
        let Some(terminator) = &block.terminator else {
            report(format!("block {:?} has no terminator", block.id));
            continue;
        };

        for successor in terminator.successors() {
            if !block_ids.contains(&successor) {
                report(format!(
                    "block {:?} jumps to {successor:?}, which does not exist",
                    block.id
                ));
            }
        }

        verify_terminator(function, block, terminator, &values, &mut report);
    }
}

fn verify_instruction(
    module: &Module,
    function: &Function,
    inst: &Instruction,
    values: &HashMap<ValueId, (BlockId, IrType)>,
    position: &str,
    report: &mut impl FnMut(String),
) {
    let type_of = |operand: &Operand| values.get(&operand.0).map(|(_, ty)| *ty);

    match &inst.kind {
        InstKind::ConstInt(_) => expect(inst.ty, IrType::Int32, position, "ConstInt", report),
        InstKind::ConstBool(_) => expect(inst.ty, IrType::Boolean, position, "ConstBool", report),
        InstKind::ConstString(id) => {
            expect(inst.ty, IrType::String, position, "ConstString", report);
            if module.strings.get(id.0 as usize).is_none() {
                report(format!(
                    "{position}: string {id:?} is not in the module table"
                ));
            }
        }

        InstKind::Alloc(id) => match module.objects.get(*id as usize) {
            None => report(format!(
                "{position}: allocates object layout {id}, which is not in the module table"
            )),
            Some(_) => expect(inst.ty, IrType::Object(*id), position, "Alloc", report),
        },

        InstKind::CheckedCast {
            object,
            target_class,
        } => match module.objects.get(*target_class as usize) {
            None => report(format!(
                "{position}: casts to object layout {target_class}, which is not in the module table"
            )),
            Some(_) => {
                expect(
                    inst.ty,
                    IrType::Object(*target_class),
                    position,
                    "CheckedCast",
                    report,
                );
                if let Some(ty) = type_of(object)
                    && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
                {
                    report(format!(
                        "{position}: casts {}, which is neither an object nor a contract",
                        ty.as_str()
                    ));
                }
            }
        },

        InstKind::Retype(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: retypes {}, which is neither an object nor a contract",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Object(_) | IrType::Contract(_)) {
                report(format!(
                    "{position}: Retype declares {}, which is neither an object nor a contract",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::BuildValue { class, fields } => match module.values.get(*class as usize) {
            None => report(format!(
                "{position}: builds value layout {class}, which is not in the module table"
            )),
            Some(layout) => {
                expect(inst.ty, IrType::Value(*class), position, "BuildValue", report);
                if fields.len() != layout.fields.len() {
                    report(format!(
                        "{position}: builds {} with {} field{}, not {}",
                        layout.name,
                        fields.len(),
                        if fields.len() == 1 { "" } else { "s" },
                        layout.fields.len()
                    ));
                }
                for (field, given) in layout.fields.iter().zip(fields) {
                    if let Some(actual) = type_of(given)
                        && actual != field.ty
                    {
                        report(format!(
                            "{position}: builds field `{}` of type {} from a {}",
                            field.name,
                            field.ty.as_str(),
                            actual.as_str()
                        ));
                    }
                }
            }
        },

        InstKind::BuildEnum {
            enum_id,
            variant,
            fields,
        } => match module.enums.get(*enum_id as usize) {
            None => report(format!(
                "{position}: builds enum layout {enum_id}, which is not in the module table"
            )),
            Some(layout) => match layout.variants.get(*variant as usize) {
                None => report(format!(
                    "{position}: builds {}, which has no variant {variant}",
                    layout.name
                )),
                Some(indices) => {
                    expect(inst.ty, IrType::Enum(*enum_id), position, "BuildEnum", report);
                    if fields.len() != indices.len() {
                        report(format!(
                            "{position}: builds variant {variant} of {} with {} field{}, not {}",
                            layout.name,
                            fields.len(),
                            if fields.len() == 1 { "" } else { "s" },
                            indices.len()
                        ));
                    }
                    for (&index, given) in indices.iter().zip(fields) {
                        let Some(field) = layout.fields.get(index as usize) else {
                            continue;
                        };
                        if let Some(actual) = type_of(given)
                            && actual != field.ty
                        {
                            report(format!(
                                "{position}: builds field `{}` of type {} from a {}",
                                field.name,
                                field.ty.as_str(),
                                actual.as_str()
                            ));
                        }
                    }
                }
            },
        },

        InstKind::Discriminant(operand) => {
            expect(inst.ty, IrType::Int32, position, "Discriminant", report);
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Enum(_))
            {
                report(format!(
                    "{position}: reads the discriminant of {}, which is not an algebraic enum",
                    ty.as_str()
                ));
            }
        }

        InstKind::Concat { left, right } => {
            expect(inst.ty, IrType::String, position, "Concat", report);
            for operand in [left, right] {
                if let Some(ty) = type_of(operand)
                    && ty != IrType::String
                {
                    report(format!(
                        "{position}: concatenates {}, which is not a String",
                        ty.as_str()
                    ));
                }
            }
        }

        InstKind::Repeat { string, count } => {
            expect(inst.ty, IrType::String, position, "Repeat", report);
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: repeats {}, which is not a String",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(count)
                && ty != IrType::Int32
            {
                report(format!(
                    "{position}: repeats by {}, which is not a count",
                    ty.as_str()
                ));
            }
        }

        InstKind::CallContract {
            object,
            contract,
            index,
            ..
        } => {
            let reachable = match type_of(object) {
                Some(IrType::Contract(id)) => id == *contract,
                // A class reaches a contract it satisfies.
                Some(IrType::Object(id)) => module
                    .objects
                    .get(id as usize)
                    .is_some_and(|o| o.contracts.iter().any(|t| t.contract == *contract)),
                _ => false,
            };
            if !reachable {
                report(format!(
                    "{position}: calls method {index} of contract {contract} through something that does not satisfy it"
                ));
            }
        }

        InstKind::CallVirtual { object, index, .. } => {
            let known = match type_of(object) {
                Some(IrType::Object(id)) => module
                    .objects
                    .get(id as usize)
                    .is_some_and(|o| (*index as usize) < o.methods.len()),
                _ => false,
            };
            if !known {
                report(format!(
                    "{position}: calls method {index} through something that is not an object with it"
                ));
            }
        }

        InstKind::LoadField { object, index } => {
            match field_type(module, type_of(object), *index) {
                Some(ty) => expect(inst.ty, ty, position, "LoadField", report),
                None => report(format!(
                    "{position}: reads field {index} of something that is not an object with it"
                )),
            }
        }

        InstKind::StoreField {
            object,
            index,
            value,
        } => {
            expect(inst.ty, IrType::Void, position, "StoreField", report);
            match field_type(module, type_of(object), *index) {
                Some(ty) => {
                    if let Some(actual) = type_of(value)
                        && actual != ty
                    {
                        report(format!(
                            "{position}: stores {} into a field of type {}",
                            actual.as_str(),
                            ty.as_str()
                        ));
                    }
                }
                None => report(format!(
                    "{position}: writes field {index} of something that is not an object with it"
                )),
            }
        }

        InstKind::Load(slot) => match function.slot(*slot) {
            None => report(format!(
                "{position}: reads slot {slot:?}, which does not exist"
            )),
            Some(s) => expect(inst.ty, s.ty, position, "Load", report),
        },

        InstKind::Store(slot, operand) => {
            expect(inst.ty, IrType::Void, position, "Store", report);
            match function.slot(*slot) {
                None => report(format!(
                    "{position}: writes slot {slot:?}, which does not exist"
                )),
                Some(s) => {
                    if let Some(value) = type_of(operand)
                        && value != s.ty
                    {
                        report(format!(
                            "{position}: stores {} into a slot of type {}",
                            value.as_str(),
                            s.ty.as_str()
                        ));
                    }
                }
            }
        }

        InstKind::Unary { op, operand } => {
            let expected = match op {
                UnaryOp::Neg => IrType::Int32,
                UnaryOp::Not => IrType::Boolean,
            };
            if let Some(value) = type_of(operand)
                && value != expected
            {
                report(format!(
                    "{position}: {op:?} applied to {}, expected {}",
                    value.as_str(),
                    expected.as_str()
                ));
            }
            expect(inst.ty, expected, position, "Unary", report);
        }

        InstKind::Binary { op, left, right } => {
            let (Some(left_ty), Some(right_ty)) = (type_of(left), type_of(right)) else {
                return;
            };
            if left_ty != right_ty {
                report(format!(
                    "{position}: {op:?} between {} and {}",
                    left_ty.as_str(),
                    right_ty.as_str()
                ));
            }
            expect(inst.ty, op.result_type(left_ty), position, "Binary", report);
        }

        InstKind::Call { callee, args } => match module.function(callee) {
            None => report(format!(
                "{position}: calls `{callee}`, which does not exist"
            )),
            Some(target) => {
                if args.len() != target.params.len() {
                    report(format!(
                        "{position}: calls `{callee}` with {} argument(s), expected {}",
                        args.len(),
                        target.params.len()
                    ));
                }
                expect(inst.ty, target.return_type, position, "Call", report);
            }
        },

        InstKind::ToString(operand) => {
            expect(inst.ty, IrType::String, position, "ToString", report);
            if let Some(value) = type_of(operand)
                && value == IrType::Void
            {
                report(format!("{position}: ToString applied to Void"));
            }
        }

        InstKind::Println(operand) => {
            expect(inst.ty, IrType::Void, position, "Println", report);
            // The runtime reads the operand as a string handle: any other type
            // would be interpreted as a pointer to an arbitrary address.
            if let Some(value) = type_of(operand)
                && value != IrType::String
            {
                report(format!(
                    "{position}: Println receives {}, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::NullValue(base) => {
            expect(
                inst.ty,
                IrType::Nullable(*base),
                position,
                "NullValue",
                report,
            );
        }

        InstKind::Wrap { base, value } => {
            expect(inst.ty, IrType::Nullable(*base), position, "Wrap", report);
            if let Some(inner) = type_of(value)
                && inner != base.inner()
            {
                report(format!(
                    "{position}: Wrap receives {}, expected {}",
                    inner.as_str(),
                    base.inner().as_str()
                ));
            }
        }

        InstKind::IsNull(operand) => {
            expect(inst.ty, IrType::Boolean, position, "IsNull", report);
            if let Some(value) = type_of(operand)
                && !matches!(value, IrType::Nullable(_))
            {
                report(format!(
                    "{position}: IsNull receives {}, which is never absent",
                    value.as_str()
                ));
            }
        }

        InstKind::MakeClosure { id, captures } => {
            expect(
                inst.ty,
                IrType::Closure(*id),
                position,
                "MakeClosure",
                report,
            );

            let Some(layout) = module.closures.get(*id as usize) else {
                report(format!(
                    "{position}: MakeClosure names layout {id}, which does not exist"
                ));
                return;
            };

            if captures.len() != layout.captures.len() {
                report(format!(
                    "{position}: MakeClosure passes {} capture(s), the layout declares {}",
                    captures.len(),
                    layout.captures.len()
                ));
                return;
            }

            for (index, (operand, expected)) in captures.iter().zip(&layout.captures).enumerate() {
                if let Some(actual) = type_of(operand)
                    && actual != *expected
                {
                    report(format!(
                        "{position}: capture {index} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }

        InstKind::CallClosure { id, callee, args } => {
            let id = *id;
            // The operand must be the very closure the id names: otherwise the
            // captures extracted from it would not match what the body expects.
            if let Some(actual) = type_of(callee)
                && actual != IrType::Closure(id)
            {
                report(format!(
                    "{position}: CallClosure receives {}, expected {}",
                    actual.as_str(),
                    IrType::Closure(id).as_str()
                ));
                return;
            }

            let Some(layout) = module.closures.get(id as usize) else {
                report(format!(
                    "{position}: CallClosure names layout {id}, which does not exist"
                ));
                return;
            };

            expect(inst.ty, layout.returns, position, "CallClosure", report);

            if args.len() != layout.params.len() {
                report(format!(
                    "{position}: CallClosure passes {} argument(s), the closure takes {}",
                    args.len(),
                    layout.params.len()
                ));
                return;
            }

            for (index, (operand, expected)) in args.iter().zip(&layout.params).enumerate() {
                if let Some(actual) = type_of(operand)
                    && actual != *expected
                {
                    report(format!(
                        "{position}: argument {index} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }

        InstKind::Unwrap(operand) => {
            // Unwrapping must produce exactly the type inside the operand:
            // that is what makes the representation change checkable rather
            // than implicit.
            if let Some(value) = type_of(operand) {
                match value {
                    IrType::Nullable(base) => {
                        expect(inst.ty, base.inner(), position, "Unwrap", report)
                    }
                    other => report(format!(
                        "{position}: Unwrap receives {}, which is not nullable",
                        other.as_str()
                    )),
                }
            }
        }
    }
}

fn verify_terminator(
    function: &Function,
    block: &Block,
    terminator: &Terminator,
    values: &HashMap<ValueId, (BlockId, IrType)>,
    report: &mut impl FnMut(String),
) {
    let position = format!("block {:?}, terminator", block.id);

    match terminator {
        // Nothing to check: a block nothing reaches transfers control nowhere.
        Terminator::Unreachable => {}

        Terminator::Return(value) => {
            let returned = match value {
                None => IrType::Void,
                Some(operand) => match values.get(&operand.0) {
                    Some((origin, ty)) => {
                        if *origin != block.id {
                            report(format!(
                                "{position}: returns value {:?} defined in another block",
                                operand.0
                            ));
                        }
                        *ty
                    }
                    None => {
                        report(format!(
                            "{position}: returns undefined value {:?}",
                            operand.0
                        ));
                        return;
                    }
                },
            };

            if returned != function.return_type {
                report(format!(
                    "{position}: returns {} in a function declared {}",
                    returned.as_str(),
                    function.return_type.as_str()
                ));
            }
        }

        Terminator::Branch { condition, .. } => match values.get(&condition.0) {
            None => report(format!(
                "{position}: branches on undefined value {:?}",
                condition.0
            )),
            Some((_, ty)) if *ty != IrType::Boolean => report(format!(
                "{position}: branches on {}, expected Boolean",
                ty.as_str()
            )),
            Some(_) => {}
        },

        Terminator::Jump(_) => {}
    }
}

fn expect(
    actual: IrType,
    expected: IrType,
    position: &str,
    what: &str,
    report: &mut impl FnMut(String),
) {
    if actual != expected {
        report(format!(
            "{position}: {what} declares type {} and should be {}",
            actual.as_str(),
            expected.as_str()
        ));
    }
}

/// The type of field `index` of an object, if the operand is one and has it.
fn field_type(module: &Module, object: Option<IrType>, index: u32) -> Option<IrType> {
    match object? {
        IrType::Object(id) => module
            .objects
            .get(id as usize)?
            .fields
            .get(index as usize)
            .map(|f| f.ty),
        IrType::Value(id) => module
            .values
            .get(id as usize)?
            .fields
            .get(index as usize)
            .map(|f| f.ty),
        IrType::Enum(id) => module
            .enums
            .get(id as usize)?
            .fields
            .get(index as usize)
            .map(|f| f.ty),
        _ => None,
    }
}

/// Operands an instruction reads.
fn operands_of(kind: &InstKind) -> Vec<Operand> {
    match kind {
        InstKind::ConstInt(_) | InstKind::ConstBool(_) | InstKind::ConstString(_) => Vec::new(),
        InstKind::Load(_) => Vec::new(),
        InstKind::Store(_, operand) => vec![*operand],
        InstKind::Alloc(_) => Vec::new(),
        InstKind::BuildValue { fields, .. } => fields.clone(),
        InstKind::BuildEnum { fields, .. } => fields.clone(),
        InstKind::Discriminant(operand) => vec![*operand],
        InstKind::CheckedCast { object, .. } => vec![*object],
        InstKind::Retype(operand) => vec![*operand],
        InstKind::Concat { left, right } => vec![*left, *right],
        InstKind::Repeat { string, count } => vec![*string, *count],
        InstKind::CallContract { object, args, .. } => {
            let mut operands = vec![*object];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::CallVirtual { object, args, .. } => {
            let mut operands = vec![*object];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::LoadField { object, .. } => vec![*object],
        InstKind::StoreField { object, value, .. } => vec![*object, *value],
        InstKind::Unary { operand, .. } => vec![*operand],
        InstKind::Binary { left, right, .. } => vec![*left, *right],
        InstKind::Call { args, .. } => args.clone(),
        InstKind::ToString(operand) => vec![*operand],
        InstKind::Println(operand) => vec![*operand],
        InstKind::NullValue(_) => Vec::new(),
        InstKind::Wrap { value, .. } => vec![*value],
        InstKind::IsNull(operand) | InstKind::Unwrap(operand) => vec![*operand],
        InstKind::MakeClosure { captures, .. } => captures.clone(),
        InstKind::CallClosure { callee, args, .. } => {
            let mut operands = vec![*callee];
            operands.extend(args.iter().copied());
            operands
        }
    }
}
