## MODIFIED Requirements

### Requirement: Format subcommand in the CLI

The CLI SHALL expose `zirk format <target>...` accepting one or more files
and directories; directories are scanned recursively for `.zrk` files.
`--check` reports required changes without writing and exits non-zero when
any target needs formatting; `--diff` prints the canonical changes without
writing.

#### Scenario: Formatting a file

- **WHEN** `zirk format main.zrk` runs on a well-formed file
- **THEN** it rewrites the file in canonical form

#### Scenario: Multiple targets

- **WHEN** `zirk format a.zrk b.zrk src/` runs
- **THEN** every listed file and every `.zrk` file under `src/` is
  formatted, and each target reports whether it changed

#### Scenario: Check mode over several files

- **WHEN** `zirk format --check` runs over multiple targets
- **THEN** it exits non-zero if any target would change and names them

#### Scenario: Diff mode writes nothing

- **WHEN** `zirk format --diff` runs
- **THEN** it prints the canonical diff for each target and writes nothing
