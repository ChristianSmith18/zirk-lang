## MODIFIED Requirements

### Requirement: Named argument shorthand is explicit

The call grammar SHALL accept `name: expression` as an explicit named argument
and `name:` as shorthand for `name: name`. The shorthand SHALL accept only a
simple identifier, SHALL remain reorderable with other named arguments, and
SHALL NOT reinterpret a reordered bare positional identifier by matching its
spelling to a parameter. After the first explicit or shorthand named argument,
the call SHALL NOT accept a positional argument.

#### Scenario: Reordered shorthand arguments

- **WHEN** source calls `client.get(timeout:, url:)`
- **THEN** the AST records named arguments equivalent to
  `client.get(timeout: timeout, url: url)`

#### Scenario: Member expression uses explicit value

- **WHEN** source attempts `client.get(config.url:)`
- **THEN** compilation diagnoses invalid shorthand and recommends
  `client.get(url: config.url)`

#### Scenario: Positional argument follows a named argument

- **WHEN** source calls `client.get(timeout:, url)`
- **THEN** compilation rejects the positional argument after the named argument
