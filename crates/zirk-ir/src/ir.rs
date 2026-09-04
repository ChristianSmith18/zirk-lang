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

/// Every integer width the IR represents (roadmap Phase 3b) — its own type,
/// not `zirk_sema::IntWidth`, for the same reason `IrType` itself stays
/// independent of `zirk_sema::Type`: this is the boundary a `.zpkg`
/// distributes, and it must not move every time the frontend's own
/// representation changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
}

impl IntWidth {
    pub const fn bits(self) -> u32 {
        use IntWidth::*;
        match self {
            I8 | U8 => 8,
            I16 | U16 => 16,
            I32 | U32 => 32,
            I64 | U64 => 64,
            I128 | U128 => 128,
        }
    }

    pub const fn signed(self) -> bool {
        matches!(
            self,
            IntWidth::I8 | IntWidth::I16 | IntWidth::I32 | IntWidth::I64 | IntWidth::I128
        )
    }

    pub const fn as_str(self) -> &'static str {
        use IntWidth::*;
        match self {
            I8 => "Int8",
            I16 => "Int16",
            I32 => "Int32",
            I64 => "Int64",
            I128 => "Int128",
            U8 => "UInt8",
            U16 => "UInt16",
            U32 => "UInt32",
            U64 => "UInt64",
            U128 => "UInt128",
        }
    }
}

/// Every float width the IR represents (roadmap Phase 3b) — its own type,
/// independent of `zirk_sema::FloatWidth`, for the same reason [`IntWidth`]
/// above is independent of `zirk_sema::IntWidth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatWidth {
    F16,
    F32,
    F64,
    F128,
}

impl FloatWidth {
    pub const fn bits(self) -> u32 {
        use FloatWidth::*;
        match self {
            F16 => 16,
            F32 => 32,
            F64 => 64,
            F128 => 128,
        }
    }

    pub const fn as_str(self) -> &'static str {
        use FloatWidth::*;
        match self {
            F16 => "Float16",
            F32 => "Float32",
            F64 => "Float64",
            F128 => "Float128",
        }
    }
}

/// A type of the IR.
///
/// Deliberately independent of `zirk_sema::Type`: the IR is the boundary that
/// gets distributed inside a `.zpkg`, and it must not move every time the
/// frontend's type representation changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    Void,
    /// The bottom type (roadmap Phase 4a) — no instruction ever produces a
    /// real value of it; only `InstKind::FatalError` declares it, and the
    /// block it is in always ends in `Terminator::Unreachable` right after,
    /// never a normal `Store`/`Jump`. It exists in the IR's own type table
    /// purely so a diverging branch of `if`/ternary can be recognized and
    /// skipped at lowering, mirroring how the checker's `Type::unify`
    /// already lets it disappear at a branch join instead of poisoning the
    /// join's type.
    Never,
    /// `Int8`…`UInt128` (roadmap Phase 3b) — width plus signedness as data,
    /// not one variant per width: LLVM already models any integer width
    /// natively, so nothing downstream needs a different *shape* per width,
    /// only the right one substituted in.
    Int(IntWidth),
    /// `Float16`…`Float128` (roadmap Phase 3b) — same reasoning as `Int`
    /// above, minus the signedness: LLVM's `FloatType` is already
    /// parameterized by width alone.
    Float(FloatWidth),
    Boolean,
    /// Opaque handle to a string. Its layout belongs to the runtime
    /// (`docs/decisions/ADR-005-representacion-string.md`).
    String,
    /// Exactly one Unicode grapheme (roadmap Phase 3b) — the same opaque
    /// runtime handle as `String` (ADR-014), but a distinct static type: no
    /// mutation methods, and no identity (`is` is rejected, unlike `String`).
    Char,
    /// A closure, identified by its layout in the module.
    ///
    /// A named function reference and a capture-less lambda share one
    /// canonical, capture-less layout per signature — `{function pointer}`,
    /// nothing else (roadmap Phase 4d, design D12) — so any two
    /// same-shaped values of that kind carry the same id. A *capturing*
    /// lambda literal keeps a layout of its own instead: its captures are
    /// part of its representation (decision D10), and a closure escaping
    /// its creating frame (roadmap Phase 4d) is already sound at this
    /// representation level regardless — nothing here holds a pointer back
    /// into the frame that built it, only the captured values themselves.
    ///
    /// This is the legacy inline-closure representation; the active
    /// lowering path uses [`IrType::Callable`] instead.
    Closure(u32),
    /// A boxed callable value: a two-word `{ function pointer, capture block
    /// pointer }` pair (roadmap Phase 4d, `phase-4d-callables`).
    ///
    /// Like [`IrType::Closure`], it is identified by the same layout id in
    /// `module.closures`, but its runtime representation is always a heap-
    /// allocated capture block plus the function pointer that knows how to
    /// use it.
    Callable(u32),
    /// A reference to an object, identified by its layout in the module.
    ///
    /// It is a reference and not a value: an object has identity, and identity
    /// is the address. Two bindings holding the same object hold the same
    /// address, which is what `is` compares.
    Object(u32),
    /// A value reached through a contract rather than through its own class.
    ///
    /// Represented as the object's address, exactly like a class reference:
    /// which class is behind it is what a contract exists not to say, and the
    /// object already carries its own descriptor to answer that at the call.
    Contract(u32),
    /// A record or value class, identified by its layout in the module.
    ///
    /// A value and not a reference: neither has identity (`is` is rejected
    /// for both, task 8.5/8.6), so nothing observes whether two equal ones
    /// share storage. Represented inline wherever it is used — a slot, a
    /// parameter, a field of a containing object — with no allocation and no
    /// descriptor header (`ZIRK_RUNTIME_SPEC.md` §9, ADR-003; roadmap task
    /// 11.5). `Base::Class` still carries both a `Value` and an ordinary
    /// `Object`'s id — `ClassType.kind` says which is which; the id spaces
    /// are the same table, so nothing here needs its own.
    Value(u32),
    /// An algebraic enum — one with at least one variant carrying associated
    /// data — identified by its layout in the module.
    ///
    /// A value, the same as a record: discriminant plus payload, inline
    /// wherever it is used, no allocation (roadmap task 11.3). A *traditional*
    /// enum, none of whose variants carry data, stays `Int32` exactly as
    /// before — this variant exists only once a payload does.
    Enum(u32),
    /// `T?`: a value that may be absent.
    ///
    /// Represented uniformly as a present flag next to the value, rather than
    /// as a null pointer for `String` and something else for the scalars. A
    /// per-type trick would be smaller for `String` and would need a separate
    /// rule for every type added later; one shape needs none.
    Nullable(Nullable),
    /// `Pointer<T>` (roadmap Phase 4e, design D1/D8), identified by the id of
    /// its pointee type in the module's own `pointer_types` table — kept as
    /// an id rather than `Box<IrType>` so `IrType` stays `Copy`, the same
    /// reason `Closure`/`Object`/`Value`/`Enum` are ids into a module table
    /// instead of holding their shape inline.
    Pointer(u32),
    /// `Weak<T>` (roadmap Phase 4e, `fase-4e-weak`, design D1), identified by
    /// the id of its referent type in the module's own `weak_types` table —
    /// kept as an id for the same reason `Pointer` is.
    ///
    /// A `Weak<T>` value is itself a managed reference: a pointer to a
    /// collector-tracked WeakCell allocation (design D1/D2), unlike
    /// `Pointer<T>`'s own raw, unmanaged pointer.
    Weak(u32),
    /// The `*mut Journal` handle an `unsafe { ... }` block's own undo log is
    /// referred to by (roadmap Phase 4e, `fase-4e-unsafe-journal`, design
    /// D1/D4) — an ordinary `zirk-runtime`-internal allocation, never a
    /// collector-tracked one (same category as `Weak<T>`'s WeakCell context
    /// or `fase-4e-clone`'s memoization table): carries no type parameter,
    /// unlike `Pointer`/`Weak`, since a journal is opaque to the type it
    /// guards — it only ever stores and restores raw bytes.
    JournalHandle,
    /// `NativeSlice<T>` (roadmap Phase 4e, `fase-4e-native-slice`, design
    /// D1/D2), identified by the id of its element type in the module's own
    /// `native_slice_types` table — kept as an id for the same reason
    /// `Pointer`/`Weak` are.
    ///
    /// Runtime representation is a plain `(pointer, length)` two-word pair
    /// (design D2) — not a collector-tracked allocation, not itself an
    /// "object" with identity: a bounded view over memory the program
    /// already owns. `IrType::is_managed_reference`'s own default `_ =>
    /// false` arm already covers this correctly, the same way it does for
    /// `Pointer`.
    NativeSlice(u32),
    /// `NativeSliceMut<T>` (roadmap Phase 4e, `fase-4e-native-slice`,
    /// design D1/D2) — the read/write counterpart of
    /// [`IrType::NativeSlice`], identified into the module's own
    /// `native_slice_mut_types` table. Same `(pointer, length)`
    /// representation; the distinction is purely in what the checker
    /// permits at each use, not in runtime shape.
    NativeSliceMut(u32),
    /// `Dependent<T>` (roadmap Phase 4e, `phase-4e-memory`, design D1):
    /// a reference whose lifetime is tied to the allocation that contains
    /// the value it refers to. Surface-only: represented as an opaque
    /// pointer until the two-word form is wired through codegen.
    Dependent(u32),
    /// `Pin<T>` (roadmap Phase 4e, `phase-4e-memory`, design D1): keeps an
    /// object's address stable for native interop. Surface-only: represented
    /// as an opaque pointer.
    Pin(u32),
}

