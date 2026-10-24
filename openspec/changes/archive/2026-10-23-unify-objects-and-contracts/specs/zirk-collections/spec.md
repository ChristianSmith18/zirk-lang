## REMOVED Requirements

### Requirement: Collection elements retain nested managed references
**Reason**: Its aggregate list explicitly includes inline records, which are removed.
**Migration**: Apply collection tracing to reference-based objects and the remaining supported value aggregates.

### Requirement: Record field spread is distinct from collection spread
**Reason**: Field spread now applies to dynamic objects and no longer preserves nominal record shape.
**Migration**: Use object spread for named keys and iterable spread for collections.

## ADDED Requirements

### Requirement: Collections preserve dynamic object identity and tracing
Collections SHALL retain dynamic `Object` values as managed references. Copying a collection element SHALL preserve the referenced object's identity; collection growth, slicing, cloning, replacement, and garbage collection SHALL keep all reachable dynamic property values valid alongside references in tuples, enums, nullable values, and closures.

#### Scenario: List retains an object value
- **WHEN** a dynamic object is inserted into a list and the original binding goes out of scope
- **THEN** the object remains reachable through the list

#### Scenario: Collection copy preserves object identity
- **WHEN** a collection containing an object is copied under the collection's ordinary copy rules
- **THEN** any copied element still refers to the same object identity
