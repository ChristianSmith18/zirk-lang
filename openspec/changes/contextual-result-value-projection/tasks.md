## 1. Normative Decision and Compatibility Surface

- [x] 1.1 Add ADR-024 documenting contextual Result preservation/projection, expected-type and generic-inference ordering, intentional error-detail loss, standalone discard, print behavior, removed helpers, alternatives, and migration; verify its decision matches every delta requirement.
- [x] 1.2 Update `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`, `docs/CORE_LANGUAGE_SEMANTICS.md`, `docs/ZIRK_LANGUAGE_SPEC.md`, and `docs/ZIRK_STDLIB_SPEC.md`; verify no normative statement still requires extraction in every value context or lists removed APIs as supported.
- [x] 1.3 Update Result and environment handbook pages, indexes, and runnable examples; verify examples demonstrate direct `match`, explicitly typed `Result<T,E>`, inferred `T?`, `??`, and permission-error visibility.

## 2. Contextual Type Checking

- [x] 2.1 Add a typed semantic representation for one-shot Result projection and resolve direct `match`, explicit Result expectations, ordinary value contexts, and ambiguity diagnostics; verify with checker tests for each context.
- [x] 2.2 Define inference ordering for explicit and inferred generic parameters, arguments, attributes, assignments, and return types; verify Result-preserving and nullable-projection calls in both inferred and explicitly instantiated generic cases.
- [x] 2.3 Preserve standalone Result must-use checking and explicit `_ =` discard while allowing projected values in value-demanding expressions; verify both accepted and rejected statement forms.
- [x] 2.4 Remove Result method recognition for `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, and `unwrap_error`; verify each produces a stable migration diagnostic.

## 3. Lowering and Runtime Behavior

- [x] 3.1 Lower `Ok(value)` to the payload and `Error(error)` to nullable absence exactly once, without reading or transforming `E`; verify generated IR for both variants and nested Result payloads.
- [x] 3.2 Implement and verify the same projection in interpreter/browser IR and native LLVM execution; run focused parity cases on every target currently claimed as supported and record unsupported targets accurately.
- [x] 3.3 Make print, println, and interpolation project Result expressions before nullable rendering while retaining direct `Result.to_string()` variant rendering; verify success, failure, explicit Result receiver, and `<Null>` output cases.

## 4. Standard Library Migration

- [ ] 4.1 Remove `Env.get_or_null` and `Env.get_or`, retain `Env.get<T>(name): Result<T, EnvironmentError>`, and migrate standard-library callers; verify absent, parse-failed, denied, and host-failure outcomes remain distinguishable by explicit matching.
- [x] 4.2 Audit the complete Result API and remove the specified inspection/extraction/fallback/unwrap helpers while retaining Result composition operations; verify no exported declaration or compiler intrinsic still exposes removed helpers.
- [x] 4.3 Audit other standard-library methods and keep unrelated nullable collection lookups and APIs whose contracts are not Result extraction; verify the public API inventory matches the implementation.

## 5. Conformance and Status

- [x] 5.1 Add compiler conformance coverage for direct matching, explicit Result annotations, inferred nullable bindings, parameter/return/attribute contexts, generic inference, coalescing, nested Results, permission-denial projection, method-removal diagnostics, and standalone discard; verify with the relevant crate and integration test commands.
- [x] 5.2 Audit feature status, current limitations, implementation-pipeline and roadmap documents after target verification; verify each claim names only behavior demonstrated by tests and supported runtime paths.
- [ ] 5.3 Run focused sema, IR, interpreter/browser, native, standard-library, formatter, and full CI checks affected by the implementation; record exact commands and distinguish passing checks from unverified targets.

## 6. Public Website Synchronization

- [ ] 6.1 Commit the reviewed authoritative Zirk documentation and status changes, then run `./scripts/sync-website-content.sh`; verify it imports the exact clean Zirk commit and updates the expected descriptors and public pages.
- [ ] 6.2 Review `../zirk-lang-site` generated and site-owned diffs, validate both repositories, and commit the site update separately; verify the public site presents the same Result contract, migration guidance, and verified implementation status.
