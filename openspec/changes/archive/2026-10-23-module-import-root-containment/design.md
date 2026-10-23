## Context

The driver currently normalizes `.` and `..` lexically, reads the resulting path, and only then assigns source identity. Project loading already knows the project root and resolved dependency entry files; standalone and editor-overlay loading have only an entry path. See `proposal.md` for the motivation and `specs/zirk-modules/spec.md` for the required behavior.

## Goals / Non-Goals

**Goals:**
- Carry an explicit local root through loader construction.
- Validate the lexical path before file access and validate the canonical target after resolution.
- Keep package source confined to its own resolved root and preserve overlay support.

**Non-Goals:**
- Changing package resolution, import syntax, or source identity semantics beyond canonical deduplication of filesystem aliases.
- Sandboxing compiler plugins, build scripts, or arbitrary compiler subprocesses.

## Decisions

- Use the project manifest directory as the local root for `load_project`; use the entry file's parent directory for `load` and `load_with_overlay`. This matches crate boundaries while preserving editor buffers whose entry file is not yet saved.
- Treat each resolved package root as the parent directory of its declared package entry file. A source path is authorized only when it is lexically under one of these roots, and an existing filesystem target must also be canonically under the same root.
- Keep lexical paths for relative-import traversal and source locations, but use canonical paths as loader deduplication keys for existing files. This prevents aliases through in-root symlinks from loading a source twice.
- Validate before reading. If the target is missing, retain the existing unreadable-file diagnostic, provided its lexical path is contained. For overlay-only entries, validate the nearest existing parent directory against the canonical root.
- Add a dedicated `E0512` diagnostic for root escapes; attach it to the requesting import/re-export when available.

## Risks / Trade-offs

- [Projects may intentionally import sibling files outside their root] → Require those files to be declared as a package root or moved under the project root; this is an intentional boundary change.
- [Filesystem changes between canonicalization and reading can race] → Canonicalize and read immediately; this protects ordinary local compilation but does not claim race-free sandboxing against a concurrently hostile filesystem.
- [Windows symlink creation may be unavailable in some test environments] → Make symlink-specific tests skip only when the platform refuses symlink creation; keep lexical traversal tests unconditional.

## Migration Plan

No data migration is required. Existing imports contained by project, standalone, and declared package roots continue to work. Imports escaping those roots fail with `E0512`; users can move the source under the root or declare it as a dependency. Rollback consists of reverting the loader and diagnostic change.
