## ADDED Requirements

### Requirement: inmut::strict applies to field declarations
A field declared `inmut::strict` SHALL be unwritable through any projection, regardless of whether its container is `mut` or `inmut`.

#### Scenario: Strict field written through mut container
- **WHEN** a `record R { x: inmut::strict Int32 }` exists and code writes `r.x = 5` even though `r` is `mut`
- **THEN** compilation fails with a strict-field write diagnostic

### Requirement: inmut::strict rejects mutating method calls
An `inmut::strict` reference SHALL NOT be used as the receiver of a method declared `mut`, and the compiler SHALL reject such a call.

#### Scenario: Mutating call on strict reference
- **WHEN** `p: inmut::strict Point` and `p.move()` is called where `move` is declared `mut`
- **THEN** compilation fails with a strict-mutation diagnostic

#### Scenario: Non-mutating call on strict reference is allowed
- **WHEN** `p: inmut::strict Point` and `p.distance()` is called where `distance` does not mutate
- **THEN** the call type-checks and compiles

### Requirement: Native view provenance is tracked
The compiler SHALL track the known extent of a `NativeSlice<T>`/`NativeSliceMut<T>` constructed from `Pointer.from(place).as_slice(n)` and SHALL reject uses that exceed that extent.

#### Scenario: View exceeds known extent
- **WHEN** `Pointer.from(arr).as_slice(100)` is used and the compiler can prove `arr` has fewer than 100 elements
- **THEN** compilation fails with an extent diagnostic

#### Scenario: View within known extent is accepted
- **WHEN** `Pointer.from(arr).as_slice(100)` is used and `arr` has at least 100 elements
- **THEN** the expression type-checks
