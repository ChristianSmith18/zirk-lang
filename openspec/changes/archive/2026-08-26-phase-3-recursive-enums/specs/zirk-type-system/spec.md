## ADDED Requirements

### Requirement: Enum declaration order is independent of self- and mutual references

Declaration order between an enum and a class, or between two enums, that reference each other SHALL NOT matter. An enum's variant payload naming its own type, or another enum's, directly and with no indirection in between SHALL be rejected at compile time rather than compiled into an unbounded-size representation.

#### Scenario: Enum and class reference each other regardless of order
- **WHEN** an enum's variant payload names a class, and that class has a field naming the enum back, in either declaration order
- **THEN** both declarations resolve correctly

#### Scenario: A directly self-referential enum field is rejected, not miscompiled
- **WHEN** an enum's variant payload names its own type with no indirection in between (for example `enum IntList { Nil, Cons(head: Int32, tail: IntList) }`)
- **THEN** compilation fails with a diagnostic naming the cycle, rather than crashing or producing an unbounded-size type

NOTE (not applied to the main spec): a self-referential enum's variant payload actually being constructible and pattern-matchable — the scenario this requirement's own name might suggest — is not delivered by this change. It requires automatic heap indirection ("boxing") for a self-referential field: a new `IrType` case, a runtime allocation kind, and GC integration, none of which exists today. That is its own future change; this change only makes the hazard a clean diagnostic instead of a compiler crash, and delivers the real, unrelated declaration-order independence for the non-cyclic case (an enum and a class referencing each other).
