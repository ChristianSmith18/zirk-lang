# zirk-generics Specification

## Purpose
TBD - created by archiving change document-refined-core-language-semantics. Update Purpose after archive.
## Requirements
### Requirement: Generic declarations, constraints, defaults, and inference
Generic parameters SHALL use `<T>`, combined constraints SHALL use `T from A & B`, and trailing parameters MAY declare defaults satisfying their constraints. Inference SHALL use arguments, receiver, expected result, callable context, and constraints and SHALL fail rather than choose an arbitrary solution.

#### Scenario: Multiple missing constraints
- **WHEN** a concrete type is supplied for `T from Clone & Serializable` and lacks Serializable
- **THEN** the call-site diagnostic identifies T and the missing Serializable contract

### Requirement: Declared generic variance
Generic parameters SHALL be invariant by default. `out T` SHALL be legal only in covariant output positions, `in T` only in contravariant input positions, and any mutable attribute use SHALL require invariance. `Fn` SHALL retain its intrinsic parameter/result variance.

#### Scenario: Mutable List invariance
- **WHEN** Dog extends Animal and `List<Dog>` is supplied as `List<Animal>`
- **THEN** compilation fails because List permits insertion

### Requirement: Recursive generics and managed indirection
Verifiable recursive constraints SHALL be accepted. Infinite inline recursive layout SHALL fail and MAY be broken with managed `Box<T>`. Associated types and higher-kinded parameters SHALL remain outside the initial language.

#### Scenario: Recursive record requires Box
- **WHEN** a record directly stores its own type without indirection
- **THEN** compilation fails with an infinite-size diagnostic and suggests `Box<T>`

### Requirement: Generic identity and monomorphization
Concrete instantiations SHALL retain distinct static and runtime type identity. Generic bodies SHALL be checked once against constraints; portable IR SHALL retain generic information and final builds MAY monomorphize/share code only when ABI and observable semantics remain unchanged.

#### Scenario: Distinct runtime instantiations
- **WHEN** runtime type identity is requested for List<String> and List<Int32>
- **THEN** the two complete instantiations remain distinguishable

