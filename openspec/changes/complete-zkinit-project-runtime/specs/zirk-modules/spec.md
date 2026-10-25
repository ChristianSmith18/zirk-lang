## MODIFIED Requirements

### Requirement: Mutual imports

Static module loading SHALL support declaration cycles without reading a source file more than once. Runtime initialization SHALL use canonical package/module identity to evaluate each reachable imported module at most once per process. Imports SHALL be considered in source order; the loader SHALL evaluate each imported module and its dependencies before continuing with the importing module. A strongly connected import component that contains executable top-level initialization SHALL be rejected before program code runs; declaration-only cycles MAY remain valid.

#### Scenario: Direct cycle
- **WHEN** file A imports declarations from file B and file B imports declarations from file A, and neither has executable top-level initialization
- **THEN** the crate compiles and each file is read once

#### Scenario: Diamond of imports
- **WHEN** A imports B and C and both import D
- **THEN** D is read and initialized exactly once

#### Scenario: Cycle with executable initialization
- **WHEN** a strongly connected import component contains top-level executable statements
- **THEN** compilation reports the complete initialization cycle before any module body executes

## ADDED Requirements

### Requirement: Imported modules initialize once before their importers

After the complete static import graph has been loaded and checked, the runtime SHALL initialize each imported module before executing the importing module's remaining top-level statements. Initialization SHALL follow deterministic depth-first source order and SHALL be keyed by canonical package/module identity so re-exports and repeated imports do not duplicate execution. Static imports SHALL appear before executable top-level statements in a file; a violation SHALL produce a source diagnostic.

#### Scenario: Import initialization precedes importer statements
- **WHEN** module A imports module B and both contain top-level statements
- **THEN** B's top-level statements execute before A's statements

#### Scenario: Re-export does not repeat initialization
- **WHEN** two package-root paths re-export the same canonical source module
- **THEN** that module's top-level statements execute once for the process

#### Scenario: All static errors precede runtime effects
- **WHEN** the import graph contains a missing module, type error, or invalid permission requirement
- **THEN** the compiler reports it before executing any module or application top-level statement

### Requirement: Module initialization failures stop dependent startup

An uncaught failure during module initialization SHALL prevent its importer and the application entry from executing. A failure explicitly handled as a value or exception inside module top-level code MAY continue according to the language's existing error semantics. The CLI SHALL report the failing module and dependency path and return a non-zero process status for an unhandled startup failure.

#### Scenario: Imported module fails during initialization
- **WHEN** a module's top-level operation raises an uncaught error
- **THEN** its importer and application entry do not execute and the diagnostic identifies the module

#### Scenario: Module handles its own initialization error
- **WHEN** top-level module code explicitly handles an operation error
- **THEN** the module may complete initialization and importer execution may continue
