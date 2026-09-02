# Phase 4d real-status decision

## Decision
Phase 4d — "Callable and binding completion" — is **complete for its scoped delivery**. Both shipped slices are archived with their tasks 100 % complete and their verification evidence present in the repository.

## Evidence

### Slice 1: callables
- Archive: `openspec/changes/archive/2026-08-25-phase-4d-callables/tasks.md`
  - All tasks in sections 1 (lexicon/grammar), 2 (checker), 3 (IR/codegen), and 4 (wrap-up) are marked `[x]`.
  - Section 4.2 lists `crates/zirk-cli/tests/corpus/valid/callable_types.zrk` as the valid end-to-end fixture; that file exists today.
  - Section 4.3 lists `crates/zirk-cli/tests/corpus/invalid/two_captured_closures_one_return_type.zrk` and `recursive_lambda_as_value.zrk` as invalid fixtures; both exist today.
- Targeted test run (`cargo test -p zirk-sema -p zirk-parser -- closure`) passed 7 relevant typing tests, including `valid_closure_returned_from_a_function`, `valid_generic_inference_from_a_closure_argument`, and `invalid_two_differently_captured_closures_in_one_return_type`.

### Slice 2: multiple declarations and simultaneous assignment
- Archive: `openspec/changes/archive/2026-08-24-phase-4d-multiple-declarations/tasks.md`
  - All tasks in sections 1–8 are marked `[x]`.
  - Section 6.1 lists `crates/zirk-cli/tests/corpus/valid/multi_decl_assign.zrk` and the invalid fixtures `multi_let_arity_mismatch.zrk`, `multi_assign_arity_mismatch.zrk`, `multi_assign_duplicate_target.zrk`; all exist today.
- Targeted test run (`cargo test -p zirk-sema -p zirk-parser -- multi`) passed 11 typing tests and 6 grammar tests, including `valid_multi_assign_swap_type_checks`, `valid_multi_let_type_and_permission_fan_out`, and the parser tests `valid_multi_let_matching_initializer_list` / `invalid_multi_let_non_identifier_in_name_list`.

## ADR-004 portability verification
- `docs/decisions/ADR-004-portabilidad.md` was reviewed in full. Its "Verification status" table (lines 80–88) already declares **Portability B (emission for 9 targets)**, **Linux x86_64/aarch64**, **macOS aarch64**, and **Windows x86_64** as verified. The remaining entries (macOS x86_64, Windows aarch64) are explicitly marked out of scope. Therefore the portability verification is already covered; no pending work is implied by this ADR.

## Consequence for this change
- `docs/init/ZIRK_ROADMAP.md` is the document that correctly records Phase 4d as complete.
- `docs/init/ZIRK_AGENT_PROMPT.md` is the document that incorrectly still declares "Next phase: Zirk 0.4d" and must be updated to reflect that Phase 4d is complete and Phase 4e is the current in-progress phase.