/// The types that have a nullable form.
///
/// Kept apart from [`IrType`] so a nullable type stays `Copy`: wrapping an
/// `IrType` would need a box, and `T??` does not exist, so one level is all
/// there is to express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nullable {
    Int(IntWidth),
    Float(FloatWidth),
    Boolean,
    String,
    Char,
    /// A reference that may be absent, by layout id.
    ///
    /// It carries the flag like every other nullable rather than reusing the
    /// null address: `T??` does not exist, but a rule that only works for
    /// pointers is one more rule, and one shape needs none.
    Object(u32),
    Contract(u32),
    Value(u32),
    Enum(u32),
}

impl Nullable {
    pub const fn inner(self) -> IrType {
        match self {
            Nullable::Int(width) => IrType::Int(width),
            Nullable::Float(width) => IrType::Float(width),
            Nullable::Boolean => IrType::Boolean,
            Nullable::String => IrType::String,
            Nullable::Char => IrType::Char,
            Nullable::Object(id) => IrType::Object(id),
            Nullable::Contract(id) => IrType::Contract(id),
            Nullable::Value(id) => IrType::Value(id),
            Nullable::Enum(id) => IrType::Enum(id),
        }
    }

    /// The nullable form of a type, if it has one.
    pub const fn of(ty: IrType) -> Option<Self> {
        Some(match ty {
            IrType::Int(width) => Nullable::Int(width),
            IrType::Float(width) => Nullable::Float(width),
            IrType::Boolean => Nullable::Boolean,
            IrType::String => Nullable::String,
            IrType::Char => Nullable::Char,
            IrType::Object(id) => Nullable::Object(id),
            IrType::Contract(id) => Nullable::Contract(id),
            IrType::Value(id) => Nullable::Value(id),
            IrType::Enum(id) => Nullable::Enum(id),
            _ => return None,
        })
    }
}

impl IrType {
    pub const fn as_str(self) -> &'static str {
        match self {
            IrType::Void => "Void",
            IrType::Never => "Never",
            IrType::Int(width) => width.as_str(),
            IrType::Float(width) => width.as_str(),
            IrType::Boolean => "Boolean",
            IrType::String => "String",
            IrType::Char => "Char",
            IrType::Closure(_) => "closure",
            IrType::Callable(_) => "Callable",
            IrType::Object(_) => "object",
            IrType::Contract(_) => "contract",
            IrType::Value(_) => "value",
            IrType::Enum(_) => "enum",
            IrType::Nullable(n) => match n {
                // A width's own name plus `?` is not `const`-friendly (no
                // `const` string concatenation), so nullable integers share
                // one spelling regardless of which width — a diagnostic
                // that needs the exact width reads `inner()` instead.
                Nullable::Int(_) => "Int?",
                Nullable::Float(_) => "Float?",
                Nullable::Boolean => "Boolean?",
                Nullable::String => "String?",
                Nullable::Char => "Char?",
                Nullable::Object(_) => "object?",
                Nullable::Contract(_) => "contract?",
                Nullable::Value(_) => "value?",
                Nullable::Enum(_) => "enum?",
            },
            IrType::Pointer(_) => "Pointer",
            IrType::Weak(_) => "Weak",
            IrType::JournalHandle => "JournalHandle",
            IrType::NativeSlice(_) => "NativeSlice",
            IrType::NativeSliceMut(_) => "NativeSliceMut",
            IrType::Dependent(_) => "Dependent",
            IrType::Pin(_) => "Pin",
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
        // `Char` shares `String`'s opaque runtime representation (ADR-014).
        matches!(self, IrType::String | IrType::Char)
    }

