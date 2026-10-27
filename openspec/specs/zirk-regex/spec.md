# zirk-regex Specification

## Purpose
Defines the Regex value surface, pattern literals, matching, replacement, iteration, and error behavior.

## Requirements

### Requirement: `Regex` text form

Regex SHALL expose `to_string(): String` backed by `zirk_regex_to_string` and wrapped as `re'pattern'`.

#### Scenario: Regex prints directly
- **WHEN** a Regex value is passed to `println`
- **THEN** it renders `re'…'`

### Requirement: Regex literal syntax

The compiler SHALL support the `re'pattern'` literal form for plain regex patterns. The pattern SHALL be parsed at compile time and a diagnostic SHALL be emitted for invalid patterns.

#### Scenario: Valid regex literal
- **WHEN** `inmut r = re'[a-z]+';` is written
- **THEN** a `Regex` value is produced and the pattern is compiled successfully

#### Scenario: Invalid regex literal
- **WHEN** `inmut r = re'[a-z';` is written
- **THEN** a diagnostic is emitted naming the regex parse error

### Requirement: Regex type and construction

The language SHALL provide a native reference `Regex` type. `Regex.parse(pattern): Result<Regex, RegexError>` SHALL accept a dynamic `String` and return a compiled regex or an error.

#### Scenario: Compile-time vs dynamic construction
- **WHEN** `re'\d+'` is used
- **THEN** the value is a compiled `Regex`

- **WHEN** `Regex.parse(user_input)` is called
- **THEN** it returns `Result<Regex, RegexError>`

### Requirement: Match testing

`Regex.matches(text): Boolean` SHALL return whether the pattern matches the full text. `Regex.find(text): Regex.Match?` SHALL return the first match or `null`.

#### Scenario: Full match
- **WHEN** `re'\d+'.matches("123")` is evaluated
- **THEN** the result is `true`

#### Scenario: Find match
- **WHEN** `re'\d+'.find("abc 123 def")` is evaluated
- **THEN** the result is a `Regex.Match` with `text = "123"`, `start = 4`, and `end = 7`

### Requirement: Capture groups

A `Regex.Match` SHALL expose `group(n): String` for the nth positional capture and `group(name): String` for named captures. Accessing a missing group SHALL produce a controlled error.

#### Scenario: Positional capture
- **WHEN** `re'(\d+)-(\d+)'.find("10-20")` and `match.group(1)` is read
- **THEN** the result is `"10"`

#### Scenario: Named capture
- **WHEN** `re'(?<year>\d{4})'.find("2026")` and `match.group("year")` is read
- **THEN** the result is `"2026"`

### Requirement: Iteration over matches

`Regex.find_all(text): List<Regex.Match>` SHALL return all non-overlapping matches in order. The returned `List<Regex.Match>` SHALL be usable with `for ... in`. (`Regex.matches` remains the `Boolean` match test required by "Match testing"; the iteration entry point is named `find_all` to avoid the conflict.)

#### Scenario: Iterate matches
- **WHEN** `for m in re'\d+'.find_all("1 22 333")` is executed
- **THEN** the loop yields three matches with texts `"1"`, `"22"`, and `"333"`

### Requirement: Replace and split

`Regex.replace(text, replacement): String` SHALL replace all matches. `Regex.split(text): List<String>` SHALL split the text at each match.

#### Scenario: Replace
- **WHEN** `re'\d+'.replace("a1b2c", "X")` is evaluated
- **THEN** the result is `"aXbXc"`

#### Scenario: Split
- **WHEN** `re','.split("a,b,c")` is evaluated
- **THEN** the result is a `List<String>` containing `"a"`, `"b"`, and `"c"`

### Requirement: Regex integration in match statements

A `match` expression SHALL support a `re'pattern'` pattern that binds a `Regex.Match` when the scrutinee is a `String` and the pattern matches.

#### Scenario: Match with regex pattern
- **WHEN** `match s { re'(?<name>\w+)' => { m } }` is evaluated with `s = "devin"`
- **THEN** the branch is taken and `m` is a `Regex.Match` with `group("name") = "devin"`

### Requirement: Native compiled-pattern retention is bounded

The native runtime SHALL impose a deterministic bound on compiled Regex cache entries or retained bytes. Cache eviction or limit failure SHALL not invalidate a Regex value still reachable by a program. User-controlled patterns SHALL not cause unbounded process-lifetime retention.

#### Scenario: Many distinct patterns are parsed
- **WHEN** a program repeatedly parses distinct user-controlled patterns beyond the configured cache bound
- **THEN** retained compiled-pattern memory stays within the bound and parsing follows a deterministic eviction or controlled limit policy

#### Scenario: Cached pattern is reused
- **WHEN** a pattern is compiled and remains in the cache for a repeated parse
- **THEN** the compiled matcher is reused without compiling the same pattern twice for one parse operation
