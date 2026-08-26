## ADDED Requirements

### Requirement: Generic substitution recurses into a nested instantiation

The checker SHALL replace a type parameter appearing inside a nested generic instantiation (an enum or contract instantiation named as part of a parameter's declared type), not only when the parameter's own declared type is directly the type parameter itself, and SHALL infer a type parameter's solution from an argument's type the same way when the declared parameter type nests it.

#### Scenario: Inference through a nested generic parameter type
- **WHEN** a generic function declares a parameter of type `Option2<T>` and is called with an argument of type `Option2<Int32>`
- **THEN** `T` is inferred as `Int32`

#### Scenario: Substitution replaces a type parameter inside a nested instantiation
- **WHEN** a generic enum's own variant payload names another generic instantiation containing the enum's type parameter (for example `Bar<Baz<T>>`)
- **THEN** substituting `T` with a concrete type produces the fully-substituted nested instantiation, not a partially-substituted one
