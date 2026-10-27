# zirk-generics Specification

## Purpose
Defines generic parameters, constraints, defaults, inference, variance,
specialization, recursion, and runtime type identity.
## Requirements

### Requirement: Generic declarations, constraints, defaults, and inference

Generic parameters SHALL use `<T>`, combined constraints SHALL use `T from A & B`, and trailing parameters MAY declare defaults satisfying their constraints. Inference SHALL use arguments, receiver, expected result, callable context, and constraints and SHALL fail rather than choose an arbitrary solution.

#### Scenario: Multiple missing constraints

- **WHEN** a concrete type is supplied for `T from Clone & Serializable` and lacks Serializable
- **THEN** the call-site diagnostic identifies T and the missing Serializable contract

### Requirement: Declared generic variance

Generic parameters SHALL be invariant by default. `out T` SHALL be legal only in covariant output positions, `in T` only in contravariant input positions, and any mutable attribute use SHALL require invariance. `Fn` SHALL retain its intrinsic parameter/result variance.

#### Scenario: Mutable List invariance

- **WHEN** Dog extends Animal and `List<Dog>` is supplied as `List<Animal>`
- **THEN** compilation fails because List permits insertion

### Requirement: Generic identity and monomorphization

Concrete instantiations SHALL retain distinct static and runtime type identity. Generic bodies SHALL be checked once against constraints; portable IR SHALL retain generic information and final builds MAY monomorphize/share code only when ABI and observable semantics remain unchanged.

#### Scenario: Distinct runtime instantiations

- **WHEN** runtime type identity is requested for List<String> and List<Int32>
- **THEN** the two complete instantiations remain distinguishable

### Requirement: Type parameters

Functions, classes, and data types SHALL support type parameters with the `<T>` syntax, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Generic function

- **WHEN** `fn identity<T>(value: T): T { return value; }` is declared
- **THEN** `identity(1)` produces an `Int32` and `identity("a")` a `String`

#### Scenario: Generic class

- **WHEN** `class Box<T>` is declared with a field of type `T`
- **THEN** `Box<Int32>` and `Box<String>` are distinct types

#### Scenario: Unused type parameter

- **WHEN** a type parameter that does not appear in the signature is declared
- **THEN** a diagnostic naming it is emitted

### Requirement: Constraints with `from`

A type parameter SHALL support constraints with `from`, and the checker SHALL verify them at the use site.

#### Scenario: Argument satisfying the constraint

- **WHEN** `serialize<T from Serializable>` is called with a type that implements `Serializable`
- **THEN** the check succeeds

#### Scenario: Argument that does not satisfy it

- **WHEN** it is called with a type that does not implement the contract
- **THEN** a diagnostic naming the concrete type and the missing contract is emitted

#### Scenario: The body only uses what the constraint guarantees

- **WHEN** the body of a generic function calls a method that the constraint does not declare
- **THEN** a diagnostic is emitted
- **AND** the help indicates that the constraint must declare it

### Requirement: The generic is checked once

The checker SHALL verify the body of a generic declaration exactly once against its constraints, not once per instantiation.

#### Scenario: Error in the body is reported once

- **WHEN** a generic function with a type error is instantiated with three distinct types
- **THEN** the error is reported exactly once, at the declaration

### Requirement: Scope of generics in this phase

The checker SHALL NOT support associated types or higher-kinded type
parameters. Declared variance (`in T` / `out T`) is part of the language and
SHALL be verified per "Declared generic variance".

None of the remaining exclusions are in `ZIRK_LANGUAGE_SPEC.md`, and
supporting them would fix semantics that the spec does not fix.

#### Scenario: Unsupported construct

- **WHEN** an associated type or a higher-kinded type parameter is written
- **THEN** a diagnostic indicating that it is not part of the language is emitted

### Requirement: Dispatch through a generic contract reference

