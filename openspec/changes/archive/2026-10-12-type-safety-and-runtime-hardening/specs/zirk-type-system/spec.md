## ADDED Requirements

### Requirement: Accepted types have end-to-end representations

The type checker SHALL accept a type use only when semantic operations, IR representation, native ABI, runtime storage, and GC behavior are defined for that use. The checker SHALL reject unsupported generic collection keys, values, or elements before lowering, with a stable diagnostic that identifies the type and operation.

#### Scenario: Type passes checking but lacks a backend layout
- **WHEN** a collection operation uses a type with no supported LLVM/runtime layout
- **THEN** semantic analysis rejects it and compilation does not reach a code-generation panic

#### Scenario: Type has a complete runtime representation
- **WHEN** a generic value has defined storage, equality, and GC metadata
- **THEN** it remains accepted and those semantics are preserved through code generation
