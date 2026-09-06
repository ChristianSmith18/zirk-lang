//! The types of the Phase 2 subset.
//!
//! Phase 1 modelled four scalars. This phase adds what its constructs need: a
//! nullable form for `T?`, enums as closed sets of names, function types for
//! closures, and ranges for `for ... in`.
//!
//! # Why a struct instead of one more variant
//!
//! `T?` is `T | Null` for every `T`, so a `Nullable` variant would have to hold
//! another type, and that makes the type non-`Copy` — which ripples through
//! every layer that passes types around by value. Since `T??` is not a thing,
//! nullability is exactly one bit, and a bit next to the base costs nothing.
//!
//! Enums and function types carry an index into a table owned by the checker
//! rather than their contents, for the same reason.

use zirk_diagnostics::Phase;

/// A type of the subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Type {
    pub base: Base,
    /// Written `T?`, per `ZIRK_LANGUAGE_SPEC.md` section 4.
    pub nullable: bool,
}

/// Every integer type the language has (roadmap Phase 3b): five signed
/// widths, five unsigned, with `Int32` the one Phase 1 already had.
///
/// A width plus a signedness, not ten separate `Base` variants: LLVM already
/// models any integer width natively (`IntType`), so the only real cost of
/// generalizing is deciding, at each site that used to assume `Int32`,
/// whether it now means "any integer" or specifically needs 32 bits — a
/// decision, not a rename. See `openspec/changes/fase-3b-scalars-and-text/design.md`
/// D1 and its audit for the accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    pub const fn name(self) -> &'static str {
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

    /// The range of values a literal of this width may write, as `i128`.
    ///
    /// `UInt128`'s true upper bound (`u128::MAX`) does not fit in `i128` —
    /// but neither does any integer literal `zirk-lexer` can produce in the
    /// first place (`TokenKind::Integer(i128)`, capped since Phase 1 with
    /// its own `INTEGER_TOO_LARGE` diagnostic). So `i128::MAX` is not an
    /// approximation of `UInt128`'s range here: it is the literal's own
    /// ceiling, which `UInt128` inherits along with every other width. A
    /// `UInt128` *value* can still exceed it — through arithmetic, not
    /// through a literal — the same way it always could for `Int32`.
    pub const fn literal_range(self) -> (i128, i128) {
        use IntWidth::*;
        match self {
            I8 => (i8::MIN as i128, i8::MAX as i128),
            I16 => (i16::MIN as i128, i16::MAX as i128),
            I32 => (i32::MIN as i128, i32::MAX as i128),
            I64 => (i64::MIN as i128, i64::MAX as i128),
            I128 => (i128::MIN, i128::MAX),
            U8 => (0, u8::MAX as i128),
            U16 => (0, u16::MAX as i128),
            U32 => (0, u32::MAX as i128),
            U64 => (0, u64::MAX as i128),
            U128 => (0, i128::MAX),
        }
    }

    /// Whether every value of `other` fits in `self` without loss.
    pub fn can_represent_all_of(self, other: Self) -> bool {
        use IntWidth::*;
        match (self, other) {
            (a, b) if a == b => true,
            (I8 | I16 | I32 | I64 | I128, U8 | U16 | U32 | U64 | U128) => false,
            (U8 | U16 | U32 | U64 | U128, I8 | I16 | I32 | I64 | I128) => false,
            (I8 | I16 | I32 | I64 | I128, I8 | I16 | I32 | I64 | I128) => {
                self.bits() >= other.bits()
            }
            (U8 | U16 | U32 | U64 | U128, U8 | U16 | U32 | U64 | U128) => {
                self.bits() >= other.bits()
            }
        }
    }

    /// Whether this integer type can be converted to `float` without losing
    /// any integer value. An n-bit integer needs a float whose mantissa can
    /// represent n bits, i.e. `n <= mantissa_bits + 1`.
    pub const fn fits_exactly_in_float(self, float: FloatWidth) -> bool {
        let mantissa = float.mantissa_bits();
        self.bits() <= mantissa + 1
    }
}

/// Every binary floating-point type the language has (roadmap Phase 3b):
/// `Float16`/`Float32`/`Float64`/`Float128`, with `Float` an alias of
/// `Float64` (`Type::FLOAT64`). Widening between widths is always exact —
/// unlike integers, a wider IEEE 754 format can represent every value the
/// narrower one can — so [`Type::accepts`] only needs `dst.bits() >=
/// src.bits()`, no signedness to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    /// The surface name of this width.
    ///
    /// The Rust identifiers in this module keep the name `Float` for the
    /// binary IEEE 754 family, but the language surface calls it `BinaryFloat*`
    /// — the plain name `Float` is the exact base-ten decimal type
    /// ([`Base::Decimal`]).
    pub const fn name(self) -> &'static str {
        use FloatWidth::*;
        match self {
            F16 => "BinaryFloat16",
            F32 => "BinaryFloat32",
            F64 => "BinaryFloat64",
            F128 => "BinaryFloat128",
        }
    }

    /// The finite magnitude a literal of this width may name without
    /// overflowing to infinity, as an `f64` upper bound.
    ///
    /// `Float128`'s true range vastly exceeds `f64::MAX`, but there is no
    /// `f64`-based way to detect a `Float128` literal that overflows *its*
    /// range without parsing decimal text at `f128` precision, which no
    /// dependency here does. So `Float128` reports no bound at all — its
    /// literal range check is a known, documented gap (see `check_float_literal`)
    /// rather than a false one built on a wrong number.
    pub const fn literal_bound(self) -> Option<f64> {
        use FloatWidth::*;
        match self {
            F16 => Some(65504.0),
            F32 => Some(f32::MAX as f64),
            F64 => Some(f64::MAX),
            F128 => None,
        }
    }

    /// Number of mantissa bits (excluding the implicit leading 1) for this
    /// IEEE 754 width. A float with `m` mantissa bits can exactly represent
    /// every integer with absolute value < 2^(m+1).
    pub const fn mantissa_bits(self) -> u32 {
        use FloatWidth::*;
        match self {
            F16 => 10,
            F32 => 23,
            F64 => 52,
            F128 => 112,
        }
    }
}

