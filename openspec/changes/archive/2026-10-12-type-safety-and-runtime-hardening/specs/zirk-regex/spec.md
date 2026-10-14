## ADDED Requirements

### Requirement: Native compiled-pattern retention is bounded

The native runtime SHALL impose a deterministic bound on compiled Regex cache entries or retained bytes. Cache eviction or limit failure SHALL not invalidate a Regex value still reachable by a program. User-controlled patterns SHALL not cause unbounded process-lifetime retention.

#### Scenario: Many distinct patterns are parsed
- **WHEN** a program repeatedly parses distinct user-controlled patterns beyond the configured cache bound
- **THEN** retained compiled-pattern memory stays within the bound and parsing follows a deterministic eviction or controlled limit policy

#### Scenario: Cached pattern is reused
- **WHEN** a pattern is compiled and remains in the cache for a repeated parse
- **THEN** the compiled matcher is reused without compiling the same pattern twice for one parse operation
