## 1. Lock the language contract and migration surface

- [x] 1.1 Review the delta requirements against the existing capability specs and remove every remaining normative `record`/`interface` behavior that conflicts with `contract` and dynamic `Object`; verify with a scoped `rg` audit and `openspec validate`.
- [x] 1.2 Update the authoritative language specification and ADRs for `contract`, `Object`, `Dynamic`, dynamic key syntax, alias-visible sealing, mutability, identity, JSON, errors, and breaking migration; verify all normative examples match the delta specs.
- [x] 1.3 Define the migration diagnostics and map old record/interface use cases to contract plus Object, class, tuple, enum, generic type, or Dynamic; verify with migration fixtures covering each replacement path.

## 2. Implement syntax and static semantics

- [x] 2.1 Add `contract` declarations with property and operation signatures and remove active `record`/`interface` declaration syntax; verify parser and diagnostic coverage with `cargo test -p zirk-lexer` and `cargo test -p zirk-parser`.
- [x] 2.2 Parse bare identifier keys, quoted keys, interpolated keys, computed string access, dynamic object patterns, spread, and rest; verify AST snapshots and malformed-input diagnostics with `cargo test -p zirk-parser`.
- [x] 2.3 Add the `Dynamic` tagged type, `Object` reference-object typing, class-to-Object widening, checked narrowing, structural contract checks, and contract-view member lookup; verify positive and negative typing scenarios with `cargo test -p zirk-sema`.
- [x] 2.4 Implement mutability and key-set sealing rules across aliases, class instances, contract views, and repeated contract checks; verify strict alias rejection and successful mutable-value updates with `cargo test -p zirk-sema`.
- [x] 2.5 Implement callable-property conformance and generic contract substitution; verify method/property dispatch and generic compatibility with `cargo test -p zirk-sema` and `cargo test -p zirk-ir`.
- [x] 2.6 Retire record-specific equality, copying, destructuring, cloning, inline layout, and pointer semantics while preserving tuple/enum value behavior; verify migration diagnostics and unaffected value aggregates with the relevant parser, sema, and IR suites.

## 3. Implement runtime objects and error behavior

- [x] 3.1 Implement managed string-keyed object storage with tagged Dynamic values, deterministic insertion order, replacement, deletion, lookup, and presence-safe lookup; verify insertion order, replacement, and missing-key behavior in runtime unit tests.
- [x] 3.2 Implement catchable missing-property, contract-violation, and JSON-conversion errors; verify each error is catchable through `try`/`catch` and does not abort the process.
- [x] 3.3 Implement contract validation and shared key-set sealing for Object values and class instances, including failure atomicity and extra-key retention; verify aliases observe the same sealed state in runtime tests.
- [x] 3.4 Update garbage-collection descriptors and runtime tracing for tagged dynamic properties, nested objects, closures, and partially initialized objects; verify collection preserves live references and ignores uninitialized slots.
- [x] 3.5 Implement shallow object spread and object rest with independent top-level identity and shared nested references; verify source-order overrides, rest key selection, and cycle-safe behavior.

## 4. Integrate IR, interpreter, and native code generation

- [x] 4.1 Add explicit IR operations for dynamic object creation/access/mutation, checked narrowing, contract validation, sealing, spread, rest, and safe lookup; verify IR validation and lowering tests with `cargo test -p zirk-ir`.
- [x] 4.2 Implement interpreter behavior for Dynamic values, Object identity, contract checks, errors, spread/rest, and class dynamic properties; verify native/interpreter parity fixtures and bounded execution behavior.
- [x] 4.3 Implement native runtime helpers and LLVM code generation for dynamic object operations and contract checks; verify generated programs match interpreter results and preserve catchable failures.
- [x] 4.4 Remove record-only value layouts and record field-pointer lowering while preserving class, tuple, and enum layouts; verify layout and pointer regression coverage with `cargo test -p zirk-ir` and the native codegen suite.

## 5. Normalize JSON and developer tooling

- [x] 5.1 Change JSON parsing to produce `Dynamic` trees with Object nodes, `List<Dynamic>` arrays, native scalar values, and exact JSON-number preservation; verify parse/stringify round trips for nested values, Unicode, large numbers, null, and invalid input.
- [x] 5.2 Adapt `JSON.stringify`, `JSONCodec<T>`, and typed decode/encode APIs to contracts; verify duplicate-key rejection, size/depth limits, contract errors, and rejection of cycles or non-JSON values.
- [x] 5.3 Update canonical formatting, syntax highlighting, completion, hover, diagnostics, and semantic tokens for `contract`, `Dynamic`, object keys, and interpolation; verify formatter stability and editor extension JSON/JS validation.
- [x] 5.4 Migrate examples and demos, including HTTP JSON responses, from quoted JSON strings and record/interface declarations to ordinary object literals and contracts; verify the examples compile and produce the documented JSON.

## 6. Verify cross-target and compatibility behavior

- [x] 6.1 Add conformance fixtures for object identity, class-to-Object views, contract checks, alias sealing, dynamic reads/writes, mutability, missing keys, patterns, spread/rest, and generic contracts; verify them on both interpreter and native execution.
- [x] 6.2 Add JSON fixtures for exact round trips, dynamic interpolated keys, typed contract codecs, duplicate keys, resource limits, cycles, and unsupported values; verify expected results and catchable failures.
- [x] 6.3 Run the relevant Rust suites (`zirk-lexer`, `zirk-parser`, `zirk-sema`, `zirk-ir`, and `zirk-cli`) and target-specific compiler checks; record platform coverage and any unsupported targets without inferring untested support.
- [x] 6.4 Verify the migration diagnostics, handbook examples, standard library references, formatter output, and editor diagnostics agree; archive the reviewed results with the implementation change.

## 7. Publish verified documentation and status

- [x] 7.1 Update the handbook, language reference, migration guide, errors reference, examples, feature-status catalog, limitations, and roadmap from verified implementation evidence; verify all public claims against passing conformance fixtures.
- [x] 7.2 Commit the source repository documentation before website sync; run `./scripts/sync-website-content.sh` from a clean source commit and verify generated hashes and imported documents match that commit.
- [x] 7.3 Review the `../zirk-lang-site` diff and site-owned status catalog; if status evidence changed, obtain human review and pass an explicit `--audit-date YYYY-MM-DD`; verify the reviewed date and catalog changes are intentional.
- [x] 7.4 Validate both repositories, commit the website changes separately from the source repository, and record both revisions; verify the website describes the same shipped language behavior and status.
