## ADDED Requirements

### Requirement: Tokenization of the subset

The lexer SHALL convert `.zrk` source text into a sequence of tokens, each with its location in the source.

The subset's tokens are: identifiers, keywords, integer literals, string literals, boolean literals, operators, delimiters, and end of file.

#### Scenario: Minimal program
- **WHEN** `fn main(): Void { }` is tokenized
- **THEN** the following sequence is produced: keyword `fn`, identifier `main`, `(`, `)`, `:`, type identifier `Void`, `{`, `}`, end of file

#### Scenario: Location of every token
- **WHEN** any input is tokenized
- **THEN** every token exposes file, line, and starting column, 1-based
- **AND** the column counts Unicode characters, not bytes

### Requirement: Integer literals

The lexer SHALL recognize decimal integer literals, allowing `_` as a separator per `ZIRK_LANGUAGE_SPEC.md` section 3.

#### Scenario: Simple integer
- **WHEN** `42` is tokenized
- **THEN** an integer literal with value 42 is produced

#### Scenario: Thousands separator
- **WHEN** `1_000_000` is tokenized
- **THEN** an integer literal with value 1000000 is produced

#### Scenario: Separator in an invalid position
- **WHEN** `_1000` or `1000_` is tokenized
- **THEN** an error diagnostic with a stable code, cause, and help is emitted

### Requirement: String literals

The lexer SHALL recognize string literals delimited by double quotes, with escape sequences.

#### Scenario: Simple string
- **WHEN** `"Hola"` is tokenized
- **THEN** a string literal with content `Hola` is produced

#### Scenario: Escape sequences
- **WHEN** a string contains `\n`, `\t`, `\"`, or `\\`
- **THEN** the literal represents them as newline, tab, quote, and backslash

#### Scenario: Unterminated string
- **WHEN** a string is not closed before the end of line or end of file
- **THEN** a diagnostic is emitted pointing to the opening of the string
- **AND** the help indicates that the closing quote is missing

#### Scenario: Unknown escape
- **WHEN** a string contains an unrecognized escape sequence
- **THEN** a diagnostic is emitted pointing to the sequence

### Requirement: Boolean literals

The lexer SHALL recognize `true` and `false` as boolean literals, not as identifiers.

#### Scenario: Boolean values
- **WHEN** `true` or `false` is tokenized
- **THEN** a boolean literal is produced

### Requirement: Comments

The lexer SHALL recognize line comments `//` and block comments `/* ... */`, discarding them from the token sequence.

#### Scenario: Line comment
- **WHEN** a line contains `// text`
- **THEN** the content from `//` to the end of the line produces no tokens

#### Scenario: Block comment
- **WHEN** the source contains `/* text */`
- **THEN** the delimited content produces no tokens

#### Scenario: Unterminated block comment
- **WHEN** a block comment is not closed before the end of file
- **THEN** a diagnostic is emitted pointing to the opening

### Requirement: Reserved words of the full language

The lexer SHALL recognize as keywords those of the full language, not only those of the implemented subset.

Recognizing them lets the parser distinguish an unimplemented construct from a syntax error, and makes the diagnostic comprehensible.

#### Scenario: Keyword outside the subset
- **WHEN** `class`, `for`, `match`, `task`, or another keyword of the full language is tokenized
- **THEN** the corresponding keyword token is produced
- **AND** an identifier is NOT produced

### Requirement: Case sensitivity

The lexer SHALL distinguish uppercase from lowercase, per `ZIRK_LANGUAGE_SPEC.md` section 1.

#### Scenario: Identifier differing only in capitalization
- **WHEN** `total` and `Total` are tokenized
- **THEN** two distinct identifiers are produced

### Requirement: Unrecognized character

The lexer SHALL emit a diagnostic for any character that does not belong to the lexicon, instead of discarding it silently.

#### Scenario: Invalid character
- **WHEN** the source contains a character that does not begin any valid token
- **THEN** a diagnostic is emitted with the exact location of the character
