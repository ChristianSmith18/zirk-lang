## ADDED Requirements

### Requirement: Local imports remain within an authorized source root

The loader SHALL resolve quoted imports and relative module re-exports only within the root of the current project, or within the entry file's directory when loading a standalone file or editor overlay. A declared dependency SHALL be confined to its resolved package root. Existing paths SHALL be checked after filesystem canonicalization so that symlinks cannot escape their authorized root. Paths that escape a root SHALL produce a dedicated diagnostic at the import or re-export that requested the path, without reading the target source.

#### Scenario: Parent traversal escapes the project
- **WHEN** a project source imports a relative path that normalizes outside the project root
- **THEN** the loader rejects the import with the root-containment diagnostic
- **AND** it does not read the target file

#### Scenario: Symlink escapes the package root
- **WHEN** an imported path resolves through a symlink to a file outside the current project or package root
- **THEN** the loader rejects the import with the root-containment diagnostic
- **AND** it does not read the target file

#### Scenario: Import stays inside its root
- **WHEN** an import resolves to a file within the current project's or package's root
- **THEN** the loader reads that file normally

#### Scenario: Declared dependency source
- **WHEN** a project imports a file within a dependency resolved from its manifest and lockfile
- **THEN** the loader accepts the file within that dependency's own root
- **AND** it rejects paths that escape the dependency root

#### Scenario: Editor overlay entry does not exist on disk
- **WHEN** an editor overlay supplies the entry source at a path whose parent is within the standalone source root
- **THEN** the loader accepts the overlay entry and resolves its imports under that root

#### Scenario: Missing file within root
- **WHEN** an imported path does not exist but remains lexically inside its authorized root
- **THEN** the loader emits the existing unreadable-file diagnostic at the requesting import
