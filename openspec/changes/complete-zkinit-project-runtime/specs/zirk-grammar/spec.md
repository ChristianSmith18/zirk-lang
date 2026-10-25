## RENAMED Requirements

- FROM: `### Requirement: Permission manifests use requires and permissions`
- TO: `### Requirement: Capability manifests use one requires block`

## MODIFIED Requirements

### Requirement: Capability manifests use one requires block

The manifest grammar SHALL accept one `requires` block in either an application or package manifest. The block SHALL contain unique capability identifiers from the finite Zirk capability vocabulary; it SHALL NOT require resource-specific scopes or build/runtime/test phase fields. The grammar SHALL reject `permissions`, `compile_permissions`, and `during` with a diagnostic that names the unified `requires` form. The parser SHALL preserve each capability name and source span for compiler diagnostics and editor actions.

#### Scenario: Application declares capability needs
- **WHEN** an application `.zkinit` contains a `requires` block with `filesystem.read` and `network.listen`
- **THEN** the manifest AST records those two capabilities and their source spans

#### Scenario: Build-only filesystem grant
- **WHEN** a legacy `.zkinit` contains a filesystem read grant marked `during: build`
- **THEN** the parser rejects the phase syntax and directs the user to the unified `requires` capability form

#### Scenario: Library declares a capability need
- **WHEN** a package `.zkinit` contains a `requires` block
- **THEN** the same capability syntax is parsed and the package identity remains the requester

#### Scenario: Legacy permission sections are rejected
- **WHEN** `.zkinit` contains `permissions`, `compile_permissions`, or `during`
- **THEN** the parser emits a targeted removed-syntax diagnostic pointing to `requires`

## ADDED Requirements

### Requirement: Project manifest grammar covers the canonical project model

The `.zkinit` grammar SHALL parse `project`, repeated `source` declarations, `build_targets`, `dependencies`, `dev_dependencies`, `globals`, and the single `requires` block. The project object SHALL support `name`, `version`, `type`, optional `authors`, `description`, `license`, and the type-specific `entry` or `root`. Dependency declarations SHALL support a logical package key with a version string or an expanded source object. Global declarations SHALL support the type shorthand and an expanded object with `type`, optional `default`, `from`, and `readonly`. Duplicate keys, malformed values, and unknown structural forms SHALL retain spans for diagnostics.

#### Scenario: Full application manifest parses
- **WHEN** a manifest declares application metadata, source roots, build targets, dependencies, globals, and requires using the canonical forms
- **THEN** the parser produces one project model with spans for each field and block

#### Scenario: Expanded dependency and global parse
- **WHEN** a dependency uses a path object and a global uses an expanded typed object
- **THEN** both objects retain their named fields without confusing source paths with package identity or type names

#### Scenario: Duplicate manifest key
- **WHEN** a block declares the same project or global key twice
- **THEN** the parser or manifest validator reports both relevant source locations

#### Scenario: Target list and source roots parse
- **WHEN** `.zkinit` contains `source "src"` and a `build_targets` list of quoted supported target identifiers
- **THEN** the manifest AST retains each path/target and its source span in declaration order

### Requirement: Source files accept executable top-level statements

The parser SHALL accept ordinary executable statements at source-file scope in addition to declarations. Imports SHALL remain static declarations and SHALL precede executable top-level statements. The parser SHALL retain the source order of top-level items so the checker and runtime can distinguish declarations from the entry/module body.

#### Scenario: Entry has sequential top-level statements
- **WHEN** a source file contains two executable statements at file scope
- **THEN** the parser preserves their source order as the file body

#### Scenario: Import follows executable code
- **WHEN** a static import appears after an executable top-level statement
- **THEN** the parser emits a targeted diagnostic requiring imports before executable statements
