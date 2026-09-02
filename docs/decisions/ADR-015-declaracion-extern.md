# ADR-015 — `extern` syntax and scope: how a native function is declared

- **Status:** accepted
- **Date:** August 24, 2026
- **Phase:** 4e

## Context

`MEMORY_AND_UNSAFE_SEMANTICS.md` §9 includes, in the closed set of unsafe operations, "calling an `unsafe fn` or unsafe native declaration". `ZIRK_LANGUAGE_SPEC.md` line 581-582 says, as its only sentence about interoperability: "Native interoperability uses the C ABI as its stable boundary; C++ and Rust expose `extern "C"` wrappers." None of the normative specs say how, on the Zirk side, a native function declaration is written — there is no `extern` keyword, not even reserved in the lexer. This is a real gap, not an implementation omission: nobody has decided the syntax yet.

This ADR settles that decision for the minimum needed to make `unsafe {}`/`Pointer<T>` genuinely useful, without building the full native-library linking system (which belongs to Phase 6, together with the `requires`/`during: build` permission manifest).

## Decision

### Syntax: single-item declaration, no grouping block

```zirk
extern "C" fn strlen(s: Pointer<Byte>): UInt64;
extern "C" fn memcpy(dst: Pointer<Byte>, src: Pointer<Byte>, n: UInt64): Pointer<Byte>;
```

An `extern "C" fn` is a top-level item, with no body, terminated by `;` — the same shape as a declared `fn`, except it has no block. The string literal in the calling-convention position (`"C"`) is the only one supported today; the position is reserved so the syntax doesn't need to change if another one is needed later (`"system"`, etc., as Rust does), but the compiler rejects any value other than `"C"`.

**Alternative considered — a grouping block `extern "C" { fn a(...); fn b(...); }`** (Rust-style). Rejected for now: it adds a new grouping form to the language (Zirk has no item blocks anywhere else in the grammar) just to save repeating `extern "C"` a few times per file. If the volume of native declarations grows, this can be added later without breaking the single-item shape.

### Calling an `extern` declaration requires `unsafe` and `commit`

Every call to an `extern` function — no exceptions, regardless of whether the programmer knows it's pure — requires being inside an `unsafe {}` block (it's operation 2 of the closed set) and, additionally, inside a nested `commit {}` (`MEMORY_AND_UNSAFE_SEMANTICS.md` §12 already lists "unknown-effect native library calls" among what requires `commit`). The compiler does not attempt to distinguish a "pure" native call from one with effects: **every** `extern` call is treated as potentially irreversible. This is the simplest and safest reading of the normative text, and it avoids inventing a taxonomy of native effects that no spec asks for yet.

### Type surface: only what has a stable C-ABI layout

The types allowed in an `extern "C" fn` signature (parameters and return) are: `Void` (return only), `Boolean`, `Int8`/`Int16`/`Int32`/`Int64`, `UInt8`/`UInt16`/`UInt32`/`UInt64`, `Float32`/`Float64`, and `Pointer<T>` where `T` is, recursively, one of these same types or another `Pointer<U>`. **`String`, classes, records, enums, and any managed type are excluded** — none of them has a stable binary layout on the C side without a marshaling layer that this ADR does not build. A program that needs to pass text to a native function does so explicitly, via `Pointer<Byte>` and the `NativeSlice<Byte>` operations that `unsafe` already exposes — the conversion is the program's responsibility, not an implicitly C-compatible `String`.

### Symbol resolution: the system linker, no new manifest

An `extern "C" fn` declaration carries no way to tell the compiler "also link this library". It relies entirely on what the linker already resolves by default on the platform — typically libc and whatever `zirk-runtime` itself already links transitively. Declaring a function that fails to resolve at link time produces the same failure that `zirk-native-codegen`'s "Link failure" already diagnoses today (symbol not found, linker output included). **Linking an additional native library is explicitly out of scope for this ADR** — it's the natural extension of the `requires`/`during: build` that `zirk-permissions` already reserves for external dependencies, but building that integration is Phase 6 work, not part of this Phase 4e piece.

### No integration with the permission system yet

`MEMORY_AND_UNSAFE_SEMANTICS.md` §12 says "Permissions are still checked before the effect" for what enters `commit {}`. Permissions do not exist as a runtime concept yet (already confirmed in `fase-4d-callables`: "permissions do not exist as a runtime concept yet"), so there is nothing to check today. This ADR does not invent a new permission scope for "call native function X" — when Phase 6 builds the real permission system, `extern`/`commit` is where it will hook in, but that is that phase's work, not this one's.

## Consequences

- `unsafe {}`/`Pointer<T>` no longer depend on an undecided syntax question: a call to a real libc function (`strlen`, `memcpy`, raw `malloc`/`free`, etc.) can be written, compiled, and linked today, without waiting for Phase 6.
- A Zirk program that declares `extern "C" fn something_that_does_not_exist(): Void;` and calls it fails at link time, with the same diagnostic mechanism as any other link failure today — honest behavior, not an `unsafe` that pretends to work.
- The deliberately narrow type surface (no `String`, no objects) is a real, known limitation: interoperating with a C API that expects `char*`/structs today requires hand-writing it with `Pointer<Byte>` and explicit arithmetic. This is the correct starting point to avoid committing to automatic marshaling that no spec has designed.
- When Phase 6 builds the native-library manifest, this ADR is the extension point: it adds *where* to link, not *how* a native function is declared — that shape should not need to change.