/// The part of a type that is not its nullability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Base {
    Void,
    /// `Int8`…`UInt128` (roadmap Phase 3b) — `Int`/`Integer` alias `Int32`,
    /// `Type::INT32` is `Int(IntWidth::I32)`.
    Int(IntWidth),
    /// The IEEE 754 binary floating family. Surface name: `BinaryFloat16`…
    /// `BinaryFloat128`, with `BinaryFloat` aliasing `BinaryFloat64`
    /// (`Type::BINARY_FLOAT64`). The Rust identifier keeps the name `Float`
    /// from before the exact type existed; see [`FloatWidth::name`].
    Float(FloatWidth),
    /// The exact base-ten decimal type. Surface name: `Float` (`Type::FLOAT`).
    /// A value is a signed 128-bit coefficient and a decimal scale; there is
    /// no width family and no `NaN`/infinity. Backed by `zirk_rt_decimal_*`.
    Decimal,
    /// Exactly one Unicode grapheme (roadmap Phase 3b) — represented exactly
    /// like `String` at runtime (ADR-014), but a distinct static type: no
    /// mutation methods, and no identity (`is` is rejected, unlike `String`).
    Char,
    Boolean,
    String,
    /// An exact elapsed-time value, stored as nanoseconds.
    Duration,
    /// A compiled regular expression, represented as an opaque runtime handle.
    Regex,
    /// The type of the `null` literal, assignable to any nullable type.
    Null,
    /// A declared enum, identified by its index in the checker's table.
    Enum(u32),
    /// A function value, identified by its index in the checker's table.
    Function(u32),
    /// A declared interface or trait, identified by its index in the checker's
    /// table.
    ///
    /// A value of this type is reached through the contract rather than
    /// through its own class: what it can do is what the contract declares,
    /// and which body runs is decided at the call.
    Contract(u32),
    /// A declared class, identified by its index in the checker's table.
    ///
    /// Nominal: two classes with identical members are different types, which
    /// is what `ZIRK_LANGUAGE_SPEC.md` section 7 means by a class having
    /// identity. Structural equality is what records are for.
    Class(u32),
    /// A generic type parameter in scope where it is declared, identified by
    /// its index in the checker's table.
    ///
    /// Opaque: it is equal only to itself, so a body can pass a `T` around and
    /// compare two `T`s, but cannot assume anything a `from` constraint does
    /// not promise. Verifying that promise is a later pass (`ZIRK_ROADMAP.md`
    /// Phase 3's generics slice); today the parameter is tracked so its own
    /// declaration type-checks instead of reporting an unknown type.
    Param(u32),
    /// A generic class instantiated with concrete type arguments, such as
    /// `Box<Int32>`, identified by its index in the checker's table.
    ///
    /// Interned like [`Base::Function`]: `Box<Int32>` written twice resolves
    /// to the same id, which is what makes two such references the same type
    /// by `==` instead of merely by structure.
    ///
    /// A value of this type cannot yet be constructed — there is no
    /// expression syntax for it — so today it exists only to let a
    /// declaration name the type and have its arguments checked against `T`'s
    /// constraints (roadmap task 7.3). Reading a member through one is not
    /// implemented.
    Instance(u32),
    /// A declared interface or trait instantiated with concrete type
    /// arguments, such as `Iterable<Int32>`, identified by its index in the
    /// checker's table. See [`Base::Instance`], its class equivalent.
    ContractInstance(u32),
    /// A declared enum instantiated with concrete type arguments, such as
    /// `Iteration<Int32>`, identified by its index in the checker's table.
    /// See [`Base::Instance`], its class equivalent.
    EnumInstance(u32),
    /// `Pointer<T>` (roadmap Phase 4e), identified by the index of its
    /// pointee type `T` in the checker's `pointer_types` table.
    ///
    /// Its own `Base` variant rather than routed through `Base::Instance`
    /// (design D1): the operation set is closed and compiler-built-in, so it
    /// needs none of the constructor resolution or method-table dispatch a
    /// user generic class does.
    Pointer(u32),
    /// `Weak<T>` (roadmap Phase 4e, `fase-4e-weak`, design D1), identified by
    /// the index of its referent type `T` in the checker's `weak_types`
    /// table.
    ///
    /// Its own `Base` variant, parallel to [`Base::Pointer`] rather than
    /// routed through [`Base::Instance`] (design D1): the operation set is
    /// closed and compiler-built-in, so it needs none of the constructor
    /// resolution or method-table dispatch a user generic class does. `T` is
    /// restricted to a reference type (checked against `is_reference_type`
    /// at resolution, not stored here).
    Weak(u32),
    /// `NativeSlice<T>` (roadmap Phase 4e, `fase-4e-native-slice`, design
    /// D1), identified by the index of its element type `T` in the
    /// checker's `native_slice_types` table.
    ///
    /// Its own `Base` variant, parallel to [`Base::Pointer`]/[`Base::Weak`]
    /// rather than routed through [`Base::Instance`] (design D1): a
    /// compiler-built-in, closed operation set that needs no constructor
    /// resolution or method-table dispatch. Read-only: indexing produces
    /// `T`, never accepts a write (design D5). `T` is restricted to the
    /// same ABI-safe subset [`Base::Pointer`] already accepts.
    NativeSlice(u32),
    /// `NativeSliceMut<T>` (roadmap Phase 4e, `fase-4e-native-slice`,
    /// design D1), identified by the index of its element type `T` in the
    /// checker's `native_slice_mut_types` table.
    ///
    /// The read/write counterpart of [`Base::NativeSlice`] — a distinct
    /// variant rather than a mutability flag on one, so a value's static
    /// type alone (not a hidden bit) decides whether `view[i] = x` is
    /// accepted (design D5).
    NativeSliceMut(u32),
    /// `Dependent<T>` (roadmap Phase 4e, `phase-4e-memory`, design D1),
    /// identified by the index of its element type `T` in the checker's
    /// `dependent_types` table.
    ///
    /// A reference whose lifetime is tied to the allocation that contains
    /// the value it refers to.
    Dependent(u32),
    /// `Pin<T>` (roadmap Phase 4e, `phase-4e-memory`, design D1), identified
    /// by the index of its referent type `T` in the checker's `pin_types`
    /// table.
    ///
    /// Keeps an object's address stable for native interop.
    Pin(u32),
    /// `A | B`, identified by its index in the checker's table.
    ///
    /// The table holds the normalized alternative list: order-independent,
    /// deduplicated, with a subtype alternative already collapsed into its
    /// supertype (`ZIRK_LANGUAGE_SPEC.md` section 4) — two unions with the
    /// same effective members are the same id no matter how each was
    /// written. `T | Null` is not stored this way at all: it folds into the
    /// ordinary `nullable` bit any [`Type`] already carries, the same as
    /// every other type's `?`, so a `Union` always has at least two
    /// alternatives.
    Union(u32),
    /// `Range<T>` (roadmap Phase 7): the result of `a..b`, `a..=b` or
    /// `a..b..step`, iterable by `for ... in`, identified by the index of
    /// its element type `T` in the checker's `range_types` table.
    Range(u32),
    /// `Tuple(T...)` (roadmap Phase 3b), identified by its index in the
    /// checker's `tuple_types` table.
    Tuple(u32),
    /// The bottom type (roadmap Phase 4a): no value of it exists, so an
    /// expression of this type is assignable anywhere and unifies with any
    /// type at a branch join (`cond ? 5 : fatalError("...")` types `Int32`,
    /// not `Never`). `fatalError`'s own return type — a call that never
    /// returns normally has no result to give a concrete type to.
    Never,
    /// Assigned to expressions whose type could not be determined.
    ///
    /// It exists so one type error does not cascade into a dozen derived ones:
    /// anything involving an `Unknown` is silently accepted, because the real
    /// error was already reported at its origin.
    Unknown,
    /// `Array<T>`, identified by the index of its element type `T` in the
    /// checker's `array_types` table.
    Array(u32),
    /// `List<T>`, identified by the index of its element type `T` in the
    /// checker's `list_types` table.
    List(u32),
}

