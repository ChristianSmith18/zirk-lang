# zirk-memory-safety Delta Spec

## MODIFIED Requirements

### Requirement: inmut::strict rejects mutating method calls
An `inmut::strict` reference SHALL NOT be used as the receiver of a method that mutates `this`. Since `mut` is no longer a written method modifier, the compiler SHALL infer mutating methods: a method mutates if its body writes to `this.*` (directly or through a place rooted at `this`) or calls another method on `this` that mutates, computed by fixed-point over the call graph. The diagnostic for a rejected call SHALL identify the mutation chain (call → callee → the `this` write) so the reason is visible without any user-written marker. Methods with no analyzable body (for example `extern "C"`) SHALL be assumed mutating for this check.

#### Scenario: Mutating call on strict reference
- **WHEN** `p: inmut::strict Point` and `p.move()` is called where `move` writes `this.x`
- **THEN** compilation fails with a strict-mutation diagnostic that names the `this` write

#### Scenario: Transitively mutating call on strict reference
- **WHEN** `p.reset()` is called on a strict reference and `reset` calls `helper`, which writes `this.count`
- **THEN** compilation fails and the diagnostic shows the chain `reset → helper → this.count = ...`

#### Scenario: Non-mutating call on strict reference is allowed
- **WHEN** `p: inmut::strict Point` and `p.distance()` is called where `distance` only reads `this`
- **THEN** the call type-checks and compiles

#### Scenario: Plain inmut receiver is unrestricted
- **WHEN** `p: inmut Point` (not strict) calls a mutating method
- **THEN** the call compiles normally
