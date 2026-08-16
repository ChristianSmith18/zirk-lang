## 1. Canonical Semantic Checkpoint

- [x] 1.1 Update `docs/ZIRK_SPEC_FINAL.md` with the accepted type-system and temporal authorial checkpoint
- [x] 1.2 Rewrite the relevant type, conversion, mutability, operator, String, and temporal sections of `docs/ZIRK_LANGUAGE_SPEC.md`
- [x] 1.3 Expand `docs/ZIRK_STDLIB_SPEC.md` with the native per-type APIs, temporal module surface, parsing, formatting, zones, and controlled errors
- [x] 1.4 Correct superseded answers in `docs/01_plantilla_zirk.md`, including `Decimal*`, Char, String, strict aliases, operators, and Duration
- [x] 1.5 Align affected active Phase 3 artifacts without adding compiler implementation work that belongs to a later phase
- [x] 1.6 Sync the six change deltas into their main OpenSpec capabilities after documentation completion

## 2. Handbook Information Architecture

- [x] 2.1 Expand the everyday-types unit order with type tree, categories, value/reference semantics, contracts, conversions, and native-operator overview chapters
- [x] 2.2 Add a dedicated temporal unit to `docs/handbook/SUMMARY.md` with overview, eight type chapters, composition, arithmetic, parsing/formatting, zones/DST, and errors
- [x] 2.3 Assign one canonical owner to every type rule in the editorial source map and identify all required cross-links
- [x] 2.4 Update unit README files with learning outcomes, recommended order, and decision guidance
- [x] 2.5 Preserve every existing published destination or add an intentional redirect/link path where the hierarchy changes

## 3. Type-System Foundations

- [x] 3.1 Write the conceptual Zirk type tree and explain that `Object` is a semantic root rather than mandatory heap allocation
- [x] 3.2 Explain compiler primitives, native reference types, native value types, user-defined types, and special types with selection examples
- [x] 3.3 Explain value versus reference behavior, identity, structural equality, nominal typing, inline storage, aliasing, and cloning
- [x] 3.4 Rewrite `mut`, `inmut`, and `inmut::strict` around separate reassignment/referent permissions and strict alias invariants
- [x] 3.5 Explain capability contracts and reserved operator methods without implying that native types can be reopened
- [x] 3.6 Write the conversion model: safe promotions, explicit lossy conversions, mixed numerics, deep contextual conversion, and its boundaries
- [x] 3.7 Write the native-operator overview with operand/result types, compound assignment behavior, precedence links, and controlled errors

## 4. Signed and Unsigned Integer Documentation

- [x] 4.1 Expand signed integer types, widths, aliases, literal inference, ranges, construction, defaults, and storage semantics
- [x] 4.2 Expand unsigned integer types, valid domains, signed/unsigned conversion rules, and absence of unary negation
- [x] 4.3 Document integer arithmetic, comparison, increment, compound assignment, bitwise operations, and shifts with valid and invalid examples
- [x] 4.4 Document truncating division, dividend-signed remainder, negative powers, zero division, and result types
- [x] 4.5 Document controlled overflow and checked, wrapping, and saturating operation families
- [x] 4.6 Publish the integer property/method catalog including numeric, parity, bit-count, zero-count, rotation, bounds, and string conversion APIs

## 5. Float Documentation

- [x] 5.1 Replace handbook `Decimal*` references with `Float16`–`Float128`, `Float`/`Float64`, and explain the future separate exact-decimal use case
- [x] 5.2 Document Float literals, inference, construction, conversions, mixed arithmetic, comparison, precision, and target-support status
- [x] 5.3 Explain deep contextual Float evaluation with nested desugarings, function-boundary counterexamples, negative powers, and unchanged operands
- [x] 5.4 Document positive/negative infinity, the absence of valid `NaN`, and controlled division, overflow, and indeterminate-operation errors
- [x] 5.5 Publish the complete Float property/method catalog for rounding, fractional inspection, finiteness, bounds, and conversions

## 6. Boolean and Char Documentation

- [x] 6.1 Expand Boolean with strict conditions, logical short-circuiting, equality, rejected truthiness/order/arithmetic, and `to_string()`
- [x] 6.2 Rewrite Char as exactly one Unicode grapheme and distinguish graphemes, code points, bytes, and rendered appearance
- [x] 6.3 Document Char literal validation, comparison, Unicode classification, normalization, byte/code-point access, and String-returning case conversion
- [x] 6.4 Document `ascii_code()` returning an ASCII code or `-1`, with ASCII, accented, combining, and emoji examples

## 7. String Documentation

- [x] 7.1 Rewrite String as a mutable native reference type with content equality, observable alias identity, shared mutation, and explicit deep cloning
- [x] 7.2 Apply `mut`, `inmut`, and `inmut::strict` to String reassignment, element mutation, function calls, aliases, and clones with valid/invalid examples
- [x] 7.3 Document grapheme-based `length`, iteration, positive/negative indexing, individual Char assignment, and strict out-of-range errors
- [x] 7.4 Document slicing components, reverse slicing, equal-grapheme-length slice assignment, and `slice length mismatch` diagnostics
- [x] 7.5 Document String concatenation, rejected implicit conversion, explicit conversion, and deep contextual `String(...)` conversion
- [x] 7.6 Document `String * Integer`, `Integer * String`, `*=`, zero/one counts, negative/non-integer rejection, mutability, and checked allocation
- [x] 7.7 Publish the complete String property/method catalog for search, prefix/suffix, replacement, trimming, case, splitting, lines, substrings, normalization, graphemes, code points, bytes, cloning, and conversion
- [x] 7.8 Explain locale-independent operator ordering versus explicit locale-aware collation

## 8. Special and Cross-Cutting Types

