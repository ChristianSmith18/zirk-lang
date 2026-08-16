## ADDED Requirements

### Requirement: Failure effects and Result consumption are checked
The checker SHALL require explicit exceptions to be handled or declared, SHALL propagate declared exception sets through calls and callable compatibility, SHALL permit documented implicit `RuntimeError` exceptions without signature declaration, and SHALL reject an unconsumed `Result` except through explicit discard.

#### Scenario: Callable throws too broadly
- **WHEN** a callable declaring `throws StorageError` is assigned to `Fn() => Void`
- **THEN** assignment fails because the target does not permit that explicit exception

### Requirement: Resource responsibility is flow-sensitive
The checker SHALL track managed, transferred, closed, dependent, and abandoned resource responsibility through branches, returns, containers, closures, tasks, and exceptional exits. It SHALL reject statically provable duplicate close, use-after-transfer, illegal escape, non-cloneable projection, and leak paths.

#### Scenario: Every branch transfers or closes
- **WHEN** all control-flow paths either close or transfer one resource responsibility
- **THEN** the function satisfies resource lifetime checking

### Requirement: Permission effects propagate outside surface callable syntax
The checker SHALL retain compiler-internal permission-effect metadata on declarations and callable values, infer it transitively through higher-order calls, and report a path from entry point to privileged API. Permission effects SHALL NOT alter the written `Fn(P...) => R` grammar.

#### Scenario: Higher-order permission propagation
- **WHEN** a permission-free wrapper invokes a callback whose concrete value reads a secret
- **THEN** the call site and application acquire the secret-read requirement in compiler metadata
