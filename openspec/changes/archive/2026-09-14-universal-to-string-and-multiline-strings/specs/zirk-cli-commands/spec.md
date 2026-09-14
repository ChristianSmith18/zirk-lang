# zirk-cli-commands Delta

## ADDED Requirements

### Requirement: Format subcommand in the CLI

The CLI SHALL expose `zirk format <file>` delivering the canonical formatter
of `ZIRK_COMPILER_SPEC.md` section 10, scoped in this delivery to the
triple-quoted-string layout rule owned by `canonical-formatter`.

#### Scenario: Formatting a file

- **WHEN** `zirk format main.zrk` runs on a well-formed file
- **THEN** the file is rewritten in canonical form

#### Scenario: Check mode

- **WHEN** `zirk format --check main.zrk` runs on a non-canonical file
- **THEN** the command reports the needed changes, exits non-zero, and does
  not modify the file