A call through a receiver of generic contract type SHALL dispatch to the
concrete adopter's method lowered under that instantiation's substitution.
The observable result SHALL be identical to calling the adopter's method
directly.

#### Scenario: Call through generic contract reference

- **WHEN** a value of `Box<Int32>` (which implements `Iterable<T>`) is held through `Iterable<Int32>` and `iterator()` is invoked
- **THEN** the call dispatches to `Box<Int32>`'s own `iterator()` implementation

#### Scenario: Generic contract as a parameter type

- **WHEN** a function declares a parameter of type `Iterable<String>` and is called with a `Box<String>`
- **THEN** contract calls inside the function dispatch to `Box<String>`'s methods

### Requirement: Generic contracts constrain structural properties and operations
Generic constraints SHALL accept `contract` declarations and SHALL preserve their substituted property and operation types at each use site. A statically known class or object shape SHALL be checked at compile time. A value whose static type is only `Object` SHALL be checked at runtime and SHALL raise a catchable contract violation if it does not satisfy the instantiated contract.

#### Scenario: Generic class satisfies a substituted contract
- **WHEN** `Box<Int32>` is passed to a generic function constrained by a contract requiring `value: T`
- **THEN** the checker substitutes `Int32` and validates the required property type

#### Scenario: Dynamic object fails a generic contract
- **WHEN** a dynamic object is passed where a runtime-checked `Contract<String>` is required but its property has an incompatible value
- **THEN** a catchable contract violation is raised before the constrained operation executes

### Requirement: Generic classes preserve contract dispatch layout
Generic class contract methods SHALL use substituted parameter and result types at every call site. Vtable method offsets SHALL remain stable across generic instantiations that share the same contract declaration.

#### Scenario: Generic class contract dispatch is stable
- **WHEN** `Box<Int32>` and `Box<String>` both implement the same generic contract
- **THEN** each dispatches to its own substituted implementation using the corresponding stable vtable layout

### Requirement: Generic recursive substitution is bounded
Generic substitution SHALL be guarded against infinite recursion. Mutually recursive generic classes, contracts, aliases, and constraints SHALL produce a controlled diagnostic instead of exhausting the compiler stack.

#### Scenario: Recursive generic alias
- **WHEN** `type Nested<T> = Box<Nested<T>>;` is written
- **THEN** a diagnostic is emitted before the compiler recurses without bound

### Requirement: Generic specialization remains representation-independent
Lowering SHALL produce one specialization for each concrete combination of type arguments actually used, and MAY share code only when ABI and observable semantics remain unchanged. This rule SHALL NOT require inline storage for dynamic objects.

#### Scenario: Repeated generic instantiation
- **WHEN** the same generic function is used multiple times with the same argument types
- **THEN** the compiler may reuse a single compatible specialization

### Requirement: Generic type aliases accept complete type expressions
Type aliases SHALL accept type parameters, constraints, defaults, and complete nested type expressions in their target. Alias substitution SHALL preserve canonical union/intersection identity and source alias names for diagnostics. Recursive aliases SHALL be accepted only when guarded by a value-bearing type constructor and SHALL terminate under the type-computation budget.

#### Scenario: Generic alias substitutes a union
- **WHEN** type Outcome<T, E> = Success<T> | Failure<E> is instantiated
- **THEN** each parameter is substituted in its corresponding alternative

#### Scenario: Defaulted alias parameter
- **WHEN** a generic alias omits a trailing type argument with a declared default
- **THEN** that default is substituted and checked against its constraint

#### Scenario: Unguarded recursive alias is rejected
- **WHEN** an alias recursively names itself without a guarding constructor
- **THEN** compilation reports a recursive-type diagnostic rather than recursing indefinitely

### Requirement: Type operators provide structural queries and transformations
The type language SHALL provide keyof, indexed access, conditional types with infer, mapped types with key remapping and optional/read-only modifier control, and finite template-literal types with intrinsic upper/lower/capitalize/uncapitalize transformations. Operators SHALL compose with generic parameters, unions, intersections, aliases, tuples, and structural object types. Distributive conditional behavior SHALL be explicit and deterministic. Type-level operators SHALL be erased before runtime and SHALL NOT permit access to private or otherwise invisible members.