- [x] 8.1 Expand `Null`, `T?`, safe access, coalescing, equality, flow narrowing, and rejected non-null assignment
- [x] 8.2 Expand `Void` and distinguish normal no-value completion from `Null`
- [x] 8.3 Expand `Never` with non-returning calls, divergent loops, branch typing, and control-flow effects
- [x] 8.4 Expand `Object` as the conceptual root with universal `type`/`to_string()` and contract-gated equality, hashing, and cloning

## 9. Temporal Foundations

- [x] 9.1 Write the Temporal overview and decision guide for birthdays, appointments, instants, zoned events, elapsed time, and calendar recurrence
- [x] 9.2 Document the sealed Temporal family and its calendar, clock, timeline, and zone capability groupings
- [x] 9.3 Publish the temporal construction/composition matrix and reject unsupported combinations with explanations
- [x] 9.4 Explain immutable value semantics, nanosecond precision, conceptual Int128 range, overflow, and new-value transformations

## 10. Date, Time, and DateTime

- [x] 10.1 Write the Date chapter with 1-based construction, parsing, validation, properties, comparison, formatting, weekday/month APIs, and errors
- [x] 10.2 Document Date and Period arithmetic, default end-of-month clamping, `add_strict`, date differences, start/end helpers, and replacement methods
- [x] 10.3 Write the Time chapter with components through nanoseconds, construction, validation, comparison, formatting, and Duration arithmetic
- [x] 10.4 Define and explain `TimeShift`, including positive and negative day offsets when Time arithmetic crosses midnight
- [x] 10.5 Write the DateTime chapter with Date/Time composition, local comparison, Duration/Period arithmetic, component replacement, parsing, and formatting
- [x] 10.6 Explain why DateTime is not an absolute instant until a TimeZone and ambiguity policy are supplied

## 11. Instant, TimeZone, and ZonedDateTime

- [x] 11.1 Write the Instant chapter with Unix constructors/properties, ISO parsing, comparison, Duration arithmetic, differences, UTC, and zone conversion
- [x] 11.2 Write the TimeZone chapter with canonical IANA identity, system/UTC zones, offsets, abbreviations, DST inspection, and invalid-zone errors
- [x] 11.3 Distinguish fixed UTC offsets from zones with historical and daylight-saving rules
- [x] 11.4 Write the ZonedDateTime chapter with construction, local components, instant equality/order, conversion preserving instant, parsing, and formatting
- [x] 11.5 Explain Duration-versus-Period arithmetic across 23/24/25-hour DST days with concrete valid examples
- [x] 11.6 Document rejected-by-default ambiguous/nonexistent local times and explicit earlier/later/previous-valid/next-valid policies

## 12. Duration and Period

- [x] 12.1 Write Duration literals and constructors from nanoseconds through weeks, named components, compact parsing, and ISO 8601 parsing
- [x] 12.2 Explain signed exact Duration semantics with differences, forward/backward movement, deadlines, early/late results, and non-negative wait API validation
- [x] 12.3 Document normalized component properties, Float total-unit methods, integer whole-unit methods, precision, and overflow
- [x] 12.4 Document Duration unary, arithmetic, scalar, ratio, remainder, comparison, and compound operators with result types and errors
- [x] 12.5 Publish the Duration method catalog for sign, absolute value, bounds, rounding, truncation, formatting, humanization, ISO output, and conversion
- [x] 12.6 Write Period construction from years/months/weeks/days, component arithmetic, equality, ISO parsing, and the absence of context-free totals/order
- [x] 12.7 Document contextual Period-to-Duration conversion and end-of-month behavior across Date, DateTime, and ZonedDateTime

## 13. User-Defined and Collection Type Integration

- [x] 13.1 Expand class documentation for nominal reference semantics, equality versus identity, binding strictness, contracts, and clone behavior
- [x] 13.2 Expand record documentation for immutable structural value semantics and derived equality/hash constraints
- [x] 13.3 Expand value-class documentation for nominal inline values, representation conversion, identity prohibition, and operator contracts
- [x] 13.4 Expand traditional/algebraic enum documentation for mappings, associated values, equality, ordering only by explicit contract, and matching
- [x] 13.5 Expand alias and union documentation for preserved versus common operator sets and required discrimination
- [x] 13.6 Update arrays, lists, maps, sets, ranges, iteration, and generic chapters where type categories, mutability, strict aliases, equality, indexing, or contracts interact

## 14. Reference Material

- [x] 14.1 Replace the built-in-types reference with a categorized complete type catalog and aliases
- [x] 14.2 Publish the native operator matrix for numeric, Boolean, Char, String, temporal, special, domain, and reference types
- [x] 14.3 Publish per-type property and method indexes linked to their explanatory chapters
- [x] 14.4 Add temporal constructor, composition, unit, formatting, parsing, error, and DST-policy reference tables
- [x] 14.5 Update keywords, literals, grammar summary, feature-status matrix, glossary, comparisons with other languages, and valid/invalid examples

## 15. Navigation and Verification

- [x] 15.1 Regenerate previous/next links for the final `SUMMARY.md` order and verify every published page appears exactly once
- [x] 15.2 Audit all relative links, anchors, Markdown fences, headings, tables, and code examples
- [x] 15.3 Audit repository-wide contradictions for `Decimal*`, Char/code-point claims, String immutability/copy-on-write, strict aliases, signed Duration, temporal composition, and native operators
- [x] 15.4 Verify every complex type chapter satisfies the editorial depth contract and every compact type omits sections only when inapplicable
- [x] 15.5 Validate the new change and all OpenSpec artifacts with strict mode
- [x] 15.6 Confirm the final diff is documentation/OpenSpec-only and record completion in the archived change
