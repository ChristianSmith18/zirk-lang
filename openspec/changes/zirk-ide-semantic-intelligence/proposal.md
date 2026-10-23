## Why

Zirk's VS Code extension already offers syntax highlighting, compiler diagnostics, type hover, document symbols, and formatting, but it does not yet connect resolved declarations to their uses or provide the navigation, completion, semantic highlighting, and runtime inspection expected from a full language experience. This change turns the existing compiler frontend and LSP MVP into a layered IDE service, with explicit contracts and staged validation so that editor features remain consistent with Zirk semantics.

## What Changes

- Establish a compiler-owned semantic snapshot that assigns stable identities to declarations and records resolved identifier uses, types, visibility, and source spans across modules.
- Add a language-neutral IDE query layer for definition, references, highlights, rename, completion, signature help, semantic tokens, inlay hints, workspace symbols, and code actions.
- Evolve `zirk-lsp` from the current four-feature MVP into a workspace-aware, incremental LSP server with truthful capability negotiation, cancellation, version-safe results, and observable indexing state.
- Upgrade the VS Code/Devin extension integration to expose the semantic features, explain server readiness and degraded modes, and retain syntax grammar coloring as a fallback beneath semantic tokens.
- Add a separate Debug Adapter Protocol (DAP) capability for source breakpoints, stepping, stack frames, scopes, and runtime values where the execution backend can provide them; static editor analysis SHALL not claim to know runtime object identity.
- Validate the current extension behavior and each delivered layer inside Devin, including a reproducible workspace fixture and documented expected interactions.
- Update the authoritative LSP/editor/compiler status and roadmap when implementation changes; synchronize the companion `../zirk-lang-site` from a committed source revision and review its status catalog.

## Capabilities

### New Capabilities

- `zirk-ide-query`: compiler-backed, editor-independent queries over symbol identities, references, types, and project exports.
- `zirk-debug-adapter`: runtime debugging through DAP, with explicit support boundaries for native and interpreter execution.

### Modified Capabilities

- `zirk-lsp`: expand the protocol contract from diagnostics, hover, document symbols, and formatting to semantic navigation, editing, completion, tokens, project indexing, and incremental analysis.
- `editor-tooling`: expose LSP-backed semantic features in VS Code and Devin while retaining syntax highlighting and the documented CLI fallback.
- `compiler-workspace`: define the compiler-owned semantic data boundary and any IDE-facing crate/module without reversing frontend dependency direction.

## Impact

Affected areas include `zirk-sema`, the shared frontend and source mapping, module/project loading, `zirk-lsp`, `editors/vscode`, native and interpreter debug metadata, and the related OpenSpec and project-status documents. This changes editor capabilities and implementation status, but does not change Zirk language syntax or runtime semantics. The implementation work must update public status/roadmap documentation and `../zirk-lang-site`; site synchronization requires a committed source revision and reviewed status-catalog changes.
