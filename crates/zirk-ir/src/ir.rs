//! Data structures of the intermediate representation.
//!
//! The form is decision D2 of the Phase 1 design: **typed three-address code
//! over basic blocks**, with local variables as slots and no SSA of our own.
//!
//! A tree would have been enough for this subset, but bringing loops in
//! Phase 2 would force redoing the representation, and `ZIRK_COMPILER_SPEC.md`
//! section 4 requires preserving enough information for escape analysis,
//! devirtualization and vectorization — all flow analyses, which are awkward
//! over a tree.
//!
//! Promoting slots to registers is delegated to the backend. Our own SSA does
//! not pay off until we have our own optimizations.

use zirk_diagnostics::Span;

/// A type of the IR.
///
/// Deliberately independent of `zirk_sema::Type`: the IR is the boundary that
/// gets distributed inside a `.zpkg`, and it must not move every time the
/// frontend's type representation changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    Void,
    Int32,
    Boolean,
    /// Opaque handle to a string. Its layout belongs to the runtime
    /// (`docs/decisions/ADR-005-representacion-string.md`).
    String,
    /// A closure, identified by its layout in the module.
    ///
    /// Each lambda has its own type rather than sharing one per signature: a
    /// closure cannot escape in this phase, so at every use site the type is
    /// statically known. Decision D10.
    Closure(u32),
    /// `T?`: a value that may be absent.
    ///
    /// Represented uniformly as a present flag next to the value, rather than
    /// as a null pointer for `String` and something else for the scalars. A
    /// per-type trick would be smaller for `String` and would need a separate
    /// rule for every type added later; one shape needs none.
    Nullable(Nullable),
}

/// The types that have a nullable form.
///
/// Kept apart from [`IrType`] so a nullable type stays `Copy`: wrapping an
/// `IrType` would need a box, and `T??` does not exist, so one level is all
/// there is to express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nullable {
    Int32,
    Boolean,
    String,
}

impl Nullable {
    pub const fn inner(self) -> IrType {
        match self {
            Nullable::Int32 => IrType::Int32,
            Nullable::Boolean => IrType::Boolean,
            Nullable::String => IrType::String,
        }
    }

    /// The nullable form of a type, if it has one.
    pub const fn of(ty: IrType) -> Option<Self> {
        Some(match ty {
            IrType::Int32 => Nullable::Int32,
            IrType::Boolean => Nullable::Boolean,
            IrType::String => Nullable::String,
            _ => return None,
        })
    }
}

impl IrType {
    pub const fn as_str(self) -> &'static str {
        match self {
            IrType::Void => "Void",
            IrType::Int32 => "Int32",
            IrType::Boolean => "Boolean",
            IrType::String => "String",
            IrType::Closure(_) => "closure",
            IrType::Nullable(n) => match n {
                Nullable::Int32 => "Int32?",
                Nullable::Boolean => "Boolean?",
                Nullable::String => "String?",
            },
        }
    }

    /// The type inside a nullable one, or the type itself.
    pub const fn unwrapped(self) -> Self {
        match self {
            IrType::Nullable(base) => base.inner(),
            other => other,
        }
    }

    /// Whether values of this type need storage the runtime must provide.
    ///
    /// This is the abstraction point required by
    /// `docs/decisions/ADR-003-memoria.md`: the IR states **that** something
    /// needs storage, never **how** it is obtained or released. No instruction
    /// names malloc, reference counting or garbage collection.
    pub const fn needs_allocation(self) -> bool {
        matches!(self, IrType::String)
    }
}

/// Identifier of a basic block within a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

/// Identifier of a local slot within a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub u32);

/// Identifier of a value produced by an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub u32);

/// Identifier of a string literal in the module table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringId(pub u32);

/// A compiled module: everything that came from one source file.
#[derive(Debug, Clone, Default)]
pub struct Module {
    pub functions: Vec<Function>,
    /// String literals, deduplicated. The backend materializes them as
    /// constants and the runtime turns them into `String` values.
    pub strings: Vec<String>,
    /// Closure layouts, indexed by the id [`IrType::Closure`] carries.
    pub closures: Vec<ClosureLayout>,
}

/// What one closure value holds and what its lifted function expects.
///
/// The captures live inside the value, so the lifted function takes them as
/// its leading parameters: nothing is allocated and nothing is dereferenced.
/// Decision D10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureLayout {
    /// The module function the lambda body was lifted into.
    pub function: String,
    pub captures: Vec<IrType>,
    pub params: Vec<IrType>,
    pub returns: IrType,
}

impl Module {
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Records a literal, reusing it if it already exists.
    pub fn intern_string(&mut self, value: &str) -> StringId {
        if let Some(index) = self.strings.iter().position(|s| s == value) {
            return StringId(index as u32);
        }
        self.strings.push(value.to_string());
        StringId(self.strings.len() as u32 - 1)
    }
}

/// A function in IR form.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<SlotId>,
    pub return_type: IrType,
    /// Local storage: parameters and variables. Reading and writing them goes
    /// through `Load` and `Store`.
    pub slots: Vec<Slot>,
    pub blocks: Vec<Block>,
    /// The block execution starts at.
    pub entry: BlockId,
    pub span: Span,
}

impl Function {
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn slot(&self, id: SlotId) -> Option<&Slot> {
        self.slots.get(id.0 as usize)
    }
}

/// Local storage for a variable or parameter.
#[derive(Debug, Clone)]
pub struct Slot {
    pub name: String,
    pub ty: IrType,
    pub span: Span,
}

