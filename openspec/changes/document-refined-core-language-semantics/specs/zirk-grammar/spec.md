## ADDED Requirements

### Requirement: Refined core syntax surface
The parser SHALL recognize `Fn(P...) => R` and `Function(P...) => R`, `override fn`, abstract classes adopted with `implements`, combined `from A & B` constraints, `in/out` generic variance, Tuple type/index syntax, collection literals, `as?`, and the accepted guard-free pattern forms. It SHALL reject property declarations, standalone override, enum user methods, match guards, and enum destructuring bindings.

#### Scenario: Callable type annotation
- **WHEN** source contains `mut print: Fn(String) => Void = stdout.println`
- **THEN** the parser produces a mutable binding whose annotation is a callable type

#### Scenario: Removed property syntax
- **WHEN** source declares `property name: String`
- **THEN** it receives a targeted diagnostic recommending an attribute plus `get_name`/`set_name` methods

### Requirement: Slice omission syntax
The parser SHALL preserve independently omitted start, end, and step components in `[:]`, `[::]`, `[n:]`, `[:w]`, `[n:w]`, `[::k]`, and reverse forms so semantic analysis can apply direction-sensitive defaults.

#### Scenario: Fully omitted slice
- **WHEN** `[::]` is parsed
- **THEN** start, end, and step are represented as omitted rather than fabricated source literals
