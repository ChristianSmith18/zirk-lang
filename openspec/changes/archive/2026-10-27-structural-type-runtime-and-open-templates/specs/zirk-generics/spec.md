## ADDED Requirements

### Requirement: Open template-literal types support inverse inference
Template-literal types SHALL support open patterns containing one inference-bearing generic interpolation surrounded by fixed prefix and/or suffix text, in addition to the existing finite-expansion behavior. For a string singleton argument matching that pattern, generic inference SHALL recover the interpolation's exact singleton type without enumerating an unbounded string domain. The inferred type SHALL be checked against its declared constraint and SHALL compose with `keyof`, indexed access, aliases, structural types, and contextual callable typing. Open template types SHALL be erased before runtime.

#### Scenario: Infer a generic key from a suffixed event string
- **WHEN** a generic callable accepts an event type `"{K}Changed"` and the call supplies `"ageChanged"`
- **THEN** inference binds K to the singleton `"age"` without expanding all possible strings

#### Scenario: Generic contract method infers its property key from the event name
- **WHEN** `Watched<T>` declares `fn on<K from String & keyof T>(event: "{K}Changed", callback: Fn(T[K]) => Void): Void` and a receiver of type `Watched<Person>` is called as `person.on("ageChanged", callback)`
- **THEN** the contract argument `T` remains `Person`, method-local `K` is inferred as `"age"`, and the callback parameter is contextually typed as `Person["age"]`

#### Scenario: Generic contract method implementation conforms after substitution
- **WHEN** a class implements `Watched<Person>` and supplies a generic `on<K from String & keyof Person>` method with a compatible signature
- **THEN** conformance checks the same method constraint, template, indexed access, parameters, and return type without erasing the method's generic contract

#### Scenario: Generic contract dispatch uses the inferred specialization
- **WHEN** a call through a contract-typed receiver infers a concrete method substitution such as `K = "age"`
- **THEN** the interpreter and native backend dispatch to a compatible implementation specialized for that substitution without a runtime generic type value

#### Scenario: Inferred key is checked against `keyof`
- **WHEN** K is constrained to `String & keyof T` and inverse inference produces `"age"`
- **THEN** the call succeeds only if `"age"` is a visible key of T

#### Scenario: Indexed access types a callback from the inferred key
- **WHEN** a callback parameter is declared as `T[K]` after K is inferred from the event string
- **THEN** the callback receives the property type for that key and is contextually typed at the call site

#### Scenario: Unknown event name is rejected
- **WHEN** a literal string matches the fixed template text but its inferred key violates the generic constraint
- **THEN** the call reports the unsatisfied key constraint at the event argument

#### Scenario: String without a usable literal does not invent a key
- **WHEN** a broad `String` value is passed where inverse inference is required and no other argument determines K
- **THEN** inference fails with a focused diagnostic instead of widening K to an arbitrary type

#### Scenario: Pattern text does not match
- **WHEN** a literal argument lacks the required prefix or suffix of an open template
- **THEN** the argument is rejected as incompatible with the template parameter

#### Scenario: Finite template expansion remains unchanged
- **WHEN** a template interpolates finite singleton unions
- **THEN** it continues to produce the existing normalized finite union within its documented budget

### Requirement: Open template inference is deterministic and bounded
Open-template matching and generic constraint solving SHALL use deterministic, documented work budgets. A supported pattern SHALL have one inference-bearing interpolation whose value is uniquely determined by its fixed surrounding text. Ambiguous or unsupported multi-hole patterns SHALL receive a diagnostic; the compiler SHALL NOT select a split based on iteration order, enumerate arbitrary strings, or return a partial inference result.

#### Scenario: Single open interpolation has a unique split
- **WHEN** an argument literal matches a pattern with one open interpolation and fixed surrounding text
- **THEN** the interpolation is inferred from the unique matching substring

#### Scenario: Ambiguous open pattern is rejected
- **WHEN** a type pattern contains multiple independently open interpolations or has more than one valid decomposition
- **THEN** compilation reports that the pattern is unsupported or ambiguous

#### Scenario: Inference budget is exceeded
- **WHEN** pattern matching or its generic relation work exceeds the configured budget
- **THEN** compilation emits a deterministic source-located limit diagnostic and produces no partial substitution
