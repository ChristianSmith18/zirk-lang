## 1. Canonical Semantics

- [x] 1.1 Add the accepted error/resource/permission checkpoint to master and contributor sources
- [x] 1.2 Rewrite normative language failure, exception, resource, and permission sections
- [x] 1.3 Expand runtime contracts for exception propagation, cleanup, suppression, and enforcement
- [x] 1.4 Expand standard-library contracts for Result, Error, Resource, Env, SecretString, and privileged APIs
- [x] 1.5 Update compiler and CLI specifications for effect analysis, incremental approval, and audit commands
- [x] 1.6 Mark superseded historical inventory answers without erasing provenance

## 2. Result and Exceptions Handbook

- [x] 2.1 Expand Result representation, mandatory consumption, explicit discard, and Go comparison
- [x] 2.2 Publish the complete Result method catalog with generic signatures and failure behavior
- [x] 2.3 Document explicit propagation, combinator exception effects, and absence of implicit Result/exception conversion
- [x] 2.4 Document explicit checked throws and implicit catchable RuntimeError hierarchy
- [x] 2.5 Replace historical catch syntax with pattern-shaped catch and exhaustiveness rules
- [x] 2.6 Document finally restrictions, exact rethrow, wrapping, cause, suppressed failures, identity, and stack traces
- [x] 2.7 Document fatalError, unwrap, assertions, task/main boundaries, and exception-free boundaries

## 3. Resource Handbook

- [x] 3.1 Expand Resource contract and acquisition versus closure responsibilities
- [x] 3.2 Document match-with syntax and exactly-once cleanup across every exit path
- [x] 3.3 Document grouped acquisition order, reverse unwind, and partial acquisition failure
- [x] 3.4 Document ResourceFailure composition and throwable suppression
- [x] 3.5 Document transfer, invalidation, dependent resources, non-clonability, and explicit duplication
- [x] 3.6 Document resource containers, take/move extraction, leak diagnostics, and runtime defense

## 4. Permission Handbook

- [x] 4.1 Replace compile_permissions with requires/permissions plus during phases
- [x] 4.2 Document automatic effect inference through functions and callables
- [x] 4.3 Document scoped filesystem, network, environment, secret, process, and shell grants
- [x] 4.4 Document Environment/Env and SecretString APIs with valid and denied examples
- [x] 4.5 Document signed approval identity by project name/location and tamper resistance
- [x] 4.6 Document dependency update reapproval and exact requester/path/version/integrity tracking
- [x] 4.7 Document incremental fingerprint fast path and changed-subgraph revalidation
- [x] 4.8 Document interactive add/approve UX, broad-grant confirmation, CI policy, and deployed denial
- [x] 4.9 Document permission inspection, diff, history, revocation, and diagnostics

## 5. Reference and Verification

- [x] 5.1 Update grammar, keyword, type, member, standard-library, CLI, manifest, and feature-status references
- [x] 5.2 Update source map, glossary, learning paths, explanations, and navigation
- [x] 5.3 Audit old catch syntax, implicit propagation, dropped close errors, compile_permissions, path-only trust, repeated prompts, and effects missing from callables
- [x] 5.4 Validate all Markdown links, anchors, fences, headings, tables, and examples
- [x] 5.5 Validate the new change and every affected active change in strict mode
- [x] 5.6 Run repository formatting, linting, and tests and confirm documentation/OpenSpec-only diff
