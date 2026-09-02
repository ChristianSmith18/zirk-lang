<!--
  Base: develop (except release/* or hotfix/*, which go to main).
  See CONTRIBUTING.md
-->

## What changes

<!-- Short description. What problem does it solve, not how. -->

## Roadmap phase

<!-- E.g. Phase 0 — foundations. Confirm it does not advance features from later phases. -->

- Phase:
- OpenSpec change (if applicable):

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] Does not advance features from later roadmap phases
- [ ] If it touches language rules: there is at least one valid and one invalid test case
- [ ] If it adds diagnostics: they have a stable code, cause and actionable help
- [ ] If it makes an architecture decision: there is an ADR in `docs/decisions/`

## Ambiguities found

<!--
  Spec rule: every ambiguity must produce a question or be documented,
  never resolved silently. If the spec did not cover something, say so here.
  If there were none, write "none".
-->

## Verification

<!-- How you verified it works. Test output, commands, screenshots. -->