impl Type {
    pub const VOID: Type = Type::of(Base::Void);
    pub const INT32: Type = Type::of(Base::Int(IntWidth::I32));
    /// The IEEE 754 binary `BinaryFloat64` (surface name), `BinaryFloat`'s
    /// target. The constant keeps its old name.
    pub const FLOAT64: Type = Type::of(Base::Float(FloatWidth::F64));
    /// The exact base-ten decimal type — surface name `Float`.
    pub const FLOAT: Type = Type::of(Base::Decimal);
    pub const BOOLEAN: Type = Type::of(Base::Boolean);
    pub const STRING: Type = Type::of(Base::String);
    pub const DURATION: Type = Type::of(Base::Duration);
    pub const REGEX: Type = Type::of(Base::Regex);
    pub const NULL: Type = Type::of(Base::Null);

    pub const UNKNOWN: Type = Type::of(Base::Unknown);

    pub const fn of(base: Base) -> Self {
        Self {
            base,
            nullable: false,
        }
    }

    /// The nullable form of this type.
    pub const fn as_nullable(self) -> Self {
        Self {
            base: self.base,
            nullable: true,
        }
    }

    /// The type with its nullability removed, which is what `??` produces.
    pub const fn without_null(self) -> Self {
        Self {
            base: self.base,
            nullable: false,
        }
    }

    pub const fn is_unknown(self) -> bool {
        matches!(self.base, Base::Unknown)
    }

    /// Whether a value of this type may be absent.
    pub const fn admits_null(self) -> bool {
        self.nullable || matches!(self.base, Base::Null | Base::Unknown)
    }

    /// Whether two types are compatible for assignment.
    ///
    /// There are no implicit conversions (`LANGUAGE_SPEC` section 3), so
    /// compatibility is equality, with two exceptions that are not conversions:
    ///
    /// - `T` is accepted where `T?` is expected. Widening loses nothing, and
    ///   the reverse is what `??` exists to make explicit.
    /// - `null` is accepted by any nullable type.
    ///
    /// `Unknown` is compatible with everything, to avoid cascading errors.
    pub fn accepts(self, other: Type) -> bool {
        if self.is_unknown() || other.is_unknown() {
            return true;
        }

        // No value of `Never` exists (roadmap Phase 4a), so a `Never`
        // expression fits anywhere — checked ahead of the `null` rule since
        // it is unconditional, unlike `null`'s "only where absence is
        // admitted".
        if matches!(other.base, Base::Never) {
            return true;
        }

        // `null` fits anything that admits absence, and nothing else.
        if matches!(other.base, Base::Null) {
            return self.admits_null();
        }

        if self.base != other.base {
            // Safe widening is the one exception that is not equality
            // (roadmap Phase 3b, task 4.3): a narrower integer fits a wider
            // one of the *same* signedness without an explicit conversion —
            // nothing is lost. Crossing signedness, or narrowing, is not
            // "unambiguous" the way `ZIRK_LANGUAGE_SPEC.md` section 3
            // requires for an implicit conversion, so neither goes through
            // here; both need `as` instead.
            return match (self.base, other.base) {
                // Integer widening, including unsigned-to-signed when the
                // signed target has enough magnitude bits.
                (Base::Int(dst), Base::Int(src)) => {
                    dst.can_represent_all_of(src) && (self.nullable || !other.nullable)
                }
                // Float widening is always exact.
                (Base::Float(dst), Base::Float(src)) => {
                    dst.bits() >= src.bits() && (self.nullable || !other.nullable)
                }
                // Int -> BinaryFloat is safe when every source value is exactly
                // representable in the target float.
                (Base::Float(dst), Base::Int(src)) => {
                    src.fits_exactly_in_float(dst) && (self.nullable || !other.nullable)
                }
                // Int -> exact Float is always exact (every integer is a
                // terminating decimal); Decimal -> Decimal is trivially exact.
                (Base::Decimal, Base::Int(_) | Base::Decimal) => {
                    self.nullable || !other.nullable
                }
                _ => false,
            };
        }

        // `T` fits `T?`; `T?` does not fit `T`.
        self.nullable || !other.nullable
    }

