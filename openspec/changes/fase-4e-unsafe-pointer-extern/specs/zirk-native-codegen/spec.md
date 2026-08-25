## ADDED Requirements

### Requirement: `extern "C" fn` lowers to an external symbol declaration

The backend SHALL translate an `extern "C" fn` declaration to an LLVM external function declaration using the C calling convention, with no defined body. Resolving the declared symbol at link time SHALL use whatever the system linker already provides — the compiler SHALL NOT introduce a native-library-linking manifest as part of resolving it.

#### Scenario: Declaration becomes an external symbol
- **WHEN** the backend translates `extern "C" fn strlen(s: Pointer<Byte>): UInt64;`
- **THEN** the emitted LLVM module declares `strlen` as an external function with the C calling convention and no body

#### Scenario: Unresolved extern symbol fails at link time
- **WHEN** a program declares and calls an `extern "C" fn` whose symbol the linker cannot resolve
- **THEN** compilation fails at the link step with the linker's own diagnostic, the same mechanism an ordinary link failure already reports

### Requirement: `Pointer<T>` operations lower to native pointer instructions

The backend SHALL translate `Pointer<T>` construction, read, write, element/byte offset, and cast to native LLVM pointer operations (address-of, load, store, `getelementptr`, and pointer cast respectively), without introducing a managed indirection layer.

#### Scenario: Pointer write lowers to a store
- **WHEN** the backend translates `pointer.write(value)`
- **THEN** the emitted code is a direct LLVM store through the pointer operand, typed by `T`

#### Scenario: Element offset lowers to `getelementptr`
- **WHEN** the backend translates `pointer.offset(n)`
- **THEN** the emitted code computes the address with `getelementptr` in units of `T`, not raw bytes