#### Scenario: Key query returns literal keys
- **WHEN** keyof is applied to a structural type with public properties
- **THEN** it produces a normalized union of those property-name singleton types

#### Scenario: Indexed access distributes over key unions
- **WHEN** T[K] uses a finite union of valid keys
- **THEN** it produces the normalized union of the corresponding property types

#### Scenario: Conditional type distributes over a naked parameter
- **WHEN** a distributive conditional type receives a union argument
- **THEN** the conditional is evaluated for each member and the results are normalized

#### Scenario: Conditional inference binds a matched component
- **WHEN** a conditional type uses infer to match a generic component
- **THEN** the inferred type is scoped to the true branch and satisfies all constraints

#### Scenario: Mapped type preserves or changes modifiers
- **WHEN** a mapped type transforms a structural type's properties
- **THEN** key selection, remapping, optionality, read-only state, and value transformation follow the written modifiers

#### Scenario: Template literal type expands finite alternatives
- **WHEN** a template-literal type interpolates finite string, numeric, Boolean, or other string-convertible singleton unions
- **THEN** the compiler produces the finite normalized string union within its expansion budget

#### Scenario: Intrinsic string transformation maps singleton unions
- **WHEN** Uppercase, Lowercase, Capitalize, or Uncapitalize is applied to a finite String singleton union
- **THEN** each value is transformed according to the documented Unicode policy and the results are normalized

#### Scenario: Private member is not exposed by type operators
- **WHEN** keyof or indexed access is evaluated outside the declaring visibility scope
- **THEN** inaccessible members are excluded or rejected under ordinary visibility rules

### Requirement: Standard type utilities are available without runtime code
The standard type prelude SHALL provide generic aliases for partial/required/read-only properties, property selection/omission/record construction, union extraction/exclusion, null removal, and callable parameter/result extraction, plus task-result unwrapping where a Zirk task type applies. Utilities SHALL preserve generic constraints, modifiers, and visibility and SHALL be expressible by the public type operators.

#### Scenario: Utility type selects properties
- **WHEN** Pick<T, K> is applied to a structural type
- **THEN** the result contains exactly the selected accessible properties with their declared types

#### Scenario: Utility type builds a record shape
- **WHEN** Record<K, V> is applied to a finite singleton key union
- **THEN** the result has exactly those keys with value type V

#### Scenario: Union utility excludes members
- **WHEN** Exclude<T, U> is applied to a finite union
- **THEN** members assignable to U are removed while remaining members preserve their canonical identity

#### Scenario: Callable utility extracts a result
- **WHEN** ReturnType<F> is applied to a supported callable type
- **THEN** it resolves to the callable's checked result type

#### Scenario: String utility transforms literal keys
- **WHEN** an intrinsic case utility is applied to a finite string singleton union
- **THEN** it returns the corresponding finite transformed singleton union

### Requirement: Generic inference composes union and intersection constraints
Generic inference SHALL collect constraints from arguments, receivers, expected results, callable contexts, and type operators. It SHALL solve union and intersection constraints without arbitrary first-match selection, honor declared variance, preserve literal information when required by context, and issue a focused diagnostic when several incompatible solutions remain.

#### Scenario: Inference combines argument and expected result
- **WHEN** a generic function's argument and expected result contribute compatible constraints on T
- **THEN** the checker selects the unique canonical solution

#### Scenario: Inference rejects ambiguous candidates
- **WHEN** equally valid candidates imply incompatible substitutions
- **THEN** the call reports ambiguity and identifies the conflicting constraints

#### Scenario: Mutable generic remains invariant
- **WHEN** a union or intersection argument would require unsound covariance through a mutable generic container
- **THEN** assignment is rejected under declared variance rules

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