    /// The smallest numeric type that can represent every value of `self` and
    /// `other` without loss. Returns `None` if no such type exists.
    ///
    /// Candidates are tried in order of increasing cost (bit width, then
    /// integer before float at the same width) so the first match is the
    /// preferred common type.
    pub fn common_numeric(self, other: Type) -> Option<Type> {
        use Base::*;
        if self.nullable || other.nullable {
            return None;
        }

        // The exact `Float` is only a *common* type when one operand already
        // is one — two integers never promote to it (that would make
        // `i128 + u128` silently exact-decimal instead of "no common type").
        let decimal_in_play = matches!(self.base, Decimal) || matches!(other.base, Decimal);

        let candidates = [
            Type::of(Int(IntWidth::I8)),
            Type::of(Int(IntWidth::U8)),
            Type::of(Int(IntWidth::I16)),
            Type::of(Int(IntWidth::U16)),
            Type::of(Int(IntWidth::I32)),
            Type::of(Int(IntWidth::U32)),
            Type::of(Int(IntWidth::I64)),
            Type::of(Int(IntWidth::U64)),
            Type::of(Int(IntWidth::I128)),
            Type::of(Int(IntWidth::U128)),
            // The exact decimal type is the common type of an integer and a
            // `Float`; it never joins with a `BinaryFloat` (its `accepts` has
            // no `Decimal`/`Float` arm), so ordering it here is safe.
            Type::FLOAT,
            Type::of(Float(FloatWidth::F16)),
            Type::of(Float(FloatWidth::F32)),
            Type::of(Float(FloatWidth::F64)),
            Type::of(Float(FloatWidth::F128)),
        ];

        candidates
            .into_iter()
            .filter(|candidate| !matches!(candidate.base, Decimal) || decimal_in_play)
            .find(|candidate| candidate.accepts(self) && candidate.accepts(other))
    }

    /// The type both operands of `??` or of a `match` share, if any.
    ///
    /// Not general unification: with no subtyping until Phase 3, two types are
    /// either the same base or incompatible. The only real work is deciding
    /// the nullability of the result.
    pub fn unify(self, other: Type) -> Option<Type> {
        if self.is_unknown() {
            return Some(other);
        }
        if other.is_unknown() {
            return Some(self);
        }

        // `Never` contributes nothing at a branch join — `cond ? 5 :
        // fatalError("...")` types `Int32`, not `Never` and not `Int32?`
        // (unlike `Null`, `Never` does not make the other side nullable:
        // there is no absent value to represent, the branch just never
        // completes).
        if matches!(self.base, Base::Never) {
            return Some(other);
        }
        if matches!(other.base, Base::Never) {
            return Some(self);
        }

        if matches!(self.base, Base::Null) {
            return Some(other.as_nullable());
        }
        if matches!(other.base, Base::Null) {
            return Some(self.as_nullable());
        }

        if self.base != other.base {
            return None;
        }

        Some(Type {
            base: self.base,
            nullable: self.nullable || other.nullable,
        })
    }

    /// Whether the type has a default value, per
    /// `docs/handbook/11-reference/03-built-in-types.md`.
    ///
    /// An omitted attribute receives it before any explicit initializer or
    /// constructor runs (`ZIRK_LANGUAGE_SPEC.md` section 7), so a constructor
    /// only has to write what has no default.
    ///
    /// A class has none: it is a reference with identity, and there is no
    /// instance to default to. Nullable types default to `null`, which is
    /// exactly what absence means.
    ///
    /// An enum has none either. Its only candidate would be the first declared
    /// variant, and `ZIRK_LANGUAGE_SPEC.md` section 7 states that a traditional
    /// enum exposes no declaration order — so a default derived from it would
    /// make observable exactly what the spec says is not.
    pub const fn has_default(self) -> bool {
        if self.nullable {
            return true;
        }
        matches!(
            self.base,
            Base::Int(_) | Base::Float(_) | Base::Decimal | Base::Boolean | Base::String
        ) && !matches!(self.base, Base::Tuple(_))
    }

    /// Range of values representable by the type, for integer literals.
    pub const fn integer_range(self) -> Option<(i128, i128)> {
        match self.base {
            Base::Int(width) => Some(width.literal_range()),
            _ => None,
        }
    }

    /// Resolves a non-nullable type from the name written in the source.
    ///
    /// Enums are not here: they are declared by the program, so the checker
    /// resolves them against its own table.
    ///
    /// `Int` and `Integer` resolve here rather than being deferred, because an
    /// alias *is* its target: both name `Int32`, which has existed since the
    /// first phase. Deferring them deferred a spelling, not a capability, and
    /// after resolution nothing distinguishes them — which is what being an
    /// alias means. `UInt` is not here because `UInt32` is not implemented.
    pub fn from_name(name: &str) -> Option<Self> {
        use IntWidth::*;
        Some(match name {
            "Void" => Type::VOID,
            "Int32" | "Int" | "Integer" => Type::INT32,
            "Int8" => Type::of(Base::Int(I8)),
            "Int16" => Type::of(Base::Int(I16)),
            "Int64" => Type::of(Base::Int(I64)),
            "Int128" => Type::of(Base::Int(I128)),
            "UInt8" => Type::of(Base::Int(U8)),
            "UInt16" => Type::of(Base::Int(U16)),
            "UInt32" => Type::of(Base::Int(U32)),
            "UInt64" => Type::of(Base::Int(U64)),
            "UInt128" => Type::of(Base::Int(U128)),
            // `Byte` is a recognized alias of `UInt8` (roadmap Phase 4e,
            // design D2) — `Pointer<Byte>` is how the native-interoperability
            // examples in `MEMORY_AND_UNSAFE_SEMANTICS.md` already spell it.
            "Byte" => Type::of(Base::Int(U8)),
            // The plain name is the exact base-ten type. The former binary
            // spellings `Float16/32/64/128` no longer resolve — the checker
            // turns them into a redirect diagnostic.
            "Float" => Type::FLOAT,
            "BinaryFloat64" | "BinaryFloat" => Type::FLOAT64,
            "BinaryFloat16" => Type::of(Base::Float(FloatWidth::F16)),
            "BinaryFloat32" => Type::of(Base::Float(FloatWidth::F32)),
            "BinaryFloat128" => Type::of(Base::Float(FloatWidth::F128)),
            "Char" => Type::of(Base::Char),
            "Boolean" => Type::BOOLEAN,
            "String" => Type::STRING,
            "Duration" => Type::DURATION,
            "Regex" => Type::REGEX,
            "Never" => Type::of(Base::Never),
            _ => return None,
        })
    }
}