    /// Whether a value of this type carries (directly, or nested inside a
    /// `Value`/`Enum`/`Closure`) at least one collector-managed heap
    /// reference (`fase-4e-colector-mark-sweep`, design D2/D4).
    ///
    /// `String`/`Char` are now collector-managed opaque handles as well, so
    /// they are included here. A slot/instruction result of this kind is what
    /// both the shadow-stack root descriptor (D2) and the D4 synthetic-slot
    /// spill key off of. A `Value`/`Enum`/`Closure` layout can never nest
    /// itself (that would be an infinitely sized type, already rejected
    /// upstream), so this recursion always terminates.
    pub fn is_managed_reference(self, module: &Module) -> bool {
        match self {
            // A `Weak<T>` value is a pointer to a collector-tracked WeakCell
            // (`fase-4e-weak`, design D1/D2) — an ordinary managed reference
            // as far as root enumeration and the GC field-offset table are
            // concerned; the collector's own mark pass is what stops short
            // of tracing *through* it as a strong edge (design D2), not
            // anything decided here.
            IrType::Object(_)
            | IrType::Contract(_)
            | IrType::Weak(_)
            | IrType::Dependent(_)
            | IrType::Pin(_)
            | IrType::String
            | IrType::Char => true,
            IrType::Nullable(n) => n.inner().is_managed_reference(module),
            IrType::Value(id) => module.values[id as usize]
                .fields
                .iter()
                .any(|f| f.ty.is_managed_reference(module)),
            IrType::Enum(id) => module.enums[id as usize]
                .fields
                .iter()
                .any(|f| f.ty.is_managed_reference(module)),
            // A boxed `Callable` carries a pointer to a GC-allocated capture
            // block (even when that block is empty, the pointer is null and
            // `mark_object` simply returns).  Keeping the callable value rooted
            // keeps the capture block — and therefore its captured managed
            // references — alive across collections.
            IrType::Closure(id) => module.closures[id as usize]
                .captures
                .iter()
                .any(|c| c.is_managed_reference(module)),
            IrType::Callable(_) => true,
            _ => false,
        }
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
    /// Object layouts, indexed by the id [`IrType::Object`] carries.
    pub objects: Vec<ObjectLayout>,
    /// Record and value class layouts, indexed by the id [`IrType::Value`]
    /// carries — its own table because they share no shape with
    /// [`ObjectLayout`]: no header, no dispatch table, no allocation
    /// (roadmap task 11.5).
    pub values: Vec<ValueLayout>,
    /// Algebraic enum layouts, indexed by the id [`IrType::Enum`] carries —
    /// `checked.enums`' own id space, indexed the same way `objects` is
    /// indexed by `checked.classes` (roadmap task 11.3). A traditional enum,
    /// none of whose variants carry data, has no entry here: it stays
    /// `Int32` and is never addressed through this table.
    pub enums: Vec<EnumLayout>,
    /// Interned `Pointer<T>` pointee types, indexed by the id
    /// [`IrType::Pointer`] carries (roadmap Phase 4e, design D1).
    pub pointer_types: Vec<IrType>,
    /// Interned `Weak<T>` referent types, indexed by the id
    /// [`IrType::Weak`] carries (roadmap Phase 4e, `fase-4e-weak`, design
    /// D1).
    pub weak_types: Vec<IrType>,
    /// Interned `NativeSlice<T>` element types, indexed by the id
    /// [`IrType::NativeSlice`] carries (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1).
    pub native_slice_types: Vec<IrType>,
    /// Interned `NativeSliceMut<T>` element types, indexed by the id
    /// [`IrType::NativeSliceMut`] carries (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D1).
    pub native_slice_mut_types: Vec<IrType>,
    /// Interned `Dependent<T>` element types, indexed by the id
    /// [`IrType::Dependent`] carries (roadmap Phase 4e, `phase-4e-memory`).
    pub dependent_types: Vec<IrType>,
    /// Interned `Pin<T>` referent types, indexed by the id [`IrType::Pin`]
    /// carries (roadmap Phase 4e, `phase-4e-memory`).
    pub pin_types: Vec<IrType>,
    /// `extern "C" fn` declarations (roadmap Phase 4e, design D7,
    /// `ADR-015`) — lowered to an LLVM `declare`, never a `define`: there is
    /// no Zirk-authored body.
    pub externs: Vec<ExternFn>,
}

/// One `extern "C" fn` declaration (roadmap Phase 4e, design D7).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternFn {
    pub name: String,
    pub params: Vec<IrType>,
    pub return_type: IrType,
}

/// What one object holds in memory.
///
/// ```text
///    object  =  [ type descriptor | next (mark bit) | size | field₁ | field₂ | … ]
/// ```
///
/// The header grew from one word to three in `fase-4e-colector-mark-sweep`
/// (ADR-003's "Cierre de la decisión", design D1): the dispatch descriptor
/// (unchanged — every existing reader keeps reading it unmasked), an
/// intrusive `next`-allocation pointer the collector's sweep walks (mark bit
/// hidden in its low bit), and the allocation's own byte size (so sweep can
/// `dealloc` correctly). Nothing here fixes the header's word count as a
/// literal — codegen (`OBJECT_HEADER_FIELDS`, `crates/zirk-codegen-llvm`) is
/// the single place that does, so growing it is a codegen-only change; this
/// layout only ever describes fields *after* the header, whatever it is.
///
/// Inherited fields come before a class's own, in the order the hierarchy
/// declares them, so a subclass's prefix matches its superclass's and reaching
/// an inherited field is the same offset whoever is looking.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectLayout {
    /// The name of the class, which the descriptor is emitted under.
    pub name: String,
    pub fields: Vec<ObjectField>,
    /// The contracts this layout satisfies, and the table for each.
    ///
    /// One table per contract, because a class satisfies several and each
    /// would want its own indices. The descriptor keeps the list so a call
    /// through a contract can find its table (D3).
    pub contracts: Vec<ContractTable>,
    /// The method table, one symbol per index.
    ///
    /// A subclass's table starts with its base's entries, in the same order,
    /// with an override replacing the entry it overrides. That is what keeps a
    /// method's index from moving as the hierarchy grows, and what makes an
    /// indirect call one load and one jump. Decision D3.
    pub methods: Vec<String>,
    /// This class's own id, plus every base's, transitively — what a
    /// checked cast (`InstKind::CheckedCast`) searches to confirm a runtime
    /// type is the target or one of its ancestors (roadmap task 11.6).
    /// `checked.classes`' own id space, the same one `IrType::Object`
    /// carries.
    pub ancestors: Vec<u32>,
}

