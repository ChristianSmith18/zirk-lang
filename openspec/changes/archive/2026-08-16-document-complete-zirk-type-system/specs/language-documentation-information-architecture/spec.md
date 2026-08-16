## ADDED Requirements

### Requirement: Progressive type-system learning path
The handbook SHALL introduce the Zirk type taxonomy, conceptual tree, primitive/native/user-defined/special categories, value/reference behavior, contracts, conversions, and native operator model before relying on those concepts in individual type chapters.

#### Scenario: Reader enters everyday types
- **WHEN** a reader follows the canonical handbook sequence into the type-system unit
- **THEN** the conceptual model precedes numeric, Boolean, Char, String, and special-type API chapters

### Requirement: Dedicated temporal unit
The handbook SHALL provide a dedicated ordered temporal unit covering `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period`, followed by composition, arithmetic, parsing/formatting, DST, and error guidance.

#### Scenario: Reader chooses temporal type
- **WHEN** a developer needs to represent a birthday, local appointment, absolute event, timeout, or calendar recurrence
- **THEN** the temporal overview directs them to a distinct appropriate type and explains why

### Requirement: Canonical type cross-links
Every detailed type chapter SHALL link to its conceptual owner, closely interacting types, operator reference, and adjacent previous/next handbook chapters without duplicating normative definitions inconsistently.

#### Scenario: String reader follows semantics
- **WHEN** a reader needs strict mutability or operator details from the String chapter
- **THEN** direct links reach the binding model and operator reference while the String chapter remains the primary owner of String behavior
