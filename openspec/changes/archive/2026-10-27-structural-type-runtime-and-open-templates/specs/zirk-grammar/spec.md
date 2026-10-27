## ADDED Requirements

### Requirement: Open template types reuse Zirk's type-string interpolation syntax
The parser SHALL accept open template-literal types using Zirk's existing type-string form, with `{TypeParameter}` interpolations inside quoted type strings, including fixed prefix and suffix text. It SHALL preserve interpolation identity and source spans for generic checking. The parser SHALL distinguish type-level template syntax from runtime string interpolation and SHALL provide a targeted diagnostic for malformed or unsupported open-pattern forms.

#### Scenario: Parse an open suffix template type
- **WHEN** a generic type or callable signature contains the type string `"{K}Changed"`
- **THEN** the parser produces a template type with a K interpolation and the literal suffix `Changed`

#### Scenario: Preserve existing finite type templates
- **WHEN** a type string interpolates a finite alias such as `"color-{Color}"`
- **THEN** it continues to parse as the existing finite template type

#### Scenario: Runtime string interpolation remains ordinary expression syntax
- **WHEN** a runtime string contains `{eventName}` in an expression
- **THEN** it remains an expression interpolation and is not treated as an open type pattern

#### Scenario: Unsupported open pattern receives a targeted diagnostic
- **WHEN** an open template type contains multiple unsupported inference holes or malformed braces
- **THEN** parsing or type analysis reports the pattern limitation at the relevant source span