/// Whether `ty` has a stable C-ABI layout (roadmap Phase 4e, design D2,
/// `ADR-015-declaracion-extern.md`): `Boolean`, every fixed-width `Int`/
/// `UInt`, `Float32`/`Float64`, or another ABI-stable `Pointer<U>`, checked
/// recursively. `Void` is deliberately excluded — it is allowed only as an
/// `extern "C" fn`'s own return type, a check made at that call site rather
/// than here (task 4.3).
pub fn is_ffi_safe(ty: Type, pointer_types: &[Type]) -> bool {
    if ty.nullable {
        return false;
    }
    match ty.base {
        Base::Boolean | Base::Int(_) | Base::Float(_) => true,
        Base::Pointer(id) => pointer_types
            .get(id as usize)
            .is_some_and(|&inner| is_ffi_safe(inner, pointer_types)),
        Base::Tuple(_) => false,
        _ => false,
    }
}

/// Names a type for diagnostics.
///
/// Enums and functions need the checker's tables to be named, so this is a
/// free function taking a resolver rather than a method: a type alone does not
/// know what `Enum(3)` is called.
pub fn describe(ty: Type, names: &dyn TypeNames) -> String {
    let base = match ty.base {
        Base::Void => "Void".to_string(),
        Base::Int(width) => width.name().to_string(),
        Base::Float(width) => width.name().to_string(),
        Base::Decimal => "Float".to_string(),
        Base::Char => "Char".to_string(),
        Base::Boolean => "Boolean".to_string(),
        Base::String => "String".to_string(),
        Base::Duration => "Duration".to_string(),
        Base::Regex => "Regex".to_string(),
        Base::Null => "Null".to_string(),
        Base::Range(id) => format!("Range<{}>", describe(names.range_element(id), names)),
        Base::Never => "Never".to_string(),
        Base::Unknown => "<unknown>".to_string(),
        Base::Enum(id) => names.enum_name(id),
        Base::Function(id) => names.function_type(id),
        Base::Class(id) => names.class_name(id),
        Base::Contract(id) => names.contract_name(id),
        Base::Param(id) => names.type_param_name(id),
        Base::Instance(id) => names.instance_name(id),
        Base::ContractInstance(id) => names.contract_instance_name(id),
        Base::EnumInstance(id) => names.enum_instance_name(id),
        Base::Union(id) => names.union_name(id),
        Base::Pointer(id) => format!("Pointer<{}>", describe(names.pointer_element(id), names)),
        Base::Weak(id) => format!("Weak<{}>", describe(names.weak_element(id), names)),
        Base::NativeSlice(id) => format!(
            "NativeSlice<{}>",
            describe(names.native_slice_element(id), names)
        ),
        Base::NativeSliceMut(id) => format!(
            "NativeSliceMut<{}>",
            describe(names.native_slice_mut_element(id), names)
        ),
        Base::Dependent(id) => format!(
            "Dependent<{}>",
            describe(names.dependent_element(id), names)
        ),
        Base::Pin(id) => format!("Pin<{}>", describe(names.pin_element(id), names)),
        Base::Tuple(id) => names.tuple_name(id),
        Base::Array(id) => format!("Array<{}>", describe(names.array_element(id), names)),
        Base::List(id) => format!("List<{}>", describe(names.list_element(id), names)),
    };

    if ty.nullable {
        format!("{base}?")
    } else {
        base
    }
}

/// What [`describe`] needs in order to name the types it cannot name alone.
pub trait TypeNames {
    fn enum_name(&self, id: u32) -> String;
    fn function_type(&self, id: u32) -> String;
    fn class_name(&self, id: u32) -> String;
    fn contract_name(&self, id: u32) -> String;
    fn type_param_name(&self, id: u32) -> String;
    fn instance_name(&self, id: u32) -> String;
    fn contract_instance_name(&self, id: u32) -> String;
    fn enum_instance_name(&self, id: u32) -> String;
    fn union_name(&self, id: u32) -> String;
    fn pointer_element(&self, id: u32) -> Type;
    fn weak_element(&self, id: u32) -> Type;
    fn native_slice_element(&self, id: u32) -> Type;
    fn native_slice_mut_element(&self, id: u32) -> Type;
    fn dependent_element(&self, id: u32) -> Type;
    fn pin_element(&self, id: u32) -> Type;
    fn tuple_name(&self, id: u32) -> String;
    fn array_element(&self, id: u32) -> Type;
    fn list_element(&self, id: u32) -> Type;
    fn range_element(&self, id: u32) -> Type;
}

/// The signature of a function type, for closures and declared functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnType {
    pub params: Vec<Type>,
    pub returns: Type,
}

/// A generic class instantiated with concrete type arguments, as the checker
/// sees it. See [`Base::Instance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericInstance {
    pub class: u32,
    pub args: Vec<Type>,
}

