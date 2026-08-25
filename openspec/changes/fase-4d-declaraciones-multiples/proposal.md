## Why

`docs/init/ZIRK_ROADMAP.md` Phase 4d has two independent bullets: the callable-type slice (shipped and archived as `fase-4d-callables`) and "multiple declarations and simultaneous assignment," deliberately left out of that change because it touches unrelated code paths (declaration/assignment grammar, checker, IR — not callable types). `openspec/specs/zirk-grammar/spec.md` ("Comma-grouped declarations and assignments are explicit") and `openspec/specs/zirk-type-system/spec.md` ("Multiple bindings and simultaneous assignment are atomic at the language level") already state the final normative behavior — `mut first, second: String;`, an optional comma-initializer list, and `left, right = right, left;` swapping without either destination write being observed by the other source read. None of it parses today: `docs/handbook/11-reference/12-feature-status.md` records it as "defined / not implemented." This change ships the implementation those specs already require; it does not change what is normative.

## What Changes

- Parser: a `mut`/`inmut`/`inmut::strict` declaration accepts a comma-separated list of simple binding names sharing one type annotation, with an optional comma-separated initializer list whose arity is preserved for the checker (not silently trimmed/padded). Kept syntactically distinct from tuple construction/destructuring patterns — no shared grammar production.
- Parser: assignment accepts a comma-separated list of assignable places on the left of `=` and a comma-separated list of source expressions on the right, producing one simultaneous-assignment AST node carrying both lists (and both arities, even when they differ — see "Assignment arity mismatch" scenario) rather than desugaring into sequential assignments at parse time.
- Checker: applies the declared type and binding permission (`mut`/`inmut`/`inmut::strict`) independently to every name in a comma-grouped declaration; requires initializer-list arity to equal binding-list arity when an initializer list is present (targeted count-mismatch diagnostic otherwise); uses the declared type's default independently per binding when no initializer list is present.
- Checker: for simultaneous assignment, requires equal source/destination arity (targeted diagnostic otherwise, using the arity both sides already preserved from parsing), type-checks each source against its positional destination, rejects duplicate destinations, rejects rebinding `inmut`/`inmut::strict`, and rejects mutating a projection through an `inmut::strict` referent — matching the single-assignment rules already enforced today, applied per position.
- IR/codegen: evaluates every simultaneous-assignment source left to right into temporaries *before* writing any destination, then commits writes left to right — so `left, right = right, left;` observes pre-assignment values on both sides (the swap case), consistent with ordinary single-target assignment's existing evaluate-then-store shape.
- New targeted diagnostic(s) for declaration-initializer arity mismatch and assignment arity mismatch, distinct from the generic type-mismatch/arity diagnostics used elsewhere, per the grammar spec's own "so semantic analysis can emit a targeted count-mismatch diagnostic."

## Capabilities

### New Capabilities
(none — no new capability area; this implements already-normative requirements)

### Modified Capabilities
- `zirk-grammar`: reaffirms the existing "Comma-grouped declarations and assignments are explicit" requirement verbatim — no requirement text changes; this delta exists only so the archive step records the requirement as delivered, not just specified.
- `zirk-type-system`: reaffirms the existing "Multiple bindings and simultaneous assignment are atomic at the language level" requirement verbatim, same reason.

(`docs/handbook/11-reference/12-feature-status.md` and `docs/init/ZIRK_ROADMAP.md` status prose are implementation-status documentation, not spec deltas, and are updated directly as part of this change's tasks.)

## Impact

- Affected code: `crates/zirk-parser` (declaration and assignment-statement grammar), `crates/zirk-sema` (checker rules for comma-grouped declarations and simultaneous assignment), `crates/zirk-ir` (lowering: evaluate-all-sources-then-commit-writes), `crates/zirk-codegen-llvm` if lowering alone does not already guarantee ordering at the backend.
- Public documentation: `docs/handbook/11-reference/12-feature-status.md` (row moves from "not implemented" to delivered) and `docs/init/ZIRK_ROADMAP.md` Phase 4d status line both describe implementation status, not normative behavior — updated in this workstream. Per project convention, `../zirk-lang-site` mirrors handbook/status content from a pinned commit; run `./scripts/sync-website-content.sh` after these commits land, with `--audit-date` if project-status evidence changed.
- No breaking changes: nothing that compiles today stops compiling; this only makes previously-rejected comma-grouped declaration and simultaneous-assignment syntax valid.
