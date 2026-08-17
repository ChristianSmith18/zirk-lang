# zirk-contracts Specification

## Purpose
TBD - created by archiving change document-refined-core-language-semantics. Update Purpose after archive.
## Requirements
### Requirement: Interface, trait, and abstract requirements
Interfaces SHALL contain behavior signatures only. Traits SHALL contain behavior requirements and reusable method bodies but no attributes or constructors. Abstract classes SHALL contain nominal attribute and abstract-method requirements but no bodies. All three SHALL be adopted through `implements`; a declaration SHALL satisfy every compatible requirement explicitly.

#### Scenario: Trait state rejected
- **WHEN** a trait declares an instance attribute
- **THEN** compilation fails and recommends a required getter/setter method or abstract-class requirement

### Requirement: Contract composition and conflict resolution
Interfaces MAY implement interfaces; traits MAY implement interfaces and traits; abstract classes MAY implement abstract classes and interfaces. Cycles and incompatible same-name signatures SHALL fail. Conflicting trait defaults SHALL require `override fn` and MAY select an implementation with `TraitName.super.method()`.

#### Scenario: Explicit trait selection
- **WHEN** two adopted traits define `print()` and the class overrides it using `JsonPrintable.super.print()`
- **THEN** the selected default is called without declaration-order precedence

### Requirement: Explicit capability derivation
Classes SHALL NOT derive equality, hashing, or cloning silently. Records, value classes, tuples, and enums MAY request explicit derivation only when every component satisfies the required capability; derived cloning SHALL be deep.

#### Scenario: Uncloneable component
- **WHEN** Clone derivation is requested for a value containing an uncloneable resource
- **THEN** compilation fails and identifies the component and missing capability

