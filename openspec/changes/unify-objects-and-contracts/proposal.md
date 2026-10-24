## Why

Zirk currently splits structural data requirements between `record` and `interface`, while `Object` does not provide the open, dynamically extensible object model expected for JSON-shaped data. This proposal consolidates structural contracts, makes ordinary objects the single representation for dynamic object data, and defines how mutability and contract conformance constrain those objects.

## What Changes

- **BREAKING**: Replace `record` and `interface` declarations and their separate semantics with `contract`, a structural declaration for required properties and operations. Keep `trait` for reusable behavior and default implementations; keep classes and abstract classes for nominal object-oriented behavior.
- **BREAKING**: Define `Object` as the reference-object supertype and open dynamic object view, including class instances with dynamic properties. It is not the universal root type of scalars and other Zirk values.
- Allow object literals to use identifier-shaped keys without quotes, quoted string keys, and interpolated string keys such as `"{key}"`. Support computed property access such as `object[key]` alongside named access.
- Allow open objects to add, update, read, and remove properties subject to Zirk mutability. Missing direct reads raise a catchable runtime error; provide an explicit presence-safe lookup path.
- When an object is assigned or checked against a contract, validate the required properties and operations. A successful conformance check seals the object's complete existing key set across aliases; extra keys remain present but are not accessible through contract-typed views. Contract-declared property mutability governs writes through those views.
- Use compile-time conformance checks when the source shape is statically known and a catchable runtime contract violation when it is dynamic. Define callable object properties as eligible to satisfy operation requirements.
- Normalize JSON object values to ordinary `Object` values and serialize them back to JSON. Preserve native scalar and array values, JSON `null`, and JSON number behavior; reject values that cannot be represented in JSON with a catchable error.
- Retire record-specific value copying and structural equality as implicit semantics. Specify reference identity and mutation behavior for objects, and rely on explicit cloning/equality behavior where available.
- Migrate examples, diagnostics, editor support, handbook references, and implementation status to the unified model. Update the companion `../zirk-lang-site` after the source documentation is committed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-contracts`: Replace interface-based structural contracts and define property requirements, structural conformance, sealing, and operation satisfaction.
- `zirk-data-types`: Replace record values with dynamic reference objects and define object/JSON conversion semantics.
- `zirk-grammar`: Add `contract` declarations, object literal key forms, interpolation keys, and computed property access; retire `record` and `interface` syntax.
- `zirk-type-system`: Define structural contract assignment, open-object typing, dynamic values, and mutability checks.
- `zirk-classes`: Define class conformance to contracts and the interaction between class instances and dynamic properties.
- `zirk-object-memory`: Define object property storage, alias-visible sealing, identity, and tracing for dynamic values.
- `zirk-memory-safety`: Define ownership, mutation, and lifetime rules for reference-based dynamic objects.
- `spread-rest-operators`: Define spread and rest behavior for open objects and contract-constrained views.
- `zirk-pattern-matching`: Define matching and binding behavior for dynamic objects and contracts.
- `zirk-generics`: Define generic constraints and nested dynamic object types using contracts.
- `zirk-callables`: Define callable values stored as dynamic properties and their contract conformance.
- `zirk-collections`: Define object values nested in collections and their managed references.
- `zirk-errors`: Define catchable failures for missing dynamic properties, failed runtime conformance, and invalid JSON conversion.
- `return-type-inference`: Define inference for object literals and contract-typed results.
- `universal-string-rendering`: Define rendering for objects after record rendering is retired.
- `zirk-ir-lowering`: Define lowering for contract views, dynamic object literals, access, and JSON conversion.
- `zirk-native-codegen`: Define native representation and operations for dynamic objects and runtime contract checks.
- `zirk-ir-interpreter`: Define interpreter values and behavior for dynamic objects and runtime conformance checks.

## Impact

This changes normative language syntax and semantics, public examples, compiler diagnostics, and the delivered-feature/roadmap descriptions. Implementation work spans the lexer/parser/AST, type checker, IR and interpreter, native code generation, runtime object storage and garbage collection, standard-library JSON handling, formatter, LSP/editor support, and migration of examples and documentation. Dynamic property reads without a statically known type use a tagged `Dynamic` value that must be narrowed before typed use. Existing programs using `record` or `interface` require migration to `contract` and `Object`.

Public handbook and status changes also affect the companion `../zirk-lang-site`; after the source documentation is committed, synchronize the site, review its diff and status catalog, and validate both repositories.
