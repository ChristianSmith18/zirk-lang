## RENAMED Requirements

- FROM: `### Requirement: Decorator effects require build authority`
- TO: `### Requirement: Decorator effects use unified capabilities`

## MODIFIED Requirements

### Requirement: Decorator expansion is deterministic and incremental

The expansion cache key SHALL cover the decorator implementation and version, arguments, typed target, relevant configuration, capability requirements and saved-consent fingerprint, all observed external build inputs, and transitive dependencies. Unchanged fingerprints SHALL reuse validated results, while changes SHALL invalidate only affected expansions.

#### Scenario: No decorator input changes
- **WHEN** a project rebuilds with matching decorator and observed-input fingerprints
- **THEN** the compiler reuses the validated expansion without executing it again

### Requirement: Decorator effects use unified capabilities

Decorator arguments and expansion SHALL declare privileged effects through the same finite `requires` vocabulary used by applications and packages. The manifest SHALL NOT add compile-time permission blocks or `during` phases. Before a decorator performs a concrete protected effect, the compiler SHALL check or request consent for its exact resource and requester path. An unapproved or rejected compile-time effect SHALL not read or modify the resource and SHALL produce an expansion diagnostic at that operation. Every observed value SHALL become a tracked build input. Secrets SHALL NOT appear in generated public structure, IR, diagnostics, logs, or cache keys.

#### Scenario: Decorator reads an unapproved environment variable
- **WHEN** a decorator attempts an environment read without saved consent for the exact name
- **THEN** expansion stops before the read and the diagnostic identifies the decorator, resource, and dependency path
