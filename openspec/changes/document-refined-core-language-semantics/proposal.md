## Why

Several core-language chapters still describe only a phase-limited subset, while the authorial design now defines escaping closures, writable function types, projection-copy semantics, abstract object contracts, richer generics, algebraic data, and collection behavior. These decisions must become one coherent public contract before other agents continue compiler and runtime implementation against outdated assumptions.

## What Changes

- **BREAKING** Add `Fn(...) => R` (`Function(...) => R`) as the writable structural callable type and permit automatically managed escaping closures.
- Define contextual lambda inference, capture cells, callable identity/cloning, bound and unbound methods, generators, pipelines, and callable contracts.
- **BREAKING** Define reference projection reads as independent deep copies while whole-reference assignment, passing, return, and capture preserve aliasing; place expressions continue to mutate original storage.
- **BREAKING** Remove language-level properties in favor of attributes and conventional `get_`/`set_` methods; default-initialize omitted attributes by type.
- Define `override fn`, ordinary `super`, virtual dispatch defaults, abstract classes adopted through `implements`, state-free traits, interface composition, and explicit derivation.
- Complete generic syntax and semantics: multiple `from` constraints, defaults, inference, declared variance, recursion, managed `Box<T>`, runtime identity, and monomorphization.
- Complete tuples, records, traditional/algebraic enums, unions, destructuring, exhaustive pattern matching, and enum behavior through external functions.
- Complete arrays, lists, maps, sets, ranges, slices, iteration, views, copying, invalidation, complexity, and controlled capacity errors.
- Update normative documents, handbook chapters, reference material, examples, navigation, and active Phase 3 planning artifacts that contain superseded rules.
- Exclude error/`Result`/exception proposals and later resource, memory, concurrency, and decorator decisions until their exploration rounds are accepted.

## Capabilities

### New Capabilities

- `zirk-callables`: Function types, callable compatibility, lambdas, escaping closures, capture behavior, generators, bound methods, and pipelines.
- `zirk-classes`: Attributes, construction, inheritance, abstract object contracts, dispatch, identity, projection-copy behavior, cloning, and casts.
- `zirk-contracts`: Interfaces, state-free traits, implementation and conflict rules, operator/callable/iteration contracts, and explicit derivation.
- `zirk-generics`: Type parameters, `from` constraints, inference, defaults, variance, recursion, identity, and monomorphization.
- `zirk-data-types`: Tuples, records, value-oriented data, traditional and algebraic enums, unions, and destructuring semantics.
- `zirk-collections`: Array, List, Map, Set, Range, indexing, slicing, copying, views, iteration, invalidation, complexity, and capacity behavior.
- `zirk-pattern-matching`: Exhaustive value, type, union, enum, regex, nested, and alternative patterns without guards.

### Modified Capabilities

- `zirk-grammar`: Add the accepted callable, object, generic, tuple, collection, slicing, and pattern syntax and remove phase-limited grammar assumptions.
- `zirk-type-system`: Add callable variance, escaping capture, projection-copy, object-contract, generic, data-type, collection, and exhaustive narrowing rules.
- `handbook-editorial-system`: Require cross-cutting semantic decisions to be updated consistently across normative, explanatory, reference, and active planning documents.
- `language-documentation-information-architecture`: Publish the completed core-language route and remove or resolve obsolete planned placeholders.

## Impact

- Normative sources: `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_SPEC_FINAL.md`, `docs/ZIRK_STDLIB_SPEC.md`, and the authorial inventory.
- Handbook units for bindings, functions, objects/contracts, generics, data types, collections, iteration, pattern matching, memory-facing reference explanations, and reference tables.
- Main OpenSpec capabilities named above and active `fase-3-objects-and-type-system` artifacts whose temporary D9/property/abstract-class/projection assumptions are superseded.
- Future parser, checker, lowering, runtime memory, collection, and code-generation work; this change documents requirements and does not implement compiler/runtime features.
