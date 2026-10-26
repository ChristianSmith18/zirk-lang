## ADDED Requirements

### Requirement: Native code generation preserves union value semantics
Native code generation SHALL represent every accepted heterogeneous union with a target-correct tag and payload layout, preserve alternative identity through parameters, returns, storage, captures, and package calls, and dispatch without exposing raw representation to safe source code. The implementation SHALL use inline storage where its measured layout and ABI are safe; any boxed fallback SHALL preserve the same semantics and be explicit in performance evidence. Intersections SHALL not allocate a second runtime object solely to combine static views.

#### Scenario: Union of distinct scalar representations
- **WHEN** a native function accepts and returns String | Int32
- **THEN** both alternatives round-trip correctly without type confusion

#### Scenario: Union with a managed object alternative
- **WHEN** a union contains a scalar alternative and a managed-reference alternative
- **THEN** tag dispatch and root enumeration identify the active representation correctly

#### Scenario: Union remains internal to a package ABI
- **WHEN** a compiled package exposes a union-typed callable to another compatible package
- **THEN** both sides agree on canonical alternatives, tag meaning, payload layout, and calling convention

#### Scenario: Target ABI differs
- **WHEN** two supported native targets classify a union aggregate differently
- **THEN** generated code uses each target's correct internal ABI while preserving the same Zirk semantics
