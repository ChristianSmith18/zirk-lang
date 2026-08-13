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

        InstKind::Println(_) => expect(inst.ty, IrType::Void, position, "Println", report),
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

/// Operands an instruction reads.
fn operands_of(kind: &InstKind) -> Vec<Operand> {
    match kind {
        InstKind::ConstInt(_) | InstKind::ConstBool(_) | InstKind::ConstString(_) => Vec::new(),
        InstKind::Load(_) => Vec::new(),
        InstKind::Store(_, operand) => vec![*operand],
        InstKind::Unary { operand, .. } => vec![*operand],
        InstKind::Binary { left, right, .. } => vec![*left, *right],
        InstKind::Call { args, .. } => args.clone(),
        InstKind::Println(operand) => vec![*operand],
    }
}
