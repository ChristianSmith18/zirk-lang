## ADDED Requirements

### Requirement: `zirk check` works without LLVM

The `zirk check` command SHALL execute and return useful results without `LLVM_SYS_201_PREFIX` set and without an LLVM installation on the host.

The LLVM major-version pin and the `llvm20-1` feature of `inkwell` SHALL apply only to commands that emit native code; `zirk check` is not one of those commands.

#### Scenario: Running `zirk check` without `LLVM_SYS_201_PREFIX`
- **WHEN** `zirk check` is executed with `LLVM_SYS_201_PREFIX` unset
- **THEN** the command runs the frontend and reports diagnostics
- **AND** it does not fail because LLVM is missing

#### Scenario: Building `zirk check` on a clean machine
- **WHEN** `cargo build -p zirk-cli --bin zirk-check --no-default-features` is run on a machine with no LLVM
- **THEN** the build succeeds
- **AND** the resulting binary can validate `.zrk` files

### Requirement: Toolchain documentation distinguishes validation from compilation

`docs/TOOLCHAIN.md` SHALL document that `zirk check` does not require LLVM, while `zirk build` and `zirk run` do.

#### Scenario: A new contributor reads the toolchain guide
- **WHEN** a contributor wants to validate a `.zrk` file without installing LLVM
- **THEN** the toolchain guide explicitly states that `zirk check` is available and does not need `LLVM_SYS_201_PREFIX`
