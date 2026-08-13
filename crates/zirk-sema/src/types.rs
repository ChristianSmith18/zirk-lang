//! The types of the Phase 1 subset.
//!
//! Only what `ZIRK_ROADMAP.md` Phase 1 fixes is modelled: `Void`, `Int32`,
//! `Boolean` and `String`. The remaining families of `ZIRK_LANGUAGE_SPEC.md`
//! section 3 are recognized by name so the diagnostic can say they exist but are
//! not implemented, rather than "unknown type".

/// A type of the subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    Void,
    Int32,
    Boolean,
    String,
    /// Assigned to expressions whose type could not be determined.
    ///
    /// It exists so one type error does not cascade into a dozen derived ones:
    /// anything involving an `Unknown` is silently accepted, because the real
    /// error was already reported at its origin.
    Unknown,
}

impl Type {
    pub const fn as_str(self) -> &'static str {
        match self {
            Type::Void => "Void",
            Type::Int32 => "Int32",
            Type::Boolean => "Boolean",
            Type::String => "String",
            Type::Unknown => "<unknown>",
        }
    }

    /// Resolves a type from the name written in the source.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "Void" => Type::Void,
            "Int32" => Type::Int32,
            "Boolean" => Type::Boolean,
            "String" => Type::String,
            _ => return None,
        })
    }

    /// Whether two types are compatible for assignment.
    ///
    /// There are no implicit conversions (`LANGUAGE_SPEC` section 3), so
    /// compatibility is equality. `Unknown` is compatible with everything to
    /// avoid cascading errors.
    pub fn accepts(self, other: Type) -> bool {
        self == other || self == Type::Unknown || other == Type::Unknown
    }

    /// Range of values representable by the type, for integer literals.
    pub const fn integer_range(self) -> Option<(i128, i128)> {
        match self {
            Type::Int32 => Some((i32::MIN as i128, i32::MAX as i128)),
            _ => None,
        }
    }
}

/// A type of the language that exists in the spec but not in this subset.
///
/// Recognizing them lets the diagnostic name the phase they arrive in, the same
/// way the parser does for constructs.
pub struct PendingType {
    pub name: &'static str,
    pub phase: u8,
}

/// Looks up a type from a later phase by name.
pub fn pending_type(name: &str) -> Option<PendingType> {
    // Phase 3 brings the rest of the type system: the remaining numeric
    // families, `Char`, collections and user-defined types.
    const PHASE_3: &[&str] = &[
        "Int8",
        "Int16",
        "Int64",
        "Int128",
        "Int",
        "Integer",
        "UInt8",
        "UInt16",
        "UInt32",
        "UInt64",
        "UInt128",
        "Decimal16",
        "Decimal32",
        "Decimal64",
        "Decimal128",
        "Dec",
        "Decimal",
        "Char",
        "Object",
        "Never",
        "Null",
        "List",
        "Map",
        "Set",
        "Array",
        "Range",
    ];
    // Phase 4 brings errors and resources.
    const PHASE_4: &[&str] = &["Result", "Pointer", "Resource"];
    // Phase 5 brings concurrency.
    const PHASE_5: &[&str] = &["Task", "Channel", "Thread", "Atomic"];

    for (names, phase) in [(PHASE_3, 3u8), (PHASE_4, 4), (PHASE_5, 5)] {
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
        assert_eq!(Type::from_name("Int32"), Some(Type::Int32));
        assert_eq!(Type::from_name("Void"), Some(Type::Void));
        assert_eq!(Type::from_name("Boolean"), Some(Type::Boolean));
        assert_eq!(Type::from_name("String"), Some(Type::String));
    }

    #[test]
    fn a_type_outside_the_subset_does_not_resolve() {
        assert_eq!(Type::from_name("Int64"), None);
        assert_eq!(Type::from_name("Whatever"), None);
    }

    #[test]
    fn types_from_later_phases_declare_their_phase() {
        assert_eq!(pending_type("Int64").map(|t| t.phase), Some(3));
        assert_eq!(pending_type("Result").map(|t| t.phase), Some(4));
        assert_eq!(pending_type("Channel").map(|t| t.phase), Some(5));
    }

    #[test]
    fn a_type_that_does_not_exist_is_not_pending_either() {
        assert!(pending_type("Whatever").is_none());
    }

    #[test]
    fn there_are_no_implicit_conversions() {
        assert!(Type::Int32.accepts(Type::Int32));
        assert!(!Type::Int32.accepts(Type::String));
        assert!(!Type::Boolean.accepts(Type::Int32));
    }

    #[test]
    fn unknown_is_compatible_with_everything() {
        // Prevents one error from cascading into a dozen derived ones.
        assert!(Type::Int32.accepts(Type::Unknown));
        assert!(Type::Unknown.accepts(Type::String));
    }

    #[test]
    fn int32_declares_its_range() {
        assert_eq!(
            Type::Int32.integer_range(),
            Some((-2_147_483_648, 2_147_483_647))
        );
    }
}
