## ADDED Requirements

### Requirement: Local verification reproduces CI's step order

`scripts/check-local.sh` SHALL run the same steps as
`.github/workflows/ci.yml`, in the same order: formatting, `clippy`, `cargo
build --workspace`, and then `cargo test --workspace`. The script SHALL NOT
skip the build step before tests.

`zirk-cli`'s end-to-end tests locate the `zirk` executable and the
runtime's static library under `cargo build`'s final output paths
(`target/<profile>/`), not under `cargo test`'s hashed paths in
`target/<profile>/deps/`. Without a prior `cargo build --workspace`, those
tests can either fail to find the artifact, or — in a tree with a stale
prior build — pass against a binary that no longer matches the current
source.

#### Scenario: Running against a clean working tree
- **WHEN** `./scripts/check-local.sh` is run in a freshly cloned copy with no prior `target/`
- **THEN** the script builds the full workspace before running the tests
- **AND** the script's result matches what CI would produce for the same commit

#### Scenario: An end-to-end test depends on the final build binary
- **WHEN** `crates/zirk-cli/tests/end_to_end.rs` looks up the `zirk` executable or `libzirk_runtime.a`/`zirk_runtime.lib`
- **THEN** it finds the artifact produced by the script's build step, not a stale or missing one

#### Scenario: Local script and CI diverge
- **WHEN** `.github/workflows/ci.yml` is modified to add, remove, or reorder a verification step
- **THEN** `scripts/check-local.sh` is updated in the same change to keep reproducing the same order
