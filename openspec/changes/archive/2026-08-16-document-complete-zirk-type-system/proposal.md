## Why

The handbook names Zirk's built-in and domain types but does not yet teach them with enough depth to serve as a language reference: classification, representation semantics, mutability, conversions, operators, methods, errors, and interactions are scattered or abbreviated. The recent authorial decisions also add a complete temporal family and settle native operator behavior, so the public documentation and normative specs must be made coherent before implementation continues.

## What Changes

- Add a progressive type-system introduction explaining primitives, native reference types, user-defined value/reference types, special types, contracts, and the conceptual type tree.
- Expand every built-in type chapter with literals, construction, inference, storage semantics, mutability, aliases, conversions, operators, properties, methods, error behavior, and realistic examples.
- Replace the `Decimal*` family with `Float16`, `Float32`, `Float64`, and `Float128`, with `Float` as the `Float64` alias and no valid `NaN` value.
- Document explicit deep contextual conversion for numeric expressions and string concatenation.
- Define `Char` as exactly one Unicode grapheme and document its Unicode and ASCII inspection APIs.
- Define `String` as a mutable native reference type governed by `mut`, `inmut`, and `inmut::strict`, including aliasing, cloning, grapheme indexing and slicing, concatenation, and repetition (`"ja" * 3`).
- Add a complete temporal documentation unit for `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period`, with nanosecond precision, calendar/timeline distinctions, time-zone and DST behavior, parsing, formatting, arithmetic, and errors.
- Add authoritative native-operator matrices and method/property catalogs by type.
- Cross-link the expanded handbook chapters with nullability, collections, classes, records, value classes, enums, unions, generics, iteration, reference pages, and the previous/next navigation chain.
- Align canonical language documents and OpenSpec capabilities with the documented semantics.

## Capabilities

### New Capabilities

- `zirk-temporal-types`: Defines the temporal family, its value semantics, composition rules, arithmetic, precision, zones, calendar behavior, and controlled errors.

### Modified Capabilities

- `zirk-type-system`: Defines the complete type taxonomy, primitive/native distinction, float family, mutability and strict aliasing, contextual conversions, native operators, and per-type contracts.
- `zirk-lexical-syntax`: Adds and clarifies numeric, character, string-repetition, duration, date/time, and temporal literal forms needed by the documented surface.
- `zirk-grammar`: Defines constructors, contextual casts, temporal composition, native operator applicability, and assignment forms used by the expanded documentation.
- `language-documentation-information-architecture`: Adds a progressive type-system and temporal learning path with authoritative per-type destinations.
- `handbook-editorial-system`: Requires genuinely in-depth type chapters with executable-style examples, operator/method tables, diagnostics, cross-links, and non-uniform depth based on semantic complexity.

## Impact

The implementation phase changes Markdown documentation under `docs/`, the handbook navigation and editorial source map, canonical language/stdlib specs, and the affected main OpenSpec specs. It does not implement compiler or runtime code, but it establishes normative public semantics that the compiler phases must subsequently follow. Existing Phase 3 planning must be audited where object/value semantics, contracts, equality, cloning, or operators overlap.
