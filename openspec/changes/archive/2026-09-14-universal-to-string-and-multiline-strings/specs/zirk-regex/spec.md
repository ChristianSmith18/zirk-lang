# zirk-regex Delta

## ADDED Requirements

### Requirement: `Regex` text form

`Regex` SHALL expose `to_string(): String` rendering the compiled pattern
wrapped in its literal spelling: `re'pattern'`. The runtime entry point
`zirk_regex_to_string` SHALL back the member.

#### Scenario: Pattern render

- **WHEN** `re'a+'.to_string()` runs
- **THEN** the result is `re'a+'`

#### Scenario: Regex prints directly

- **WHEN** a `Regex` value is passed to `println`
- **THEN** the check succeeds and the `re'…'` rendering is printed
