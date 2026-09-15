# inferred-return-types

## Why

Locals and fields already infer their type from an initializer (`mut x = 5`),
but every `fn` and method must still spell out `: ReturnType` — the parser emits
`MISSING_RETURN_TYPE` when it is absent. The result is boilerplate on the most
common case in the language (`fn main(): Void`) and an inconsistency with the
inference the rest of the declaration surface already offers. At the same time,
the editor extension shows nothing about types on hover: a programmer cannot
see what an inferred (or declared) signature resolved to, the way TypeScript's
hover does.

## What Changes

- The `: ReturnType` annotation becomes **optional** on `fn` declarations and on
  class/record methods **that have a body**. When omitted, the checker unifies
  the types of the body's `return` expressions:
  - no value-returning `return` and the body can fall off the end → `Void`;
  - no value-returning `return` and the body always diverges (throws,
    `fatalError`, `loop` without `break`) → `Never`;
  - one or more `return expr` → their types are unified with the same join
    rules `if`-expression branches already use; an incompatible mix is an
    error.
- An **explicit annotation remains the contract**: when `: T` is written, the
  body must satisfy `T` exactly as today — inference only fills in what is
  absent.
- Return-type inference is **demand-driven with cycle detection**: a function
  whose signature is still pending is inferred the first time a caller (or the
  body-check pass) needs it; a recursive or mutually recursive cycle with no
  annotated member is rejected with a diagnostic naming the cycle and asking
  for an annotation on one member.
- Signatures that have **no body to infer from keep the annotation mandatory**:
  `interface` members, `abstract` methods, `extern "C" fn`, and `fn dec`.
- The entrypoint rule is unchanged but applies to the resolved type:
  `fn main() { }` infers `Void` and is valid; `fn main() { return 0 }` infers
  `Int32` and is rejected until Phase 6's extended `main` signatures arrive.
- **Editor hover** (new capability surface): `zirk check` gains a `--types`
  mode that emits a JSON type map — file/span → rendered type text — for every
  declaration name and resolved expression. The VS Code extension registers a
  hover provider that queries the map (refreshed on save, like diagnostics) and
  renders TS-style hovers (`fn hola(): Int32`, `count: Int32`), marking
  inferred types as inferred. Extension version bumps to 0.5.0.

## Capabilities

### New Capabilities

- `return-type-inference`: the language capability — optional return
  annotation on `fn` and methods with bodies, unification and `Void`/`Never`
  defaulting rules, demand-driven inference with cycle detection, and the
  contexts where an annotation remains mandatory.

### Modified Capabilities

- `zirk-grammar`: the "return type after `:`" requirement and its
  "Missing return type" scenario change — an omitted annotation is now a valid
  declaration, not a diagnostic, when a body is present.
- `zirk-type-system`: the return-checking requirement is restated over the
  *resolved* (declared or inferred) return type; the entrypoint requirement
  applies to the resolved type of `main`.
- `zirk-check`: the command gains an optional `--types` output mode emitting
  the span→type map without producing backend artifacts.
- `editor-cli-bridge`: the bridge gains a type-map retrieval requirement and
  a hover-mapping requirement (position → entry → rendered text).
- `editor-tooling`: the "Scope without a language server" requirement is
  amended — hover is no longer excluded when it is served by the CLI bridge;
  autocomplete, navigation and rename remain Phase 9 exclusions.

## Impact

- `crates/zirk-parser`: `parse_return_type` becomes optional-aware;
  `MISSING_RETURN_TYPE` is repurposed for bodyless signatures only.
- `crates/zirk-sema`: signature declaration registers an "inferred-pending"
  return; a demand-driven inference pass (reusing the
  `infer_method_mutation`-style body walk) resolves it with cycle detection;
  the checker records a span→type map during checking.
- `crates/zirk-cli`: `zirk check --types` emits the type map as JSON.
- `editors/vscode`: hover provider, type-map cache, version 0.5.0; per
  `AGENTS.md` the extension is copied to
  `~/.devin/extensions/christiansmith.zirk-lang-0.5.0/` and validated.
- **Public docs/status affected**: `docs/init/ZIRK_FEATURE_STATUS.md` gains
  the inference + hover rows; handbook pages that show `fn main(): Void`
  examples stay valid but the function-declaration pages document the optional
  annotation → `../zirk-lang-site` must be re-synced per AGENTS.md
  (`./scripts/sync-website-content.sh`, `--audit-date` if status text moved).
- **No normative-language rollback**: explicit annotations keep their exact
  current meaning; this change only adds an optional omission. Not a breaking
  change.
