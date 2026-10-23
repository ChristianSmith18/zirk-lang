## 1. Loader Boundary Enforcement

- [x] 1.1 Add explicit project and standalone source roots to loader construction and verify existing module-loader tests pass.
- [x] 1.2 Reject lexical traversal and canonical symlink escapes before source reads, assign diagnostic `E0512`, and verify focused traversal and symlink regression tests pass.
- [x] 1.3 Preserve overlays and declared package roots while deduplicating canonical aliases; verify overlay and package graph tests pass.

## 2. Contract and Documentation

- [x] 2.1 Add the root-containment requirement and scenarios to the `zirk-modules` spec and verify strict OpenSpec validation passes.
- [x] 2.2 Document quoted-import root boundaries and update compiler improvement status; verify references and `git diff --check` are clean.
- [x] 2.3 Review the companion compiler website synchronization requirements and record that its sync script requires committed compiler docs; defer publication sync until those changes are committed.
