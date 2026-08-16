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

/// The part of a type that is not its nullability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Base {
    Void,
    Int32,
    Boolean,
    String,
    /// The type of the `null` literal, assignable to any nullable type.
    Null,
    /// A declared enum, identified by its index in the checker's table.
    Enum(u32),
    /// A function value, identified by its index in the checker's table.
    Function(u32),
    /// The result of `a..b`, iterable by `for ... in`.
    Range,
    /// Assigned to expressions whose type could not be determined.
    ///
    /// It exists so one type error does not cascade into a dozen derived ones:
    /// anything involving an `Unknown` is silently accepted, because the real
    /// error was already reported at its origin.
    Unknown,
}

impl Type {
    pub const VOID: Type = Type::of(Base::Void);
    pub const INT32: Type = Type::of(Base::Int32);
    pub const BOOLEAN: Type = Type::of(Base::Boolean);
    pub const STRING: Type = Type::of(Base::String);
    pub const NULL: Type = Type::of(Base::Null);
    pub const RANGE: Type = Type::of(Base::Range);
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

        // `null` fits anything that admits absence, and nothing else.
        if matches!(other.base, Base::Null) {
            return self.admits_null();
        }

        if self.base != other.base {
            return false;
        }

        // `T` fits `T?`; `T?` does not fit `T`.
        self.nullable || !other.nullable
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

    /// Range of values representable by the type, for integer literals.
    pub const fn integer_range(self) -> Option<(i128, i128)> {
        match self.base {
            Base::Int32 => Some((i32::MIN as i128, i32::MAX as i128)),
            _ => None,
        }
    }

    /// Resolves a non-nullable type from the name written in the source.
    ///
    /// Enums are not here: they are declared by the program, so the checker
    /// resolves them against its own table.
    ///
    /// `Int` and `Integer` resolve here rather than being deferred, because an
    /// alias *is* its target: both name `Int32`, which has existed since Phase
    /// 1. Deferring them deferred a spelling, not a capability. After
    /// resolution nothing distinguishes them, which is what being an alias
    /// means. `UInt` is not here because `UInt32` is not implemented.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "Void" => Type::VOID,
            "Int32" | "Int" | "Integer" => Type::INT32,
            "Boolean" => Type::BOOLEAN,
            "String" => Type::STRING,
            _ => return None,
        })
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
        Base::Int32 => "Int32".to_string(),
        Base::Boolean => "Boolean".to_string(),
        Base::String => "String".to_string(),
        Base::Null => "Null".to_string(),
        Base::Range => "Range".to_string(),
        Base::Unknown => "<unknown>".to_string(),
        Base::Enum(id) => names.enum_name(id),
        Base::Function(id) => names.function_type(id),
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
}

/// The signature of a function type, for closures and declared functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnType {
    pub params: Vec<Type>,
    pub returns: Type,
}

/// A declared enum: a closed set of names, without associated data.
///
/// Associated data is the extension Phase 3 adds, per decision D1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub name: String,
    pub variants: Vec<String>,
    /// Marked `share`, so files that import it may name it.
    pub shared: bool,
    /// Where it was declared, which is also which file owns it.
    pub span: zirk_diagnostics::Span,
}

impl EnumType {
    pub fn discriminant(&self, variant: &str) -> Option<u32> {
        self.variants
            .iter()
            .position(|v| v == variant)
            .map(|i| i as u32)
    }
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
    const PHASE_3: &[&str] = &["Object", "Never"];
    // Phase 3b brings the rest of the scalars: the remaining integer widths,
    // the binary floating family and `Char`.
    const PHASE_3B: &[&str] = &[
        "Int8", "Int16", "Int64", "Int128", "UInt8", "UInt16", "UInt32", "UInt64", "UInt128",
        "UInt", "Float16", "Float32", "Float64", "Float128", "Float", "Char",
    ];
    // Phase 4 brings errors and resources.
    const PHASE_4: &[&str] = &["Result", "Pointer", "Resource"];
    // Phase 5 brings concurrency.
    const PHASE_5: &[&str] = &["Task", "Channel", "Thread", "Atomic"];
    // Phase 7 brings the stdlib, and with it the collection and temporal
    // families. They are compiler-known native types, not library objects:
    // what that phase adds is their implementation, not their existence.
    const PHASE_7: &[&str] = &[
        "List",
        "Map",
        "Set",
        "Array",
        "Date",
        "Time",
        "DateTime",
        "Instant",
        "ZonedDateTime",
        "TimeZone",
        "Duration",
        "Period",
        "Regex",
    ];
    // Phase 7b brings the functional style.
    const PHASE_7B: &[&str] = &["Iterable", "Iterator"];

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
        assert_eq!(Type::from_name("Int64"), None);
        assert_eq!(Type::from_name("Whatever"), None);
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
        assert_eq!(pending_type("Int64").map(|t| t.phase), Some(Phase::THREE_B));
        assert_eq!(pending_type("Object").map(|t| t.phase), Some(Phase::THREE));
        assert_eq!(pending_type("Result").map(|t| t.phase), Some(Phase::FOUR));
        assert_eq!(pending_type("Channel").map(|t| t.phase), Some(Phase::FIVE));
    }

    #[test]
    fn the_float_family_is_pending_not_unknown() {
        for name in ["Float", "Float16", "Float32", "Float64", "Float128"] {
            assert_eq!(
                pending_type(name).map(|t| t.phase),
                Some(Phase::THREE_B),
                "`{name}` should announce its phase"
            );
        }
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
            "Duration",
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
        let e = EnumType {
            name: "Direction".into(),
            variants: vec!["North".into(), "South".into()],
            shared: false,
            span: zirk_diagnostics::Span::new(0, 0),
        };
        assert_eq!(e.discriminant("North"), Some(0));
        assert_eq!(e.discriminant("South"), Some(1));
        assert_eq!(e.discriminant("Up"), None);
    }
}
