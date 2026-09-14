# canonical-formatter Specification

## Purpose

Deliver the first slice of the canonical formatter promised by
`ZIRK_COMPILER_SPEC.md` section 10: a `zirk format` subcommand that is
canonical, idempotent and configuration-free, initially covering the
triple-quoted-string layout rule.

## ADDED Requirements

### Requirement: `zirk format` subcommand

The CLI SHALL expose `zirk format <file>` that rewrites the file in place
into canonical form. The `.zrk` extension SHALL be optional, matching the
other file-taking subcommands.

#### Scenario: Format writes in place

- **WHEN** `zirk format file.zrk` runs on a well-formed file
- **THEN** the file is rewritten in canonical form and the command exits
  successfully

#### Scenario: Unparseable file

- **WHEN** `zirk format` runs on a file with syntax errors
- **THEN** the diagnostics are reported and the file is left unchanged

### Requirement: Triple-quoted-string layout rule

`zirk format` SHALL rewrite every `'''` literal so that the opening `'''`
sits on the line after the `=`, `(` or `name:` that introduces it, indented
one level deeper than the introducing line, and the closing `'''` sits on
its own line aligned to the same column as the opening `'''`. Content lines
SHALL be re-indented to preserve their content relative to the left edge.

#### Scenario: Inline opening normalized

- **WHEN** `inmut t = '''\nhello\n    '''` is formatted
- **THEN** the literal is rewritten with `'''` on the line after `=`, one
  indent deeper, and the closing `'''` aligned to it

#### Scenario: Nested call argument

- **WHEN** a `'''` literal appears as `f('''…''')` or `f(text: '''…''')`
- **THEN** the opening `'''` moves to the line after `(`/`text:`, one indent
  deeper than the call line

### Requirement: Idempotence

`zirk format` SHALL be a fixed point: formatting an already-canonical file
produces no changes.

#### Scenario: Second run is a no-op

- **WHEN** `zirk format` runs twice on the same file
- **THEN** the second run reports no changes and leaves identical bytes

### Requirement: Check mode

`zirk format --check <file>` SHALL report whether the file is already
canonical without modifying it, exiting non-zero when changes would be
written.

#### Scenario: CI gate

- **WHEN** `zirk format --check` runs on a non-canonical file
- **THEN** it reports the file needs formatting, exits non-zero, and the
  file is unchanged

### Requirement: Rule scope is explicit

In this first delivery, `zirk format` SHALL only apply the `'''` layout
rule; all other §10 formatting concerns remain future work. The command's
help and documentation SHALL state this scope.

#### Scenario: Other style left untouched

- **WHEN** a file with non-canonical spacing but no `'''` literals is
  formatted
- **THEN** the file is unchanged
