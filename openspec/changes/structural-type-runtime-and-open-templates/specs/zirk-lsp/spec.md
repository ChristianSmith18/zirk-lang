## ADDED Requirements

### Requirement: LSP presents structural composite and open-template analysis
The language server SHALL use the compiler's canonical structural-union/intersection and open-template inference results for diagnostics, hover, signature information, and completion. It SHALL not implement a separate approximation that disagrees with CLI checking. Diagnostics SHALL identify the relevant alternative, inferred interpolation, failed constraint, or computation limit while retaining source locations.

#### Scenario: Hover shows the inferred callback property type on a generic contract method
- **WHEN** a callback is passed to `person.on("ageChanged", ...)` through a `Watched<Person>` receiver
- **THEN** hover and inlay information show the indexed property type inferred from the event literal

#### Scenario: Completion follows a narrowed structural alternative
- **WHEN** completion is requested inside a branch narrowed to one structural union alternative
- **THEN** it offers members valid on that alternative and does not offer members exclusive to removed alternatives

#### Scenario: Invalid open-template method call matches CLI diagnostic
- **WHEN** a method call's string literal does not infer a key satisfying the declared constraint
- **THEN** the LSP reports the same diagnostic category and source range as CLI analysis

#### Scenario: Signature help shows the specialized generic method signature
- **WHEN** signature help is requested at the callback argument of `person.on("ageChanged", ...)`
- **THEN** the displayed method signature reflects the receiver's contract arguments and the inferred `K`, including the resolved callback type

#### Scenario: Ambiguous structural construction is explained
- **WHEN** an object literal matches multiple structural union alternatives without sufficient context
- **THEN** the LSP reports the ambiguity and identifies the viable alternatives
