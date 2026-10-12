## ADDED Requirements

### Requirement: Lowering preserves managed-reference metadata

IR lowering SHALL preserve the complete type-derived reference layout for roots, collection elements, nested values, and runtime-owned container entries. It SHALL not erase distinctions required for safe pointer tracing or typed storage.

#### Scenario: Nested reference-bearing value is lowered
- **WHEN** a value containing managed references is lowered into a collection or object field
- **THEN** IR carries sufficient layout metadata for runtime tracing after copies and mutation

### Requirement: Unsupported typed operations fail before backend emission

Lowering SHALL emit a stable diagnostic for any type/operation pair without a defined IR and runtime representation. It SHALL NOT construct an IR instruction whose only handling in the backend is an unchecked cast, `panic!`, or `unreachable!` for a source program that passed semantic analysis.

#### Scenario: Unsupported collection operation is lowered
- **WHEN** semantic analysis identifies an unsupported key, value, or element representation
- **THEN** lowering reports the diagnostic and no invalid instruction reaches LLVM emission