/// A basic block: a straight-line sequence ending in exactly one terminator.
#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    /// `None` only while the block is under construction. Well-formed IR has a
    /// terminator on every block, and the verifier enforces it.
    pub terminator: Option<Terminator>,
}

impl Block {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            instructions: Vec::new(),
            terminator: None,
        }
    }
}

/// An instruction that may produce a value.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// Value produced. `None` for instructions executed for their effect.
    pub result: Option<ValueId>,
    pub kind: InstKind,
    pub ty: IrType,
    /// Source location that produced it.
    ///
    /// Required by `ZIRK_COMPILER_SPEC.md` section 11: without it the faithful
    /// mapping back to `.zrk` that the debugger needs is impossible.
    pub span: Span,
}

impl Instruction {
    /// Whether the instruction requires storage from the runtime.
    ///
    /// See [`IrType::needs_allocation`] for why this is expressed abstractly.
    pub fn allocates(&self) -> bool {
        matches!(self.kind, InstKind::ConstString(_) | InstKind::ToString(_))
            && self.ty.needs_allocation()
    }
}

/// What an instruction does.
#[derive(Debug, Clone, PartialEq)]
pub enum InstKind {
    ConstInt(i32),
    ConstBool(bool),
    /// Materializes a string literal. This is the only allocating operation of
    /// the subset, and it is expressed without naming a memory strategy.
    ConstString(StringId),

    /// Reads a slot.
    Load(SlotId),
    /// Writes a slot.
    Store(SlotId, Operand),

    Unary {
        op: UnaryOp,
        operand: Operand,
    },
    Binary {
        op: BinaryOp,
        left: Operand,
        right: Operand,
    },

    Call {
        callee: String,
        args: Vec<Operand>,
    },
    /// Converts a value into a `String`.
    ///
    /// `ZIRK_STDLIB_SPEC.md` section 3 states that every printable value goes
    /// through `to_string(): String`. Making the conversion explicit in the IR
    /// is what keeps `Println` receiving a `String` always, and it is the shape
    /// that becomes a real trait call once traits exist in Phase 3.
    ToString(Operand),
    /// `stdout.println`, an intrinsic while there is no stdlib (design D4).
    ///
    /// Its operand is **always** a `String`: the lowering inserts a `ToString`
    /// when it is not, and the verifier enforces it.
    Println(Operand),

    /// The absent value of a nullable type.
    NullValue(Nullable),
    /// Widens a value into its nullable form.
    ///
    /// `T` is accepted where `T?` is expected, and this is that widening made
    /// explicit: the IR never has an implicit representation change.
    Wrap { base: Nullable, value: Operand },
    /// Whether a nullable value is absent.
    IsNull(Operand),
    /// The value inside a nullable one.
    ///
    /// Only emitted on a path where an [`InstKind::IsNull`] already proved it
    /// present, which is what `??` establishes before using it.
    Unwrap(Operand),

    /// Builds a closure value from its captures.
    MakeClosure { id: u32, captures: Vec<Operand> },
    /// Calls a closure value.
    ///
    /// The captures travel inside the operand, so the call passes them ahead of
    /// the arguments the caller wrote.
    CallClosure {
        id: u32,
        callee: Operand,
        args: Vec<Operand>,
    },
}

/// An input to an instruction.
///
/// Only values produced by earlier instructions: constants have their own
/// instruction so every operand carries a type and a location uniformly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operand(pub ValueId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

impl BinaryOp {
    /// Type produced by the operator given its operands.
    pub const fn result_type(self, operand: IrType) -> IrType {
        use BinaryOp::*;
        match self {
            Add | Sub | Mul | Div | Rem => operand,
            Eq | NotEq | Lt | LtEq | Gt | GtEq | And | Or => IrType::Boolean,
        }
    }
}

/// How a basic block ends.
#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Return(Option<Operand>),
    Jump(BlockId),
    Branch {
        condition: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
}

impl Terminator {
    /// Blocks this terminator can transfer control to.
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Return(_) => Vec::new(),
            Terminator::Jump(target) => vec![*target],
            Terminator::Branch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_string_needs_allocation_in_this_subset() {
        assert!(IrType::String.needs_allocation());
        assert!(!IrType::Int32.needs_allocation());
        assert!(!IrType::Boolean.needs_allocation());
        assert!(!IrType::Void.needs_allocation());
    }

    #[test]
    fn arithmetic_preserves_the_operand_type() {
        assert_eq!(BinaryOp::Add.result_type(IrType::Int32), IrType::Int32);
    }

    #[test]
    fn comparison_and_logic_yield_boolean() {
        for op in [BinaryOp::Eq, BinaryOp::Lt, BinaryOp::And, BinaryOp::Or] {
            assert_eq!(op.result_type(IrType::Int32), IrType::Boolean);
        }
    }

    #[test]
    fn a_return_has_no_successors() {
        assert!(Terminator::Return(None).successors().is_empty());
    }

    #[test]
    fn a_branch_has_two_successors() {
        let t = Terminator::Branch {
            condition: Operand(ValueId(0)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        assert_eq!(t.successors(), vec![BlockId(1), BlockId(2)]);
    }

    #[test]
    fn interning_a_literal_reuses_it() {
        let mut module = Module::default();
        let first = module.intern_string("hola");
        let second = module.intern_string("hola");
        let other = module.intern_string("chau");

        assert_eq!(first, second);
        assert_ne!(first, other);
        assert_eq!(module.strings.len(), 2);
    }
}