/// A generic contract instantiated with concrete type arguments, as the
/// checker sees it. See [`Base::ContractInstance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericContractInstance {
    pub contract: u32,
    pub args: Vec<Type>,
}

/// A generic enum instantiated with concrete type arguments, as the checker
/// sees it. See [`Base::EnumInstance`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericEnumInstance {
    pub enum_id: u32,
    pub args: Vec<Type>,
}

/// A declared enum: traditional (a closed set of bare names) or algebraic
/// (each variant carries zero or more named, typed associated values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub name: String,
    pub variants: Vec<EnumVariantInfo>,
    /// Its own `<T>`, empty when the enum is not generic. Each id indexes the
    /// checker's type-parameter table, the same as [`ClassType::type_params`].
    pub type_params: Vec<u32>,
    /// Marked `share`, so files that import it may name it.
    pub shared: bool,
    /// Where it was declared, which is also which file owns it.
    pub span: zirk_diagnostics::Span,
}

impl EnumType {
    pub fn discriminant(&self, variant: &str) -> Option<u32> {
        self.variants
            .iter()
            .position(|v| v.name == variant)
            .map(|i| i as u32)
    }

    pub fn variant(&self, name: &str) -> Option<&EnumVariantInfo> {
        self.variants.iter().find(|v| v.name == name)
    }
}

/// One variant of an [`EnumType`], as the checker sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantInfo {
    pub name: String,
    /// Empty for a traditional variant or a bare algebraic one.
    pub associated: Vec<AssociatedFieldInfo>,
    pub span: zirk_diagnostics::Span,
}

/// One named, typed value an algebraic variant carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedFieldInfo {
    pub name: String,
    pub ty: Type,
}

/// One element of a tuple type, a value type without a name.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleType {
    pub elements: Vec<Type>,
}

/// A declared class: a nominal type with fields, constructors and methods.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassType {
    pub name: String,
    /// `Class` or `Record` — what rules apply, per
    /// `zirk_ast::ClassKind`.
    pub kind: zirk_ast::ClassKind,
    /// The class this one extends, by its id.
    pub base: Option<u32>,
    /// Every field, inherited ones first, in the order the hierarchy declares
    /// them.
    ///
    /// Flattening the chain here means a member lookup is one search rather
    /// than a walk, and — more importantly — it is what makes a subclass's
    /// layout start with its base's: reaching an inherited field is the same
    /// offset whoever is looking. Decision D2.
    pub fields: Vec<FieldInfo>,
    /// One entry per `construct`. More than one is admissible when their
    /// effective signatures differ.
    pub constructors: Vec<Vec<crate::scope::ParamInfo>>,
    /// Every method, inherited ones first, with an override replacing the
    /// entry it overrides so its index does not move.
    pub methods: Vec<MethodInfo>,
    /// The contracts this class satisfies, its base's included.
    pub contracts: Vec<u32>,
    /// The specific instantiation this class satisfies, for each generic
    /// contract in `contracts` it implements with concrete type arguments
    /// (task 6.9, e.g. `Iterable<Int32>`) — ids into the checker's
    /// `contract_instances` table. A class satisfying a non-generic contract
    /// has nothing here for it; `for ... in` reads this to find `T`.
    pub contract_instances: Vec<u32>,
    /// The `abstract class` requirement sets this class adopts with
    /// `implements`, by their id in this same table — its base's included.
    /// Separate from `contracts`: an abstract class is a `ClassType`, not a
    /// `ContractType`, so its id lives in a different space.
    pub abstract_bases: Vec<u32>,
    /// Its own `<T from A & B, U>`, empty when the class is not generic.
    ///
    /// Each id indexes the checker's type-parameter table, minted once when
    /// the class is registered — ahead of its members and ahead of any other
    /// class's fields, which may name `Box<Int32>` before `Box` itself is
    /// declared later in the file.
    pub type_params: Vec<u32>,
    /// Marked `share`, so files that import it may name it.
    pub shared: bool,
    /// Where it was declared, which is also which file owns it.
    pub span: zirk_diagnostics::Span,
}

impl ClassType {
    pub fn field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn method(&self, name: &str) -> Option<&MethodInfo> {
        self.methods.iter().find(|m| m.name == name)
    }
}

/// One method of a class, as the checker sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodInfo {
    pub name: String,
    pub params: Vec<crate::scope::ParamInfo>,
    pub returns: Type,
    pub visibility: zirk_ast::Visibility,
    pub span: zirk_diagnostics::Span,
    /// Its position among the class's methods, which is the name it is
    /// emitted under.
    pub index: usize,
    /// The class that declares the body this call reaches.
    pub owner: u32,
    /// The contract whose default body this entry adopts, if it is one.
    pub from_contract: Option<u32>,
    /// Whether some subclass redefines it.
    ///
    /// A method nobody overrides is called directly: that is most calls, and
    /// paying an indirection for all of them would be paying for a generality
    /// the program does not use. Decision D3.
    pub overridden: bool,
    /// Its own `throws Type (| Type)*`, empty when it declares none (roadmap
    /// Phase 4b) — see [`crate::scope::Signature::throws`].
    pub throws: Vec<Type>,
    /// Written `mut fn`, marking a method that mutates its receiver.
    pub is_mut: bool,
}

/// A declared interface or trait.
///
/// One type for both: `ZIRK_LANGUAGE_SPEC.md` section 7 separates them by a
/// single thing — a trait may supply bodies. Everything else is identical, and
/// two types would duplicate every rule to say the same.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractType {
    pub name: String,
    pub kind: zirk_ast::ContractKind,
    pub methods: Vec<ContractMethod>,
    /// Its own `<T>`, empty when the contract is not generic. Each id indexes
    /// the checker's type-parameter table, the same as
    /// [`ClassType::type_params`].
    pub type_params: Vec<u32>,
    pub shared: bool,
    pub span: zirk_diagnostics::Span,
}