impl ObjectLayout {
    /// The position of a field in the object, header excluded.
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }

    pub fn field(&self, name: &str) -> Option<&ObjectField> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// The dispatch table of one contract, as one class satisfies it.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractTable {
    /// Which contract this table answers for.
    pub contract: u32,
    /// One symbol per contract method, in the contract's own order.
    pub methods: Vec<String>,
}

/// One field inside an object layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectField {
    pub name: String,
    pub ty: IrType,
}

/// What one record or value class holds — its fields, in declaration order,
/// and nothing else: no header, no dispatch table (roadmap task 11.5).
#[derive(Debug, Clone, PartialEq)]
pub struct ValueLayout {
    pub name: String,
    pub fields: Vec<ObjectField>,
}

impl ValueLayout {
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }
}

/// What one algebraic enum holds: a discriminant plus a flattened payload —
/// every variant's associated fields, concatenated in variant-declaration
/// order, each with its own dedicated slot (roadmap task 11.3).
///
/// Not a byte-level union: a variant's fields never share storage with
/// another's. Simpler and correct — reading a field always reads what was
/// written there, never another variant's differently-typed data
/// reinterpreted — at the cost of a layout sized for every variant's fields
/// at once rather than only the largest one. The set of variants in this
/// phase is small, so the trade is not tight.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumLayout {
    pub name: String,
    pub fields: Vec<ObjectField>,
    /// Per variant, by discriminant index: the indices into `fields` it
    /// owns, in its own declared order. A traditional variant (no data) or
    /// one with none owns an empty slice.
    pub variants: Vec<Vec<u32>>,
}

/// What one closure value holds and what its lifted function expects.
///
/// The captures live inside the value, so the lifted function takes them as
/// its leading parameters: nothing is allocated and nothing is dereferenced.
/// Decision D10.
///
/// Carries no target function of its own (roadmap Phase 4d, design D12):
/// many differently-targeted values (a lambda's own lifted body, or an
/// altogether different named function) can share one id's shape here when
/// they have the same captures — the target a given value calls is
/// [`InstKind::MakeClosure`]'s own `target` field instead, one per value
/// built, not one per shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureLayout {
    pub captures: Vec<IrType>,
    pub params: Vec<IrType>,
    pub returns: IrType,
}

impl Module {
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// The layout id of a class, by name — what `objects[id]` should be
    /// indexed by instead of a hardcoded position, since the compiler-known
    /// classes (roadmap Phase 4b: `Error`/`Throwable`/`RuntimeError`/
    /// `StackTrace`) are registered ahead of anything a program declares.
    pub fn object_id(&self, name: &str) -> Option<u32> {
        self.objects
            .iter()
            .position(|o| o.name == name)
            .map(|i| i as u32)
    }

    /// Records a literal, reusing it if it already exists.
    pub fn intern_string(&mut self, value: &str) -> StringId {
        if let Some(index) = self.strings.iter().position(|s| s == value) {
            return StringId(index as u32);
        }
        self.strings.push(value.to_string());
        StringId(self.strings.len() as u32 - 1)
    }

    /// Interns a `Pointer<T>` pointee type, returning the id
    /// [`IrType::Pointer`] carries.
    pub fn intern_pointer_type(&mut self, pointee: IrType) -> u32 {
        if let Some(index) = self.pointer_types.iter().position(|&t| t == pointee) {
            return index as u32;
        }
        self.pointer_types.push(pointee);
        (self.pointer_types.len() - 1) as u32
    }

    /// Interns a `Weak<T>` referent type, returning the id
    /// [`IrType::Weak`] carries.
    pub fn intern_weak_type(&mut self, referent: IrType) -> u32 {
        if let Some(index) = self.weak_types.iter().position(|&t| t == referent) {
            return index as u32;
        }
        self.weak_types.push(referent);
        (self.weak_types.len() - 1) as u32
    }

    /// Interns a `NativeSlice<T>` element type, returning the id
    /// [`IrType::NativeSlice`] carries.
    pub fn intern_native_slice_type(&mut self, element: IrType) -> u32 {
        if let Some(index) = self.native_slice_types.iter().position(|&t| t == element) {
            return index as u32;
        }
        self.native_slice_types.push(element);
        (self.native_slice_types.len() - 1) as u32
    }

    /// Interns a `NativeSliceMut<T>` element type, returning the id
    /// [`IrType::NativeSliceMut`] carries.
    pub fn intern_native_slice_mut_type(&mut self, element: IrType) -> u32 {
        if let Some(index) = self
            .native_slice_mut_types
            .iter()
            .position(|&t| t == element)
        {
            return index as u32;
        }
        self.native_slice_mut_types.push(element);
        (self.native_slice_mut_types.len() - 1) as u32
    }

    /// Interns a `Dependent<T>` element type, returning the id
    /// [`IrType::Dependent`] carries.
    pub fn intern_dependent_type(&mut self, element: IrType) -> u32 {
        if let Some(index) = self.dependent_types.iter().position(|&t| t == element) {
            return index as u32;
        }
        self.dependent_types.push(element);
        (self.dependent_types.len() - 1) as u32
    }

