# Language Server

The LSP uses cancelable incremental snapshots for diagnostics, completion, hover, definitions, references, rename, formatting, semantic tokens, signatures, and code actions. Interactive requests receive priority over stale work.

Each document edit creates an immutable snapshot. Requests name the snapshot
they read; when a newer edit makes work stale, the server cancels it and never
publishes old diagnostics over new text. Parsing and semantic dependencies are
invalidated at file/symbol granularity rather than rebuilding through LLVM.

Completion and hover use resolved visibility, types, optional/named parameters,
permissions, and generated public API. Rename checks module boundaries,
generated declarations, hygienic identities, and public compatibility before
offering edits. Semantic tokens distinguish declarations, types, decorators,
placeholders, literals, and invalid/recovered syntax.

The server does not execute build scripts, decorators with build authority, or
application code merely because an editor opened a project. Operations needing
authority return an explicit action for the user to run through the trusted
CLI.

> **Implementation status:** editor syntax support exists; the full incremental
> semantic LSP remains Phase 9 work.

---

**Previous:** [← Linter](10-linter.md) · **Next:** [ Debugger](12-debugger.md)