impl ContractType {
    pub fn method(&self, name: &str) -> Option<&ContractMethod> {
        self.methods.iter().find(|m| m.name == name)
    }
}

/// One method a contract declares.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractMethod {
    pub name: String,
    pub params: Vec<crate::scope::ParamInfo>,
    pub returns: Type,
    pub span: zirk_diagnostics::Span,
    /// Whether the contract supplies a body, which only a trait may do.
    ///
    /// A class that does not declare the method adopts this one, so it is what
    /// makes a trait reusable rather than merely required.
    pub has_default: bool,
    /// Its position in the contract, which is its slot in the dispatch table.
    pub index: usize,
    /// Its own `throws Type (| Type)*`, empty when it declares none (roadmap
    /// Phase 4b) — see [`crate::scope::Signature::throws`].
    pub throws: Vec<Type>,
}

/// One field of a class, as the checker sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Type,
    pub visibility: zirk_ast::Visibility,
    pub mutability: zirk_ast::Mutability,
    pub span: zirk_diagnostics::Span,
    /// The class that declares it, which is not always the one that has it.
    pub owner: u32,
}

/// A generic type parameter, as the checker sees it.
///
/// It exists so a class's or function's own `<T from A & B>` is a real type
/// (`Base::Param`) inside its declaration rather than an unknown name.
/// `constraints` is recorded for later passes — verifying them at the use
/// site and inside the body is not implemented yet.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParamInfo {
    pub name: String,
    pub constraints: Vec<Type>,
    pub span: zirk_diagnostics::Span,
}

/// A type of the language that exists in the spec but not in this subset.
///
/// Recognizing them lets the diagnostic name the phase they arrive in, the same
/// way the parser does for constructs.
pub struct PendingType {
    pub name: &'static str,
    pub phase: Phase,
}

