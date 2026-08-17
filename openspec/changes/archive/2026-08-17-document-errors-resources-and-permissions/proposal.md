## Why

Zirk already distinguishes expected failure, exceptional recovery, deterministic resource cleanup, and manifest permissions, but the published documentation does not define their complete interaction. Compiler, runtime, package-manager, and tooling contributors need one implementable contract for `Result`, typed exceptions, resource responsibility, permission inference, and developer authorization before those subsystems diverge.

## What Changes

- Finalize `Result<T, E>` as the mandatory representation of expected failure, including its initial API, exhaustive handling, explicit discard, combinator effects, and fatal unwrap contract.
- Define checked explicit exceptions, implicit typed `RuntimeError` failures, `Throwable`, patterned `catch`, exact rethrowing, causes, suppressed failures, immutable exception identity, and structured stack traces.
- Define deterministic `Resource<E>` cleanup through `match with`, multiple-resource acquisition, close-failure composition, transfer, dependent lifetimes, non-clonability, and compile-time leak prevention.
- Replace separate public runtime/build permission blocks with library `requires` and application `permissions`, using `during: build | runtime | both` inside grants.
- Define automatic permission-effect inference through functions and callables without adding permission syntax to `Fn`.
- Define signed developer approvals bound to project name and canonical location, incremental permission/dependency fingerprints, reapproval triggers, interactive manifest editing, CI policy, audit history, and runtime denial behavior.
- Define scoped filesystem, network, environment/secret, process, and shell grants, including the standard `Environment` / `Env` API.
- **BREAKING**: replace historical `catch<Type> name` examples with pattern-shaped `catch Type(binding)` and remove `compile_permissions` as a public top-level manifest block.

## Capabilities

### New Capabilities

- `zirk-errors`: `Result`, explicit and implicit exceptions, error contracts, handling, propagation, and diagnostics.
- `zirk-resources`: deterministic acquisition, cleanup, transfer, dependency, and leak-prevention semantics.
- `zirk-permissions`: requirements, grants, inferred effects, secure approvals, sandbox enforcement, and audit tooling.

### Modified Capabilities

- `zirk-grammar`: add final `throws`, `throw`, patterned `catch`, `match with`, `requires`, `permissions`, and `during` forms.
- `zirk-type-system`: type exception effects, `Result` consumption, resource responsibility, transfer, and permission-effect propagation.
- `zirk-runtime-io`: enforce resource closure, exception propagation, secure environment/secret access, and scoped runtime permissions.
- `zirk-cli-commands`: add incremental permission scanning, approval prompts, secure history, revocation, and CI policy commands.
- `handbook-editorial-system`: publish complete tutorials, valid/invalid examples, API tables, and cross-links for the accepted semantics.
- `language-documentation-information-architecture`: establish canonical ownership and reading order for error, resource, and permission contracts.

## Impact

The change updates normative specifications, the handbook error/resource chapters, manifest and standard-library references, CLI documentation, security explanations, glossary, navigation, and the historical inventory checkpoint. It is documentation/OpenSpec work only; compiler and runtime implementation remains assigned to later delivery changes. Existing Phase 3 implementation scope is not expanded.