    /// Interns a `Pin<T>` referent type, returning the id [`IrType::Pin`]
    /// carries.
    pub fn intern_pin_type(&mut self, referent: IrType) -> u32 {
        if let Some(index) = self.pin_types.iter().position(|&t| t == referent) {
            return index as u32;
        }
        self.pin_types.push(referent);
        (self.pin_types.len() - 1) as u32
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
    /// The static shadow-stack root descriptor (`fase-4e-colector-mark-sweep`,
    /// design D2): every slot — named or D4's synthetic ones — whose type is
    /// [`IrType::is_managed_reference`]. Codegen turns this into the actual
    /// root addresses pushed at function entry and popped before every
    /// `Terminator::Return`.
    pub gc_roots: Vec<SlotId>,
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
        matches!(
            self.kind,
            InstKind::Alloc(_) | InstKind::Concat { .. } | InstKind::Repeat { .. }
        ) || (matches!(self.kind, InstKind::ConstString(_) | InstKind::ToString(_))
            && self.ty.needs_allocation())
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
    /// Materializes a character literal (roadmap Phase 3b) — a separate
    /// instruction from `ConstString`, even though both build from the same
    /// module string table through the same runtime constructor (ADR-014),
    /// so the verifier can keep telling a `Char` value apart from a `String`
    /// one by the instruction that produced it, not only by `inst.ty`.
    ConstChar(StringId),
    /// `fatalError(message)` (roadmap Phase 4a, `ZIRK_LANGUAGE_SPEC.md`
    /// section 9): reports `message` and terminates the process — the same
    /// controlled-abort pattern every checked arithmetic/cast operation has
    /// used since Phase 1, generalized to an arbitrary program-supplied
    /// message instead of a fixed compiler-chosen cause. Declares
    /// `IrType::Never`; the block it is in always ends in
    /// `Terminator::Unreachable` immediately after, since control never
    /// returns to whatever instruction would otherwise follow.
    FatalError(Operand),
    /// Records `Operand` as the pending exception (roadmap Phase 4b) —
    /// `throw`'s own lowering, right before the current function's own
    /// early `Terminator::Return` (unlike `FatalError`, this returns
    /// normally rather than aborting the process; see
    /// `zirk-runtime/src/exceptions.rs`'s own doc comment for the whole
    /// propagation mechanism, design decision D1 of
    /// `fase-4b-excepciones`).
    Throw(Operand),
    /// Attaches `suppressed` to `exception` before it is thrown (roadmap
    /// Phase 4b). Emitted only when a `throw` occurs inside an active `catch`.
    SetSuppressed {
        exception: Operand,
        suppressed: Operand,
    },
    /// Returns the `String` stack trace of `exception`, building it lazily
    /// and caching the result in the runtime (roadmap Phase 4b).
    StackTrace(Operand),
    /// Returns the `Throwable?` suppressed by `exception`, or absent if
    /// none was set (roadmap Phase 4b).
    Suppressed(Operand),
    /// Whether an exception is pending (roadmap Phase 4b), `IrType::Boolean`
    /// — emitted right after a call to a function/method that can throw.
    HasPendingException,
    /// Whether the active cancellation token is set (roadmap Phase 4c,
    /// `phase-4c-resources`) — consulted before a resource `close()` action.
    /// Produces `IrType::Boolean`.
    IsCancelled,
    /// Takes the pending exception, clearing the slot (roadmap Phase 4b) —
    /// a matching `catch`'s own lowering, right after `IsInstance` confirms
    /// it. Typed as whichever class the matching `catch` declared: the
    /// pointer is type-erased at the runtime boundary, but `IsInstance`
    /// already proved it fits.
    TakePendingException,
    /// Byte offset of the `index`-th grapheme in `string`, or `-1` if `index`
    /// is past the end (roadmap Phase 4e, `String[index]` read-only grapheme
    /// access). Used together with `GraphemeLenAt` and `GraphemeSlice`.
    StringGraphemeOffset {
        string: Operand,
        index: Operand,
    },
    /// Byte length of the grapheme at `offset` within `string`, or `-1` past
    /// the end (roadmap Phase 3b, task 6.3: `for ... in` over `String`
    /// produces `Char`). `offset` is threaded as an ordinary `Int64`
    /// loop-private value — the same shape a range loop already threads its
    /// own counter with — so this needs no instruction that mutates
    /// anything; it only ever answers a question.
    GraphemeLenAt {
        string: Operand,
        offset: Operand,
    },
    /// Builds a `Char` from the grapheme at byte range `[offset, offset +
    /// len)` — the `len` a prior `GraphemeLenAt` at the same `offset`
    /// already confirmed is exactly one grapheme.
    GraphemeSlice {
        string: Operand,
        offset: Operand,
        len: Operand,
    },

    /// Reads a slot.
    Load(SlotId),
    /// Writes a slot.
    Store(SlotId, Operand),

    /// Obtains storage for an object of the given layout.
    ///
    /// Expressed as "give me an object of this type", not as "reserve these
    /// bytes here": the strategy behind it belongs to the runtime, and naming
    /// one here is exactly what ADR-003 forbids the IR to do.
    Alloc(u32),
    /// Builds a record or value class value from its fields, in declaration
    /// order — no allocation, no header (roadmap task 11.5): a value class's
    /// construction is packaging, not obtaining storage, so it has no
    /// `Alloc` counterpart.
    BuildValue {
        class: u32,
        fields: Vec<Operand>,
    },
    /// Builds an algebraic enum value: the discriminant of `variant`, plus
    /// its own associated fields in declared order — everywhere else in the
    /// flattened payload is left undefined, the same way a record's omitted
    /// field never is but nothing here ever reads (roadmap task 11.3).
    BuildEnum {
        enum_id: u32,
        variant: u32,
        fields: Vec<Operand>,
    },
    /// Reads the discriminant of an algebraic enum value (roadmap task
    /// 11.3) — a traditional enum's own `Int32` value already is one, so
    /// only `IrType::Enum` needs this.
    Discriminant(Operand),
    /// `value as Target`: confirms at runtime that `value`'s actual class is
    /// `target_class` or one of its ancestors, terminating the process if
    /// not (roadmap task 11.6) — see [`ObjectLayout::ancestors`]. Produces
    /// the same pointer unchanged; only its declared type differs from
    /// `value`'s.
    CheckedCast {
        object: Operand,
        target_class: u32,
    },
    /// A value nothing may read (roadmap Phase 4b) — the placeholder
    /// `throw`'s own early `Terminator::Return` needs when the enclosing
    /// function's return type has no cheap zero value
    /// ([`Lowering::default_value`] covers Int/Float/Boolean/String/
    /// Nullable; this covers everything else). Sound only because the
    /// caller checks `zirk_rt_has_pending_exception()` before ever looking
    /// at what a `throws` call returned — LLVM's own `undef`, the same
    /// escape hatch codegen already reaches for elsewhere when a value is
    /// built field-by-field and momentarily incomplete.
    Undefined,
    /// `catch Type(name)`'s own runtime test (roadmap Phase 4b): whether
    /// `object`'s actual class is `target_class` or one of its ancestors —
    /// [`InstKind::CheckedCast`]'s own question, but answered rather than
    /// asserted, since a `catch` that does not match must keep running (try
    /// the next `catch`, or re-propagate) rather than terminate the process.
    IsInstance {
        object: Operand,
        target_class: u32,
    },
    /// Reinterprets an object or contract reference as a different, wider
    /// static type the checker already proved it fits — a subclass accepted
    /// where its base is expected, or a class accepted where a contract it
    /// implements is. No runtime work: an object's address does not change
    /// shape, only which methods a static type promises change with it.
    Retype(Operand),
    /// `value as <a different integer width>` (roadmap Phase 3b, task 4.3):
    /// truncates or sign/zero-extends to the destination's own width,
    /// exactly like Rust's own `as` between integers — unchecked, the
    /// explicit half of the widening/narrowing rule (`Type::accepts`
    /// covers the implicit, always-safe half). Codegen reads the *source*
    /// operand's own signedness from its recorded type to choose sign- vs
    /// zero-extension when widening; truncation needs no such choice.
    IntCast(Operand),
    /// A fractional or scientific literal (roadmap Phase 3b). The text is
    /// carried verbatim rather than parsed to `f64` here, for the same
    /// reason `zirk_ast::FloatLit` keeps it as text: `Float128` may exceed
    /// what an `f64` represents exactly, and codegen parses this same text
    /// once, at its own destination width, through LLVM's own literal
    /// parser — never through a Rust float in between.
    ConstFloat(FloatWidth, String),
    /// `value as <a different Float width>` (roadmap Phase 3b, task 4.3/5):
    /// truncates (`fptrunc`) or extends (`fpext`) to the destination width,
    /// unchecked — narrowing may lose precision or overflow to an infinity,
    /// the explicit half of the widening/narrowing rule the same way
    /// [`InstKind::IntCast`] is for integers.
    FloatCast(Operand),
    /// An integer operand implicitly widened to a `Float` width before a
    /// mixed-type arithmetic operator (`ZIRK_LANGUAGE_SPEC.md` section 3:
    /// "mixed integer and Float arithmetic produces Float"), or an explicit
    /// `as` from an integer to a `Float` — `sitofp`/`uitofp`, chosen by the
    /// source operand's own recorded signedness. Always exact: a `Float`
    /// width wide enough to hold every value of the specific integer width
    /// involved is not guaranteed by this instruction alone, so — like every
    /// other conversion in this family — precision loss is possible and
    /// accepted, not prevented.
    IntToFloat(Operand),
    /// `value as <an integer width>` from a `Float` operand — `fptosi`/
    /// `fptoui`, chosen by the *destination*'s signedness. Unchecked: a
    /// `Float` value outside the destination's representable range is
    /// undefined at the LLVM level exactly like Rust's own lossy `as`
    /// between float and integer, and is accepted on the same footing as
    /// every other explicit conversion in this family (roadmap Phase 3b).
    FloatToInt(Operand),
    /// `String + String`.
    Concat {
        left: Operand,
        right: Operand,
    },
    /// `String * Integer`, in either operand order.
    Repeat {
        string: Operand,
        count: Operand,
    },
    /// Calls method `index` of `contract` through the object's table for it.
    ///
    /// Which table that is cannot be known statically — the whole point of a
    /// contract — so the descriptor is searched at the call.
    CallContract {
        object: Operand,
        contract: u32,
        index: u32,
        args: Vec<Operand>,
    },
    /// Calls method `index` through the object's own table.
    ///
    /// Emitted only where the target is not statically known — that is, where
    /// some subclass redefines the method. Anything else is an ordinary call.
    CallVirtual {
        object: Operand,
        index: u32,
        args: Vec<Operand>,
    },
    /// Reads field `index` of an object, header excluded.
    LoadField {
        object: Operand,
        index: u32,
    },
    /// Writes field `index` of an object.
    StoreField {
        object: Operand,
        index: u32,
        value: Operand,
    },

    Unary {
        op: UnaryOp,
        operand: Operand,
    },
    Binary {
        op: BinaryOp,
        left: Operand,
        right: Operand,
    },
    /// Overflow detection for integer `+`, `-`, and `*` (roadmap Phase 4b):
    /// uses LLVM's `llvm.*.with.overflow` intrinsics and returns the
    /// `overflowed` flag as a `Boolean`. The matching arithmetic result is
    /// computed separately by `InstKind::Binary` on the non-failing path.
    CheckedArithmetic {
        op: BinaryOp,
        left: Operand,
        right: Operand,
        signed: bool,
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
    Wrap {
        base: Nullable,
        value: Operand,
    },
    /// Whether a nullable value is absent.
    IsNull(Operand),
    /// The value inside a nullable one.
    ///
    /// Only emitted on a path where an [`InstKind::IsNull`] already proved it
    /// present, which is what `??` establishes before using it.
    Unwrap(Operand),

    /// Builds a closure value from its captures.
    ///
    /// `target` names the module function this particular value's pointer
    /// field embeds — carried on the instruction itself, independent of
    /// `id`, so many differently-targeted values (a lambda's own lifted
    /// body, or a *different* named function entirely) can share one
    /// `id`'s `ClosureLayout` shape when they have the same captures
    /// (roadmap Phase 4d, design D12): `id` alone decides the struct's
    /// *layout* (how many captures, of what types), never which function a
    /// given value calls — `InstKind::CallClosure` never reads `target` at
    /// all, only the pointer the value carries at runtime.
    MakeClosure {
        id: u32,
        captures: Vec<Operand>,
        target: String,
    },
    /// Calls a closure value.
    ///
    /// The captures travel inside the operand, so the call passes them ahead of
    /// the arguments the caller wrote.
    CallClosure {
        id: u32,
        callee: Operand,
        args: Vec<Operand>,
    },

    /// Builds a boxed callable value from a function reference and its
    /// captured values (roadmap Phase 4d, boxed callables task 1).
    ///
    /// This is the two-word representation `{ function pointer, capture block
    /// pointer }`. The capture block is allocated by `zirk_rt_alloc_callable`
    /// and the captured values are copied into it during codegen.
    MakeCallable {
        target: String,
        captures: Vec<Operand>,
    },
    /// Calls a boxed callable value.
    ///
    /// The callable operand carries the two-word value; the call loads the
    /// function pointer and the capture block pointer, reads the captured
    /// values back from the block, then calls the function pointer with the
    /// captures followed by the written arguments.
    CallCallable {
        callable: Operand,
        args: Vec<Operand>,
    },

    /// `Pointer.from(place)` where `place` is a local/parameter slot
    /// (roadmap Phase 4e, design D8) — codegen reuses the `alloca` already
    /// computed for that slot; no new storage.
    PointerFromSlot(SlotId),
    /// `Pointer.from(place)` where `place` is a field of an object or value
    /// (design D8) — codegen reuses the field's own already-computed GEP.
    PointerFromField {
        object: Operand,
        index: u32,
    },
    /// `.read()` — a plain LLVM `load` through the pointer, typed by `T`.
    PointerRead(Operand),
    /// `.write(value)` — a plain LLVM `store` through the pointer.
    PointerWrite {
        pointer: Operand,
        value: Operand,
    },
    /// `.offset(n)` — `getelementptr` in units of `T` (design D8).
    PointerOffset {
        pointer: Operand,
        amount: Operand,
    },
    /// `.offset_bytes(n)` — `getelementptr` over an `i8`-typed view of the
    /// same pointer (design D8).
    PointerOffsetBytes {
        pointer: Operand,
        amount: Operand,
    },
    /// `.cast<U>()` — an LLVM pointer bitcast.
    PointerCast(Operand),
    /// `.is_null` — the one pointer operation that does not require `unsafe`.
    PointerIsNull(Operand),

    /// `Weak.from(value)` (roadmap Phase 4e, `fase-4e-weak`, design D1):
    /// allocates a fresh WeakCell and stores `value`'s own address (already
    /// a managed reference — no address-of needed, unlike `Pointer.from`)
    /// into its single field. Produces a `Weak<T>` value: a pointer to the
    /// WeakCell, itself an ordinary collector-tracked allocation.
    WeakFrom(Operand),
    /// `.upgrade()` (design D4): reads the WeakCell's target field and
    /// produces the nullable result — present when the referent is still
    /// reachable, absent (`null`) otherwise. This is a managed-reference-
    /// typed instruction result when present, so it goes through the same
    /// unconditional synthetic-slot spill every such result already does
    /// (`fase-4e-colector-mark-sweep` design D4) — nothing new needed here.
    WeakUpgrade(Operand),
    /// `.is_alive` (design D4): the same null-check `.upgrade()` does,
    /// without producing a new strong reference.
    WeakIsAlive(Operand),

    /// `x.clone()` on a `Clone`-derived class receiver (roadmap Phase 4e,
    /// `fase-4e-clone`, design D2): a deep-clone-graph traversal, producing
    /// new identity for every reachable object while preserving internal
    /// sharing and cycles through a per-call memoization map (source
    /// address → already-cloned copy address).
    ///
    /// Lowered to one call into the runtime's own generic, descriptor-driven
    /// traversal (`zirk_rt_clone`, `crates/zirk-codegen-llvm/src/emit.rs`) —
    /// deliberately *not* five separate IR-level ops (`CloneBegin`/
    /// `CloneLookup`/`CloneRecord`/`CloneAlloc`/`CloneEnd`) threading a
    /// context through per-field recursion, despite that being this
    /// change's own design doc's literal D2 wording. Deviation, documented
    /// here rather than only in `tasks.md` (matching `fase-4e-weak`'s own
    /// `Base::Weak(u32)` vs `Box<Type>` precedent for correcting design
    /// prose against a checked reality): the field-offset table a
    /// compile-time, per-declared-field-type recursion would need to bake
    /// in (this same crate's/codegen's `gc_reference_paths`) is keyed to
    /// the *static* field type, but a `class` receiver may at runtime be a
    /// more-derived subclass with a larger, different layout (ordinary
    /// class polymorphism, already handled correctly by the collector's own
    /// `mark_object`, which reads size/fields from the object's own runtime
    /// header rather than from the static type at the allocation site).
    /// Baking clone's own field walk into IR/codegen at the *declared*
    /// type would silently miss a subclass's own extra fields or use the
    /// wrong size — the design doc's own "Alternative considered" already
    /// prefers "a single generic runtime entry point parameterized by the
    /// per-class field-layout table" over a distinct compile-time-generated
    /// function per class; this applies that same reasoning one layer
    /// further, to the *whole* recursive walk, not merely which function
    /// dispatches it, and is what makes the operation correct for a
    /// polymorphic receiver rather than merely simpler. Runtime-side
    /// correctness (memoization scoping, cycle safety, collector-safety of
    /// a clone in progress) is unchanged from design's own D2/D3 — only
    /// where the recursion executes (in `zirk-runtime`'s own Rust, driven
    /// by the same field-offset table the collector's mark pass already
    /// reads from each object's header, instead of unrolled across several
    /// IR instructions) moved.
    Clone(Operand),

    /// Begins a new per-`unsafe`-block undo log (roadmap Phase 4e,
    /// `fase-4e-unsafe-journal`, design D1): `journal_begin` on entry to an
    /// `unsafe { ... }` block. The result is an ordinary local SSA pointer
    /// value, spilled to a slot like any other cross-block pointer, never a
    /// GC root — a `zirk-runtime`-internal allocation outside the
    /// collector's own object model (design D4).
    JournalBegin,
    /// Snapshots slot `slot`'s current bytes into `journal`'s undo log,
    /// immediately before the `Store` it guards executes (design D1/D3) —
    /// the slot's own static type (already known at this call site from
    /// [`Slot::ty`]) is what sizes the snapshot; no runtime size discovery.
    /// Never emitted for a slot declared inside the same `unsafe` block
    /// (design D1's own exemption): nothing outside the block could observe
    /// rolling back a write to storage the block itself allocated.
    JournalRecordSlot {
        journal: Operand,
        slot: SlotId,
    },
    /// Snapshots object field `index`'s current bytes into `journal`'s undo
    /// log, immediately before the `StoreField` it guards executes (design
    /// D1/D3) — sized from the field's own static type. Unlike
    /// [`InstKind::JournalRecordSlot`], every `StoreField` inside an active,
    /// not-yet-committed `unsafe` block is journaled unconditionally: a
    /// field belongs to heap state, not to this function's own lexical
    /// scoping, so there is no "declared inside the block" exemption to
    /// apply to it (accepted minor over-journaling, same trade-off design's
    /// own risk list already accepts for duplicate records).
    JournalRecordField {
        journal: Operand,
        object: Operand,
        index: u32,
    },
    /// Durably commits `journal`: discards the undo log without restoring,
    /// and frees the journal itself (design D1) — the success path, taken
    /// both at an `unsafe {}` block's own normal fall-through exit and at
    /// `commit {}`'s own entry, against the *enclosing* `unsafe` block's
    /// journal (design D2).
    JournalCommit(Operand),
    /// Rolls `journal` back: restores every recorded snapshot in reverse
    /// order, then discards and frees the log (design D1/D2) — the failure
    /// path, taken instead of [`InstKind::JournalCommit`] at an `unsafe {}`
    /// block's own exit check point when an exception is pending.
    JournalRollback(Operand),

    /// Runs `pointer.as_slice(length)`/`.as_slice_mut(length)`'s runtime
    /// validation (roadmap Phase 4e, `fase-4e-native-slice`, design D3):
    /// non-null, alignment against the element type, extent
    /// representability, and — when `known_length` is `Some` — that
    /// `length` does not exceed it. Produces `Boolean`: `true` when every
    /// check passes.
    ///
    /// `known_length` is this pass's own scope decision on design D3's
    /// "where the underlying allocation's own size is knowable": tracked
    /// only for the syntactically direct case `Pointer.from(place).as_slice(n)`
    /// (`place`'s own single-element storage, `Self::known_pointer_extent`),
    /// not general provenance/dataflow tracking — `None` (opaque
    /// provenance) for everything else, trusting the caller's `length`
    /// beyond what is otherwise checkable, exactly the accepted risk
    /// design's own "Risks/Trade-offs" section documents.
    ///
    /// `element` is the view's own element `IrType`, not a precomputed
    /// byte size/alignment: those are backend layout facts (`ADR-003`
    /// keeps the IR from naming how a type is laid out), so codegen derives
    /// them from `element`'s own LLVM type at the point it lowers this
    /// instruction, the same way `InstKind::WeakFrom`'s codegen already
    /// derives a WeakCell's size from its own LLVM struct type rather than
    /// a value carried on the instruction.
    NativeSliceValidate {
        pointer: Operand,
        length: Operand,
        element: IrType,
        known_length: Option<u64>,
    },
    /// Packs an already-validated `(pointer, length)` pair into a
    /// `NativeSlice<T>`/`NativeSliceMut<T>` value (design D2) — no
    /// allocation, no header: a plain two-word struct value. Only ever
    /// emitted on the branch `NativeSliceValidate` proved `true` for.
    NativeSliceValue {
        pointer: Operand,
        length: Operand,
    },
    /// `view.length` (roadmap Phase 4e, `fase-4e-native-slice`): reads the
    /// length word out of the `(pointer, length)` representation.
    NativeSliceLength(Operand),
    /// `view[i]` read (design D5): a bounds-checked load at `pointer + i *
    /// sizeof(T)`. Only ever emitted on the branch a bounds check
    /// (`Self::lower_native_slice_bounds_check`) already proved `i` safe
    /// for — this instruction itself performs no check of its own.
    NativeSliceLoad {
        receiver: Operand,
        index: Operand,
    },
    /// `view[i] = value` (design D5): a bounds-checked store, the write
    /// counterpart of [`InstKind::NativeSliceLoad`] — only ever emitted
    /// against a `NativeSliceMut<T>` receiver (the checker's own dispatch
    /// table rejects a write through a read-only `NativeSlice<T>` before
    /// lowering ever sees one).
    NativeSliceStore {
        receiver: Operand,
        index: Operand,
        value: Operand,
    },
    /// `transfer(source)` (roadmap Phase 4c): transfers ownership of a
    /// `TransferableResource`, returning the resource pointer in a fresh owned
    /// slot and invalidating the source binding statically.
    ResourceTransfer {
        source: Operand,
    },

    /// `Dependent.from(base, ptr)` (roadmap Phase 4e, `phase-4e-memory`,
    /// design D1): records both the field pointer and the base object that
    /// owns it, producing a `Dependent<T>` value. Surface-only stub.
    DependentFrom {
        base: Operand,
        field_ptr: Operand,
    },
    /// `Pin<T>(object)` / automatic pin on interior pointer exposure
    /// (roadmap Phase 4e, `phase-4e-memory`, design D1): adds `object` to the
    /// per-thread pin list and produces a `Pin<T>` value that is the same
    /// object pointer.
    PinObject {
        object: Operand,
    },
    /// Releases a pin previously created by [`InstKind::PinObject`]. Produces
    /// no value (`Void`). Surface-only stub.
    UnpinObject {
        object: Operand,
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
    BitNot,
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
    /// `is`: whether two references name the same instance.
    ///
    /// A comparison of addresses, which is why it needs no contract and no
    /// runtime call: identity *is* the address.
    Identical,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinaryOp {
    /// Type produced by the operator given its operands.
    pub const fn result_type(self, operand: IrType) -> IrType {
        use BinaryOp::*;
        match self {
            Add | Sub | Mul | Div | Rem | BitAnd | BitOr | BitXor | Shl | Shr => operand,
            Eq | NotEq | Lt | LtEq | Gt | GtEq | And | Or | Identical => IrType::Boolean,
        }
    }
}

/// How a basic block ends.
#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Return(Option<Operand>),
    /// Control never arrives here.
    ///
    /// A block still needs a terminator even when nothing can reach it — the
    /// block after `loop { return x; }` is the ordinary case. Inventing a
    /// return value for it would have to invent one of the function's type,
    /// and that value would be a lie about code that never runs.
    Unreachable,
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
            Terminator::Return(_) | Terminator::Unreachable => Vec::new(),
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
        assert!(!IrType::Int(IntWidth::I32).needs_allocation());
        assert!(!IrType::Boolean.needs_allocation());
        assert!(!IrType::Void.needs_allocation());
    }

    #[test]
    fn arithmetic_preserves_the_operand_type() {
        assert_eq!(
            BinaryOp::Add.result_type(IrType::Int(IntWidth::I32)),
            IrType::Int(IntWidth::I32)
        );
    }

    #[test]
    fn comparison_and_logic_yield_boolean() {
        for op in [BinaryOp::Eq, BinaryOp::Lt, BinaryOp::And, BinaryOp::Or] {
            assert_eq!(op.result_type(IrType::Int(IntWidth::I32)), IrType::Boolean);
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
