## ADDED Requirements

### Requirement: Failure, resource, and permission chapters teach complete contracts
The handbook SHALL document the accepted model with mental models, syntax, valid and invalid examples, API/exception/permission tables, composition behavior, diagnostics, security rationale, implementation status, and previous/next links. It SHALL distinguish final semantics from historical syntax and current compiler availability.

#### Scenario: Contributor reads error unit
- **WHEN** a contributor follows the error handbook unit
- **THEN** they can determine whether a failure uses `Result`, declared `throws`, implicit `RuntimeError`, or `fatalError`, and how it composes with cleanup

### Requirement: Security-sensitive prompts and policies have worked examples
Permission documentation SHALL show manifest requests/grants, dependency paths, moved-project reapproval, update reapproval, incremental fast paths, broad-grant confirmation, CI policy, runtime denial, audit history, and tamper scenarios.

#### Scenario: Malicious manifest edit is explained
- **WHEN** a reader examines the permission-security chapter
- **THEN** it explicitly demonstrates that changing `init.zrk` cannot create a valid signed approval
