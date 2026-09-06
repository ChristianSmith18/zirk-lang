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
        InstKind::ConstInt(_) => {
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: ConstInt must produce an Int type, got {ty:?}",
                    ty = inst.ty
                ));
            }
        }
        InstKind::ConstBool(_) => expect(inst.ty, IrType::Boolean, position, "ConstBool", report),
        InstKind::ConstString(id) => {
            expect(inst.ty, IrType::String, position, "ConstString", report);
            if module.strings.get(id.0 as usize).is_none() {
                report(format!(
                    "{position}: string {id:?} is not in the module table"
                ));
            }
        }
        InstKind::ConstChar(id) => {
            expect(inst.ty, IrType::Char, position, "ConstChar", report);
            if module.strings.get(id.0 as usize).is_none() {
                report(format!(
                    "{position}: string {id:?} is not in the module table"
                ));
            }
        }

        InstKind::FatalError(message) => {
            expect(inst.ty, IrType::Never, position, "FatalError", report);
            if let Some(ty) = type_of(message)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: FatalError's message is {}, expected String",
                    ty.as_str()
                ));
            }
        }

        InstKind::Throw(exception) => {
            expect(inst.ty, IrType::Void, position, "Throw", report);
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: Throw's exception is {}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::SetSuppressed {
            exception,
            suppressed,
        } => {
            expect(inst.ty, IrType::Void, position, "SetSuppressed", report);
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: SetSuppressed's exception is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(suppressed)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: SetSuppressed's suppressed is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::StackTrace(exception) => {
            expect(inst.ty, IrType::String, position, "StackTrace", report);
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: StackTrace's exception is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::Suppressed(exception) => {
            if !matches!(inst.ty, IrType::Nullable(_)) {
                report(format!(
                    "{position}: Suppressed expects a nullable type, found {}",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: Suppressed's exception is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::HasPendingException => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "HasPendingException",
                report,
            );
        }

        InstKind::IsCancelled => {
            expect(inst.ty, IrType::Boolean, position, "IsCancelled", report);
        }

        // No shape to check: `ty` is whichever class the matching `catch`
        // declared (roadmap Phase 4b) — see `InstKind::TakePendingException`'s
        // own doc comment.
        InstKind::TakePendingException => {}

        InstKind::StringGraphemeOffset { string, index } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I64),
                position,
                "StringGraphemeOffset",
                report,
            );
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: StringGraphemeOffset reads {}, which is not a String",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: StringGraphemeOffset's index is {}, expected Int64",
                    ty.as_str()
                ));
            }
        }

        InstKind::GraphemeLenAt { string, offset } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I64),
                position,
                "GraphemeLenAt",
                report,
            );
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: GraphemeLenAt reads {}, which is not a String",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(offset)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: GraphemeLenAt's offset is {}, expected Int64",
                    ty.as_str()
                ));
            }
        }

        InstKind::GraphemeSlice {
            string,
            offset,
            len,
        } => {
            expect(inst.ty, IrType::Char, position, "GraphemeSlice", report);
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: GraphemeSlice reads {}, which is not a String",
                    ty.as_str()
                ));
            }
            for (name, operand) in [("offset", offset), ("len", len)] {
                if let Some(ty) = type_of(operand)
                    && ty != IrType::Int(IntWidth::I64)
                {
                    report(format!(
                        "{position}: GraphemeSlice's {name} is {}, expected Int64",
                        ty.as_str()
                    ));
                }
            }
        }

        // No shape to check: `ty` is whatever the caller asked for, by
        // construction (roadmap Phase 4b's own `throw` early-return
        // placeholder) — see `InstKind::Undefined`'s own doc comment.
        InstKind::Undefined => {}

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

        InstKind::IsInstance {
            object,
            target_class,
        } => {
            if module.objects.get(*target_class as usize).is_none() {
                report(format!(
                    "{position}: tests object layout {target_class}, which is not in the module table"
                ));
            }
            expect(inst.ty, IrType::Boolean, position, "IsInstance", report);
            if let Some(ty) = type_of(object)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: tests {}, which is neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

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

        InstKind::IntCast(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: IntCast converts {}, which is not an integer",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: IntCast declares {}, which is not an integer",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::ConstFloat(width, _) => expect(
            inst.ty,
            IrType::Float(*width),
            position,
            "ConstFloat",
            report,
        ),

        InstKind::FloatCast(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Float(_))
            {
                report(format!(
                    "{position}: FloatCast converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Float(_)) {
                report(format!(
                    "{position}: FloatCast declares {}, which is not a Float",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::IntToFloat(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: IntToFloat converts {}, which is not an integer",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Float(_)) {
                report(format!(
                    "{position}: IntToFloat declares {}, which is not a Float",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::FloatToInt(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Float(_))
            {
                report(format!(
                    "{position}: FloatToInt converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: FloatToInt declares {}, which is not an integer",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::ConstDecimal(_) => {
            expect(inst.ty, IrType::Decimal, position, "ConstDecimal", report)
        }
        InstKind::IntToDecimal(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: IntToDecimal converts {}, which is not an integer",
                    ty.as_str()
                ));
            }
            expect(inst.ty, IrType::Decimal, position, "IntToDecimal", report);
        }
        InstKind::FloatToDecimal(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Float(_))
            {
                report(format!(
                    "{position}: FloatToDecimal converts {}, which is not a BinaryFloat",
                    ty.as_str()
                ));
            }
            expect(inst.ty, IrType::Decimal, position, "FloatToDecimal", report);
        }
        InstKind::DecimalToInt(operand) => {
            if let Some(ty) = type_of(operand)
                && ty != IrType::Decimal
            {
                report(format!(
                    "{position}: DecimalToInt converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: DecimalToInt declares {}, which is not an integer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::DecimalToFloat(operand) => {
            if let Some(ty) = type_of(operand)
                && ty != IrType::Decimal
            {
                report(format!(
                    "{position}: DecimalToFloat converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Float(_)) {
                report(format!(
                    "{position}: DecimalToFloat declares {}, which is not a BinaryFloat",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::BuildValue { class, fields } => match module.values.get(*class as usize) {
            None => report(format!(
                "{position}: builds value layout {class}, which is not in the module table"
            )),
            Some(layout) => {
                expect(
                    inst.ty,
                    IrType::Value(*class),
                    position,
                    "BuildValue",
                    report,
                );
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
                    expect(
                        inst.ty,
                        IrType::Enum(*enum_id),
                        position,
                        "BuildEnum",
                        report,
                    );
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
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "Discriminant",
                report,
            );
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
                && ty != IrType::Int(IntWidth::I32)
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
            // Every unary operator preserves its operand's own type — `Neg`
            // and `BitNot` at whatever integer width it is (`Neg` also over
            // any `Float` width, roadmap Phase 3b), `Not` over `Boolean` —
            // so there is one shape to check, not a fixed `Int32` the way
            // this read before the integer-widths migration generalized
            // everything else: an operand at another width or `Float` never
            // exercised this path in a test, so the omission went unnoticed
            // until a `Float` unary `Neg` did.
            if let Some(value) = type_of(operand) {
                let ok = match op {
                    UnaryOp::Neg => matches!(value, IrType::Int(_) | IrType::Float(_)),
                    UnaryOp::BitNot => matches!(value, IrType::Int(_)),
                    UnaryOp::Not => value == IrType::Boolean,
                };
                if !ok {
                    report(format!("{position}: {op:?} applied to {}", value.as_str()));
                }
                expect(inst.ty, value, position, "Unary", report);
            }
        }

        InstKind::Binary { op, left, right } => {
            let (Some(left_ty), Some(right_ty)) = (type_of(left), type_of(right)) else {
                return;
            };
            // A shift's amount is its own, independent integer width — the
            // checker never requires it to match the shifted value's
            // (roadmap Phase 3b, task 4.4); every other operator still
            // requires both sides to agree.
            let shares_type = matches!(op, BinaryOp::Shl | BinaryOp::Shr) || left_ty == right_ty;
            if !shares_type {
                report(format!(
                    "{position}: {op:?} between {} and {}",
                    left_ty.as_str(),
                    right_ty.as_str()
                ));
            }
            expect(inst.ty, op.result_type(left_ty), position, "Binary", report);
        }

        InstKind::CheckedArithmetic {
            op, left, right, ..
        } => {
            let (Some(left_ty), Some(right_ty)) = (type_of(left), type_of(right)) else {
                return;
            };
            if left_ty != right_ty {
                report(format!(
                    "{position}: CheckedArithmetic between {} and {}",
                    left_ty.as_str(),
                    right_ty.as_str()
                ));
            }
            if !matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
                report(format!(
                    "{position}: CheckedArithmetic does not cover {op:?}"
                ));
            }
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "CheckedArithmetic",
                report,
            );
        }

        // An `extern "C" fn` (roadmap Phase 4e, design D7) shares this same
        // instruction — no body to check argument count against a
        // `Function`'s own slots, only its declared signature.
        InstKind::Call { callee, args } if module.function(callee).is_none() => {
            match module.externs.iter().find(|e| &e.name == callee) {
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
            }
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

        InstKind::MakeClosure {
            id,
            captures,
            target,
        } => {
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

            if module.function(target).is_none() {
                report(format!(
                    "{position}: MakeClosure targets `{target}`, which is not a module function"
                ));
                return;
            }

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

        InstKind::PointerFromSlot(slot) => {
            let Some(declared) = function.slot(*slot) else {
                report(format!(
                    "{position}: PointerFromSlot names slot {slot:?}, which does not exist"
                ));
                return;
            };
            if !matches!(inst.ty, IrType::Pointer(id) if module.pointer_types.get(id as usize) == Some(&declared.ty))
            {
                report(format!(
                    "{position}: PointerFromSlot declares {}, expected a Pointer to {}",
                    inst.ty.as_str(),
                    declared.ty.as_str()
                ));
            }
        }
        InstKind::PointerFromField { object, .. } => {
            if let Some(ty) = type_of(object) {
                match ty {
                    IrType::Pointer(id) => {
                        if !matches!(
                            module.pointer_types.get(id as usize),
                            Some(IrType::Object(_) | IrType::Value(_))
                        ) {
                            report(format!(
                                "{position}: PointerFromField reads {}, which is not a Pointer to an object or value",
                                ty.as_str()
                            ));
                        }
                    }
                    other => {
                        report(format!(
                            "{position}: PointerFromField reads {}, which is not a Pointer",
                            other.as_str()
                        ));
                    }
                }
            }
            if !matches!(inst.ty, IrType::Pointer(_)) {
                report(format!(
                    "{position}: PointerFromField declares {}, expected a Pointer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PointerRead(pointer) => {
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerRead reads through {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
        }
        InstKind::PointerWrite { pointer, .. } => {
            expect(inst.ty, IrType::Void, position, "PointerWrite", report);
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerWrite writes through {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
        }
        InstKind::PointerOffset { pointer, .. } | InstKind::PointerOffsetBytes { pointer, .. } => {
            if let Some(ty) = type_of(pointer)
                && ty != inst.ty
            {
                report(format!(
                    "{position}: pointer offset declares {}, expected {} (the operand's own type)",
                    inst.ty.as_str(),
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Pointer(_)) {
                report(format!(
                    "{position}: pointer offset declares {}, expected a Pointer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PointerCast(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerCast converts {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Pointer(_)) {
                report(format!(
                    "{position}: PointerCast declares {}, expected a Pointer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PointerIsNull(operand) => {
            expect(inst.ty, IrType::Boolean, position, "PointerIsNull", report);
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerIsNull reads {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
        }

        InstKind::NativeSliceValidate {
            pointer, length, ..
        } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NativeSliceValidate",
                report,
            );
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: NativeSliceValidate validates {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(length)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceValidate's length is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceValue { pointer, length } => {
            if !matches!(inst.ty, IrType::NativeSlice(_) | IrType::NativeSliceMut(_)) {
                report(format!(
                    "{position}: NativeSliceValue declares {}, expected a NativeSlice/NativeSliceMut",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: NativeSliceValue's pointer is {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(length)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceValue's length is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceLength(receiver) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "NativeSliceLength",
                report,
            );
            if let Some(ty) = type_of(receiver)
                && !matches!(ty, IrType::NativeSlice(_) | IrType::NativeSliceMut(_))
            {
                report(format!(
                    "{position}: NativeSliceLength reads {}, which is not a NativeSlice/NativeSliceMut",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceLoad { receiver, index } => {
            if let Some(ty) = type_of(receiver)
                && !matches!(ty, IrType::NativeSlice(_) | IrType::NativeSliceMut(_))
            {
                report(format!(
                    "{position}: NativeSliceLoad reads {}, which is not a NativeSlice/NativeSliceMut",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceLoad's index is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceStore {
            receiver,
            index,
            value: _,
        } => {
            expect(inst.ty, IrType::Void, position, "NativeSliceStore", report);
            if let Some(ty) = type_of(receiver)
                && !matches!(ty, IrType::NativeSliceMut(_))
            {
                report(format!(
                    "{position}: NativeSliceStore writes through {}, which is not a NativeSliceMut",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceStore's index is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }

        InstKind::WeakFrom(value) => {
            if let Some(ty) = type_of(value)
                && !matches!(inst.ty, IrType::Weak(id) if module.weak_types.get(id as usize) == Some(&ty))
            {
                report(format!(
                    "{position}: WeakFrom wraps {}, declares {}, expected a Weak of the wrapped type",
                    ty.as_str(),
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::WeakUpgrade(weak) => match type_of(weak) {
            Some(IrType::Weak(id)) => {
                let expected = module
                    .weak_types
                    .get(id as usize)
                    .copied()
                    .and_then(Nullable::of)
                    .map(IrType::Nullable);
                if let Some(expected) = expected
                    && inst.ty != expected
                {
                    report(format!(
                        "{position}: WeakUpgrade declares {}, expected {}",
                        inst.ty.as_str(),
                        expected.as_str()
                    ));
                }
            }
            Some(other) => report(format!(
                "{position}: WeakUpgrade reads through {}, which is not a Weak",
                other.as_str()
            )),
            None => {}
        },
        InstKind::WeakIsAlive(weak) => {
            expect(inst.ty, IrType::Boolean, position, "WeakIsAlive", report);
            if let Some(ty) = type_of(weak)
                && !matches!(ty, IrType::Weak(_))
            {
                report(format!(
                    "{position}: WeakIsAlive reads {}, which is not a Weak",
                    ty.as_str()
                ));
            }
        }
        InstKind::Clone(value) => {
            if let Some(ty) = type_of(value)
                && inst.ty != ty
            {
                report(format!(
                    "{position}: Clone declares {}, expected {} (the receiver's own type)",
                    inst.ty.as_str(),
                    ty.as_str()
                ));
            }
        }

        InstKind::JournalBegin => {
            expect(
                inst.ty,
                IrType::JournalHandle,
                position,
                "JournalBegin",
                report,
            );
        }
        InstKind::JournalRecordSlot { journal, slot } => {
            expect(inst.ty, IrType::Void, position, "JournalRecordSlot", report);
            if let Some(ty) = type_of(journal)
                && ty != IrType::JournalHandle
            {
                report(format!(
                    "{position}: JournalRecordSlot's journal is {}, expected JournalHandle",
                    ty.as_str()
                ));
            }
            if function.slot(*slot).is_none() {
                report(format!(
                    "{position}: JournalRecordSlot names slot {slot:?}, which does not exist"
                ));
            }
        }
        InstKind::JournalRecordField {
            journal,
            object,
            index,
        } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "JournalRecordField",
                report,
            );
            if let Some(ty) = type_of(journal)
                && ty != IrType::JournalHandle
            {
                report(format!(
                    "{position}: JournalRecordField's journal is {}, expected JournalHandle",
                    ty.as_str()
                ));
            }
            if field_type(module, type_of(object), *index).is_none() {
                report(format!(
                    "{position}: JournalRecordField writes field {index} of something that is not an object with it"
                ));
            }
        }
        InstKind::JournalCommit(journal) | InstKind::JournalRollback(journal) => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "JournalCommit/Rollback",
                report,
            );
            if let Some(ty) = type_of(journal)
                && ty != IrType::JournalHandle
            {
                report(format!(
                    "{position}: journal commit/rollback operates on {}, expected JournalHandle",
                    ty.as_str()
                ));
            }
        }
        InstKind::MakeCallable { target, captures } => {
            let IrType::Callable(id) = inst.ty else {
                report(format!(
                    "{position}: MakeCallable returns {}, expected a Callable type",
                    inst.ty.as_str()
                ));
                return;
            };

            let Some(layout) = module.closures.get(id as usize) else {
                report(format!(
                    "{position}: MakeCallable names layout {id}, which does not exist"
                ));
                return;
            };

            if module.function(target).is_none() {
                report(format!(
                    "{position}: MakeCallable targets `{target}`, which is not a module function"
                ));
                return;
            }

            if captures.len() != layout.captures.len() {
                report(format!(
                    "{position}: MakeCallable passes {} capture(s), the layout declares {}",
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
        InstKind::CallCallable { callable, args } => {
            let Some(actual) = type_of(callable) else {
                report(format!("{position}: CallCallable callable is undefined"));
                return;
            };
            let IrType::Callable(id) = actual else {
                report(format!(
                    "{position}: CallCallable receives {}, expected a Callable type",
                    actual.as_str()
                ));
                return;
            };

            let Some(layout) = module.closures.get(id as usize) else {
                report(format!(
                    "{position}: CallCallable names layout {id}, which does not exist"
                ));
                return;
            };

            expect(inst.ty, layout.returns, position, "CallCallable", report);

            if args.len() != layout.params.len() {
                report(format!(
                    "{position}: CallCallable passes {} argument(s), the callable takes {}",
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
        InstKind::ResourceTransfer { source } => {
            if let Some(ty) = type_of(source) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_)) {
                    report(format!(
                        "{position}: ResourceTransfer source is {}, expected an object or contract reference",
                        ty.as_str()
                    ));
                }
                if inst.ty != ty {
                    report(format!(
                        "{position}: ResourceTransfer declares type {} but its source is {}",
                        inst.ty.as_str(),
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: ResourceTransfer source is undefined"));
            }
        }
        InstKind::DependentFrom { base, field_ptr } => {
            if let Some(ty) = type_of(base) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_)) {
                    report(format!(
                        "{position}: DependentFrom base is {}, expected an object or contract reference",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: DependentFrom base is undefined"));
            }
            if let Some(ty) = type_of(field_ptr) {
                if !matches!(ty, IrType::Pointer(_)) {
                    report(format!(
                        "{position}: DependentFrom field_ptr is {}, expected Pointer",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: DependentFrom field_ptr is undefined"));
            }
            if !matches!(inst.ty, IrType::Dependent(_)) {
                report(format!(
                    "{position}: DependentFrom declares {}, expected Dependent",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PinObject { object } => {
            if !matches!(inst.ty, IrType::Pin(_)) {
                report(format!(
                    "{position}: PinObject declares {}, expected Pin",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(object) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_) | IrType::Pin(_)) {
                    report(format!(
                        "{position}: PinObject target is {}, expected an object/contract/Pin reference",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: PinObject target is undefined"));
            }
        }
        InstKind::UnpinObject { object } => {
            expect(inst.ty, IrType::Void, position, "Unpin", report);
            if let Some(ty) = type_of(object) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_) | IrType::Pin(_)) {
                    report(format!(
                        "{position}: UnpinObject target is {}, expected an object/contract/Pin reference",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: UnpinObject target is undefined"));
            }
        }

        InstKind::ArrayLength(operand) => {
            if inst.ty != IrType::Int(IntWidth::U64) {
                report(format!(
                    "{position}: ArrayLength declares {}, expected UInt64",
                    inst.ty.as_str()
                ));
            }
            if !matches!(type_of(operand), Some(IrType::Array(_))) {
                report(format!("{position}: ArrayLength receiver is not an Array"));
            }
        }
        InstKind::ListLength(operand) => {
            if inst.ty != IrType::Int(IntWidth::U64) {
                report(format!(
                    "{position}: ListLength declares {}, expected UInt64",
                    inst.ty.as_str()
                ));
            }
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!("{position}: ListLength receiver is not a List"));
            }
        }

        InstKind::ArrayNew {
            element_id,
            capacity,
        } => {
            if inst.ty != IrType::Array(*element_id) {
                report(format!(
                    "{position}: ArrayNew declares {}, expected Array<{element_id}>",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(capacity)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ArrayNew capacity is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListNew { element_id } => {
            if inst.ty != IrType::List(*element_id) {
                report(format!(
                    "{position}: ListNew declares {}, expected List<{element_id}>",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::ArrayListLoad { receiver, index } => {
            let element = match type_of(receiver).unwrap_or(IrType::Void) {
                IrType::Array(id) => module
                    .array_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                IrType::List(id) => module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                _ => IrType::Void,
            };
            if element != IrType::Void && inst.ty != element {
                report(format!(
                    "{position}: ArrayListLoad declares {}, expected {}",
                    inst.ty.as_str(),
                    element.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ArrayListLoad index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ArrayListStore {
            receiver,
            index,
            value,
        } => {
            expect(inst.ty, IrType::Void, position, "ArrayListStore", report);
            let element = match type_of(receiver).unwrap_or(IrType::Void) {
                IrType::Array(id) => module
                    .array_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                IrType::List(id) => module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                _ => IrType::Void,
            };
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: ArrayListStore value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ArrayListStore index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListAdd { receiver, value } => {
            expect(inst.ty, IrType::Void, position, "ListAdd", report);
            let element = if let IrType::List(id) = type_of(receiver).unwrap_or(IrType::Void) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: ListAdd value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
        }
        InstKind::ListInsert {
            receiver,
            index,
            value,
        } => {
            expect(inst.ty, IrType::Void, position, "ListInsert", report);
            let element = if let IrType::List(id) = type_of(receiver).unwrap_or(IrType::Void) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: ListInsert value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ListInsert index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListRemove { receiver, index } => {
            expect(inst.ty, IrType::Void, position, "ListRemove", report);
            if !matches!(type_of(receiver).unwrap_or(IrType::Void), IrType::List(_)) {
                report(format!("{position}: ListRemove receiver is not a List"));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ListRemove index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListRemoveValue { receiver, value } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "ListRemoveValue",
                report,
            );
            let element = if let Some(IrType::List(id)) = type_of(receiver) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if element == IrType::Void {
                report(format!(
                    "{position}: ListRemoveValue receiver is not a List"
                ));
            }
            if let Some(ty) = type_of(value)
                && ty != element
            {
                report(format!(
                    "{position}: ListRemoveValue value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
        }
        InstKind::ArrayClone { receiver } => {
            let expected = if let Some(IrType::Array(id)) = type_of(receiver) {
                IrType::Array(id)
            } else {
                report(format!("{position}: ArrayClone receiver is not an Array"));
                inst.ty
            };
            if inst.ty != expected {
                report(format!(
                    "{position}: ArrayClone declares {}, expected {}",
                    inst.ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::ListClone { receiver } => {
            let expected = if let Some(IrType::List(id)) = type_of(receiver) {
                IrType::List(id)
            } else {
                report(format!("{position}: ListClone receiver is not a List"));
                inst.ty
            };
            if inst.ty != expected {
                report(format!(
                    "{position}: ListClone declares {}, expected {}",
                    inst.ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::ArraySlice {
            receiver,
            start,
            end,
            step,
        } => {
            let expected = if let Some(IrType::Array(id)) = type_of(receiver) {
                IrType::Array(id)
            } else {
                report(format!("{position}: ArraySlice receiver is not an Array"));
                inst.ty
            };
            if inst.ty != expected {
                report(format!(
                    "{position}: ArraySlice declares {}, expected {}",
                    inst.ty.as_str(),
                    expected.as_str()
                ));
            }
            for (name, op) in [("start", start), ("end", end), ("step", step)] {
                if let Some(ty) = type_of(op)
                    && !matches!(ty, IrType::Int(_))
                {
                    report(format!(
                        "{position}: ArraySlice {name} is {}, expected an integer",
                        ty.as_str()
                    ));
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
        IrType::Pin(id) => field_type(module, Some(module.pin_types[id as usize]), index),
        _ => None,
    }
}

/// Operands an instruction reads.
fn operands_of(kind: &InstKind) -> Vec<Operand> {
    match kind {
        InstKind::ConstInt(_)
        | InstKind::ConstBool(_)
        | InstKind::ConstString(_)
        | InstKind::ConstChar(_) => Vec::new(),
        InstKind::FatalError(message) => vec![*message],
        InstKind::Throw(exception) => vec![*exception],
        InstKind::SetSuppressed {
            exception,
            suppressed,
        } => vec![*exception, *suppressed],
        InstKind::StackTrace(exception) | InstKind::Suppressed(exception) => vec![*exception],
        InstKind::HasPendingException | InstKind::IsCancelled | InstKind::TakePendingException => {
            Vec::new()
        }
        InstKind::Load(_) => Vec::new(),
        InstKind::Store(_, operand) => vec![*operand],
        InstKind::Alloc(_) => Vec::new(),
        InstKind::BuildValue { fields, .. } => fields.clone(),
        InstKind::BuildEnum { fields, .. } => fields.clone(),
        InstKind::Discriminant(operand) => vec![*operand],
        InstKind::CheckedCast { object, .. } => vec![*object],
        InstKind::IsInstance { object, .. } => vec![*object],
        InstKind::Undefined => vec![],
        InstKind::Retype(operand) => vec![*operand],
        InstKind::IntCast(operand) => vec![*operand],
        InstKind::ConstFloat(_, _) | InstKind::ConstDecimal(_) => Vec::new(),
        InstKind::FloatCast(operand)
        | InstKind::IntToFloat(operand)
        | InstKind::FloatToInt(operand)
        | InstKind::IntToDecimal(operand)
        | InstKind::DecimalToInt(operand)
        | InstKind::DecimalToFloat(operand)
        | InstKind::FloatToDecimal(operand) => vec![*operand],
        InstKind::StringGraphemeOffset { string, index } => vec![*string, *index],
        InstKind::GraphemeLenAt { string, offset } => vec![*string, *offset],
        InstKind::GraphemeSlice {
            string,
            offset,
            len,
        } => vec![*string, *offset, *len],
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
        InstKind::CheckedArithmetic { left, right, .. } => vec![*left, *right],
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
        InstKind::PointerFromSlot(_) => Vec::new(),
        InstKind::PointerFromField { object, .. } => vec![*object],
        InstKind::PointerRead(operand)
        | InstKind::PointerCast(operand)
        | InstKind::PointerIsNull(operand) => vec![*operand],
        InstKind::PointerWrite { pointer, value } => vec![*pointer, *value],
        InstKind::PointerOffset { pointer, amount }
        | InstKind::PointerOffsetBytes { pointer, amount } => {
            vec![*pointer, *amount]
        }
        InstKind::WeakFrom(operand)
        | InstKind::WeakUpgrade(operand)
        | InstKind::WeakIsAlive(operand)
        | InstKind::Clone(operand) => vec![*operand],
        InstKind::JournalBegin => Vec::new(),
        InstKind::JournalRecordSlot { journal, .. } => vec![*journal],
        InstKind::JournalRecordField {
            journal, object, ..
        } => vec![*journal, *object],
        InstKind::JournalCommit(journal) | InstKind::JournalRollback(journal) => vec![*journal],
        InstKind::NativeSliceValidate {
            pointer, length, ..
        } => vec![*pointer, *length],
        InstKind::NativeSliceValue { pointer, length } => vec![*pointer, *length],
        InstKind::NativeSliceLength(operand) => vec![*operand],
        InstKind::ArrayLength(operand) | InstKind::ListLength(operand) => vec![*operand],
        InstKind::NativeSliceLoad { receiver, index } => vec![*receiver, *index],
        InstKind::NativeSliceStore {
            receiver,
            index,
            value,
        } => vec![*receiver, *index, *value],
        InstKind::MakeCallable { captures, .. } => captures.clone(),
        InstKind::CallCallable { callable, args } => {
            let mut operands = vec![*callable];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::ResourceTransfer { source } => vec![*source],
        InstKind::DependentFrom { base, field_ptr } => vec![*base, *field_ptr],
        InstKind::PinObject { object } | InstKind::UnpinObject { object } => vec![*object],
        InstKind::ArrayNew { capacity, .. } => vec![*capacity],
        InstKind::ListNew { .. } => Vec::new(),
        InstKind::ArrayListLoad { receiver, index } => vec![*receiver, *index],
        InstKind::ArrayListStore {
            receiver,
            index,
            value,
        } => vec![*receiver, *index, *value],
        InstKind::ListAdd { receiver, value } => vec![*receiver, *value],
        InstKind::ListInsert {
            receiver,
            index,
            value,
        } => vec![*receiver, *index, *value],
        InstKind::ListRemove { receiver, index } => vec![*receiver, *index],
        InstKind::ListRemoveValue { receiver, value } => vec![*receiver, *value],
        InstKind::ArrayClone { receiver } | InstKind::ListClone { receiver } => vec![*receiver],
        InstKind::ArraySlice {
            receiver,
            start,
            end,
            step,
        } => vec![*receiver, *start, *end, *step],
    }
}