/// Looks up a type from a later phase by name.
///
/// The `Decimal*` family is deliberately absent: it is not a type family of
/// Zirk. An exact base-ten type may arrive one day as a standard-library type,
/// and it would be a different thing from `Float`. Announcing a phase for a
/// name the language does not have would teach a language that does not exist.
pub fn pending_type(name: &str) -> Option<PendingType> {
    // Phase 3 brings user-defined types and the roots they hang from.
    // `Never` is implemented now (roadmap Phase 4a, `Type::from_name`,
    // checked ahead of this list) — it is not new debt of Phase 3 left
    // open, just a type whose real home turned out to be here.
    const PHASE_3: &[&str] = &["Object"];
    // Phase 3b brings the rest of the scalars. The integer widths, the
    // binary floating family and `Char` are all implemented (`Type::from_name`,
    // checked ahead of this list) — `UInt` stays here on its own: the spec
    // never names it as an alias the way `Int`/`Integer` name `Int32`, so it
    // resolves to nothing even once every explicit width does.
    const PHASE_3B: &[&str] = &["UInt"];
    // Phase 4 brings errors and resources. `Result<T,E>` and `Pointer<T>` are
    // resolved by name; `Resource<E>` is registered as a native contract in
    // `checker.rs` and is no longer pending here.
    const PHASE_4: &[&str] = &[];
    // Phase 5 brings concurrency.
    const PHASE_5: &[&str] = &["Task", "Channel", "Thread", "Atomic"];
    // Phase 7 brings the stdlib, and with it the collection and temporal
    // families. They are compiler-known native types, not library objects:
    // what that phase adds is their implementation, not their existence.
    const PHASE_7: &[&str] = &[
        "Map",
        "Set",
        "Date",
        "Time",
        "DateTime",
        "Instant",
        "ZonedDateTime",
        "TimeZone",
        "Period",
    ];
    // Phase 7b brings the functional style. `Iterable<T>` and `Iterator<T>`
    // are registered as native contracts in `checker.rs` and are no longer
    // pending here; the remaining functional-style constructs are not types.
    const PHASE_7B: &[&str] = &[];

    for (names, phase) in [
        (PHASE_3, Phase::THREE),
        (PHASE_3B, Phase::THREE_B),
        (PHASE_4, Phase::FOUR),
        (PHASE_5, Phase::FIVE),
        (PHASE_7, Phase::SEVEN),
        (PHASE_7B, Phase::SEVEN_B),
    ] {
        if let Some(found) = names.iter().find(|n| **n == name) {
            return Some(PendingType { name: found, phase });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subset_types_resolve_by_name() {
        assert_eq!(Type::from_name("Int32"), Some(Type::INT32));
        assert_eq!(Type::from_name("Void"), Some(Type::VOID));
        assert_eq!(Type::from_name("Boolean"), Some(Type::BOOLEAN));
        assert_eq!(Type::from_name("String"), Some(Type::STRING));
    }

    #[test]
    fn a_type_outside_the_subset_does_not_resolve() {
        assert_eq!(Type::from_name("Whatever"), None);
    }

    #[test]
    fn valid_char_resolves() {
        assert_eq!(Type::from_name("Char"), Some(Type::of(Base::Char)));
        assert!(pending_type("Char").is_none());
    }

    #[test]
    fn valid_every_integer_width_resolves() {
        for name in [
            "Int8", "Int16", "Int32", "Int64", "Int128", "UInt8", "UInt16", "UInt32", "UInt64",
            "UInt128",
        ] {
            assert!(Type::from_name(name).is_some(), "`{name}` should resolve");
            assert!(
                pending_type(name).is_none(),
                "`{name}` should not be pending"
            );
        }
    }

    #[test]
    fn float_is_exact_and_binaryfloat_is_the_ieee_family() {
        // The plain name is the exact base-ten decimal type.
        assert_eq!(Type::from_name("Float"), Some(Type::FLOAT));
        assert_eq!(Type::from_name("Float").map(|t| t.base), Some(Base::Decimal));

        // The IEEE 754 binary family is `BinaryFloat*`.
        for name in [
            "BinaryFloat",
            "BinaryFloat16",
            "BinaryFloat32",
            "BinaryFloat64",
            "BinaryFloat128",
        ] {
            assert!(Type::from_name(name).is_some(), "`{name}` should resolve");
            assert!(pending_type(name).is_none());
        }
        assert_eq!(Type::from_name("BinaryFloat"), Some(Type::FLOAT64));

        // The former binary spellings no longer resolve (the checker turns
        // them into a redirect diagnostic).
        for name in ["Float16", "Float32", "Float64", "Float128"] {
            assert_eq!(Type::from_name(name), None, "`{name}` must not resolve");
        }
    }

    #[test]
    fn the_short_aliases_of_the_default_integer_resolve() {
        // An alias *is* its target: after resolution nothing distinguishes it.
        assert_eq!(Type::from_name("Int"), Some(Type::INT32));
        assert_eq!(Type::from_name("Integer"), Some(Type::INT32));
    }

    #[test]
    fn the_alias_of_an_unimplemented_type_stays_pending() {
        // `UInt` is `UInt32`, which does not exist yet.
        assert_eq!(Type::from_name("UInt"), None);
        assert_eq!(pending_type("UInt").map(|t| t.phase), Some(Phase::THREE_B));
    }

    #[test]
    fn types_from_later_phases_declare_their_phase() {
        assert_eq!(pending_type("UInt").map(|t| t.phase), Some(Phase::THREE_B));
        assert_eq!(pending_type("Object").map(|t| t.phase), Some(Phase::THREE));
        assert_eq!(pending_type("Task").map(|t| t.phase), Some(Phase::FIVE));
        assert_eq!(pending_type("Channel").map(|t| t.phase), Some(Phase::FIVE));
        assert_eq!(pending_type("Map").map(|t| t.phase), Some(Phase::SEVEN));
        assert!(pending_type("Array").is_none(), "`Array<T>` is implemented");
        assert!(pending_type("List").is_none(), "`List<T>` is implemented");
    }

    #[test]
    fn the_temporal_family_is_pending_not_unknown() {
        for name in [
            "Date",
            "Time",
            "DateTime",
            "Instant",
            "ZonedDateTime",
            "TimeZone",
            "Period",
        ] {
            assert_eq!(
                pending_type(name).map(|t| t.phase),
                Some(Phase::SEVEN),
                "`{name}` should announce its phase"
            );
        }
    }

    #[test]
    fn duration_resolves_to_its_own_type() {
        assert_eq!(Type::from_name("Duration"), Some(Type::DURATION));
        assert!(
            pending_type("Duration").is_none(),
            "`Duration` is implemented, not pending"
        );
    }

    #[test]
    fn the_decimal_family_is_not_a_type_of_the_language() {
        // It was removed from Zirk. Announcing a phase for it would teach a
        // language that does not exist.
        for name in ["Decimal", "Decimal16", "Decimal32", "Decimal64", "Dec"] {
            assert!(pending_type(name).is_none(), "`{name}` is not a Zirk type");
            assert_eq!(Type::from_name(name), None);
        }
    }

    #[test]
    fn a_type_that_does_not_exist_is_not_pending_either() {
        assert!(pending_type("Whatever").is_none());
    }

    #[test]
    fn there_are_no_implicit_conversions() {
        assert!(Type::INT32.accepts(Type::INT32));
        assert!(!Type::INT32.accepts(Type::STRING));
        assert!(!Type::BOOLEAN.accepts(Type::INT32));
    }

    #[test]
    fn unknown_is_compatible_with_everything() {
        // Prevents one error from cascading into a dozen derived ones.
        assert!(Type::INT32.accepts(Type::UNKNOWN));
        assert!(Type::UNKNOWN.accepts(Type::STRING));
    }

    #[test]
    fn int32_declares_its_range() {
        assert_eq!(
            Type::INT32.integer_range(),
            Some((-2_147_483_648, 2_147_483_647))
        );
    }

    #[test]
    fn a_value_widens_to_its_nullable_form() {
        assert!(Type::STRING.as_nullable().accepts(Type::STRING));
    }

    #[test]
    fn a_nullable_value_does_not_narrow_on_its_own() {
        // This is what `??` exists to make explicit.
        assert!(!Type::STRING.accepts(Type::STRING.as_nullable()));
    }

    #[test]
    fn null_fits_only_what_admits_absence() {
        assert!(Type::STRING.as_nullable().accepts(Type::NULL));
        assert!(!Type::STRING.accepts(Type::NULL));
        assert!(!Type::INT32.accepts(Type::NULL));
    }

    #[test]
    fn nullability_of_different_bases_does_not_make_them_compatible() {
        assert!(
            !Type::STRING
                .as_nullable()
                .accepts(Type::INT32.as_nullable())
        );
    }

    #[test]
    fn unifying_keeps_nullability_if_either_side_has_it() {
        assert_eq!(
            Type::STRING.unify(Type::STRING.as_nullable()),
            Some(Type::STRING.as_nullable())
        );
        assert_eq!(Type::STRING.unify(Type::STRING), Some(Type::STRING));
    }

    #[test]
    fn unifying_with_null_yields_the_nullable_form() {
        assert_eq!(
            Type::STRING.unify(Type::NULL),
            Some(Type::STRING.as_nullable())
        );
    }

    #[test]
    fn unifying_different_bases_fails() {
        assert_eq!(Type::STRING.unify(Type::INT32), None);
    }

    #[test]
    fn an_enum_knows_the_discriminant_of_its_variants() {
        let variant = |name: &str| EnumVariantInfo {
            name: name.into(),
            associated: Vec::new(),
            span: zirk_diagnostics::Span::new(0, 0),
        };
        let e = EnumType {
            name: "Direction".into(),
            variants: vec![variant("North"), variant("South")],
            type_params: Vec::new(),
            shared: false,
            span: zirk_diagnostics::Span::new(0, 0),
        };
        assert_eq!(e.discriminant("North"), Some(0));
        assert_eq!(e.discriminant("South"), Some(1));
        assert_eq!(e.discriminant("Up"), None);
    }
}
