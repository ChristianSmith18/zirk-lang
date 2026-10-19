## ADDED Requirements

### Requirement: Network, HTTP, and environment packages ship with the toolchain

The standard library SHALL bundle the official `std.net`, `std.http`, and `std.environment` Zirk source packages and resolve them through their stable logical imports on every supported native target. These packages SHALL enter the ordinary module graph, and compiler-owned host behavior SHALL bind only to declarations from the official bundled sources and their fully qualified identities.

#### Scenario: Compile without a separately installed standard library
- **WHEN** a project imports `std.http` or `std.environment` using the installed Zirk toolchain
- **THEN** imports resolve without a checkout-relative path or separate library installation
- **AND** the same source package identity is used across supported native targets

#### Scenario: An external package imitates a standard declaration
- **WHEN** an external package declares a function with the same short name as a standard host-bound function
- **THEN** the function remains an ordinary package function and receives no compiler-owned host binding
