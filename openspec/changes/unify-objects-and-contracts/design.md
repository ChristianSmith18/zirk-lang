## Context

See `proposal.md` for the motivation and scope. The existing language model has three relevant constraints: `Object` is currently documented as a conceptual root for every value, records are inline immutable values, and interfaces carry only operation signatures. The compiler has separate value and object representations, strict reference mutability rules, generated structural metadata, and a JSON-specific exact-number value tree. This change affects all of those boundaries.

## Goals / Non-Goals

**Goals:**

- Make `contract` the only structural declaration for required properties and operations.
- Make `Object` the mutable, reference-based object container for dynamic data, including JSON-shaped values.
- Keep dynamic property access type-safe through tagged `Dynamic` values and explicit narrowing.
- Preserve the distinction between structural contracts, reusable traits, nominal classes, abstract classes, and closed enums.
- Keep contract validation, object identity, key sealing, mutability, exceptions, and JSON conversion consistent across native and interpreted execution.

**Non-Goals:**

- Merge traits into contracts or add method bodies to contract declarations.
- Make scalar values instances of `Object` or make `Object` a universal `Any` type.
- Add prototype inheritance, prototype mutation, or JavaScript coercion rules.
- Preserve record value-copy or automatic structural-equality semantics under another named type.
- Change class inheritance, enum closure, tuple value semantics, or iterable spread.

## Decisions

### `contract` is structural and does not allocate a wrapper

A contract declaration describes required property types, mutability, and operation signatures. Classes may declare `implements Contract`; ordinary objects may be checked by shape. Contract values are typed views over the same reference, not wrapper allocations. For classes, declared members satisfy requirements under existing visibility and override rules. For ordinary objects, an operation may be satisfied by a callable property with a compatible signature and effects.

`trait` remains the home for reusable method bodies and conflict selection. `abstract class` remains nominal and may require state and behavior. These roles do not overlap after interfaces are removed.

Alternative considered: put default implementations in contracts. Rejected because that would combine shape and behavior reuse, duplicating the trait role and complicating conflict resolution.

### `Object` owns dynamic properties; `Dynamic` represents an unknown property value

`Object` is the common view for reference objects, including class instances, and provides access to their open string-keyed dynamic properties. It is no longer the root type of scalars, tuples, enums, or other value types. Upcasting a class instance to `Object` preserves its identity; checked casts may recover a class type. A property whose type is statically known through a class or contract has that declared type. A property read through open `Object` produces `Dynamic`, a tagged value that preserves the actual Zirk type and must be narrowed before typed use. JSON arrays become ordinary `List<Dynamic>` values; JSON object nodes become `Object`; JSON scalars remain their native values.

Class instances retain their generated fixed layout for declared members. Properties added through an `Object` view use a sidecar keyed by the class instance's managed identity. The collector traces the sidecar values while the owner is live and drops the sidecar when the owner is swept. Dynamic member lookup through an open `Object` addresses the sidecar; statically declared fields continue through normal class or contract typing.

Alternative considered: make `Object` accept every Zirk value. Rejected because it conflates object properties with the universal value type and makes member access on scalars ambiguous. Alternative considered: infer a permanent anonymous record type for each literal. Rejected because aliases and later dynamic writes make that shape stale. The narrow `Dynamic` result keeps the object model open while keeping the rest of the type system checked.

### Dynamic keys are strings and use ordinary interpolation

An unquoted identifier-shaped key in a literal denotes that literal string key. A quoted key is a string. A quoted interpolated key evaluates the same string interpolation used elsewhere and converts interpolated values through the language's normal string conversion. Computed access uses a string expression, such as `object[key]`. Keys are stored in deterministic insertion order; assigning an existing key replaces its value without changing its position, while a new key is appended. Static and runtime duplicate literal keys follow the same left-to-right replacement rule.

Alternative considered: require bracket syntax for every dynamic key. Rejected because quoted interpolation is already a natural string form and keeps ordinary object literals concise.

### Contract conformance seals the current key set on the shared object

Conformance is checked statically when the compiler knows the literal or class shape. When the only available type is `Object` or `Dynamic`, conformance runs at runtime. A successful check marks the underlying object's complete current set of own keys as sealed. Every alias sees that state. No later operation may add or remove keys, including keys not named by the contract. Those extra keys remain on the object and remain usable through an `Object` view; a contract view exposes only its declared members. Existing values may still change when both the property and the reference mutability rules permit it.

Applying another contract to a sealed object validates against the existing properties and operations and does not reopen or extend its keys. A failed check does not change the object or seal it. Class-declared fields remain part of the object's contract-checkable members; sealing prevents dynamic-key changes and does not override declared-field mutability.

Alternative considered: seal only the keys mentioned in a contract. Rejected because that would permit undeclared keys to be added through another alias after a contract promises a fixed shape. Alternative considered: wrap the object in an immutable projection. Rejected because the user's model requires alias-visible key rules and preserved identity.

### Mutability continues to govern values independently from keys

Before conformance, property insertion, replacement, and removal are mutations and follow Zirk's existing `mut`, `inmut`, and `inmut::strict` alias rules: `inmut` prevents rebinding but does not by itself freeze the object, while `inmut::strict` prevents reachable mutation. After conformance, insertion and removal are always forbidden by sealing. Updating an existing value remains subject to the property's declared mutability where the access is contract-typed and to strict-reference rules through an `Object` view. Contract property declarations reuse the language's field mutability syntax and defaults rather than introducing another modifier system.

Direct reads of absent keys raise a catchable missing-property error. A named safe lookup operation returns an optional result and avoids throwing. The exact public method spelling should align with the standard library naming conventions during implementation, while preserving this required behavior.

### Object assignment and spread use reference identity and shallow copying

Assigning an `Object` or viewing it through a contract aliases the same reference. There is no automatic deep copy or structural equality. Deep copying remains an explicit `clone()` operation governed by `Clone`. Object spread produces a new top-level identity, copies own keys in insertion order, and preserves nested reference identities, following the useful shallow-copy behavior of JavaScript object spread without adopting its coercion rules.

### JSON parses to ordinary values and stringifies only JSON-compatible values

`JSON.parse` returns `Dynamic`; each object node is an ordinary `Object`, each array is a `List<Dynamic>`, and strings, booleans, null, and representable numbers use ordinary Zirk values. The runtime preserves exact JSON number tokens so parsing and stringifying do not silently round values. Converting a dynamic number to a concrete numeric type is checked. `JSON.stringify` traverses ordinary values, rejects cycles and non-JSON runtime values with a catchable error, and returns no partial document. Existing `JSONCodec<T>` support is adapted to contract shapes; it remains generated or explicitly implemented and does not depend on runtime reflection.

Alternative considered: retain a separate public `JSONValue` object tree. Rejected because that would preserve a second object representation after `Object` becomes the normal representation for JSON objects. The exact number token is an internal tagged case of dynamic JSON data, not another user-facing object type.

### Removing `record` is an explicit breaking migration

Record declarations and record-only inline layout are removed. A former record used as a data shape migrates to a `contract` plus an `Object` literal. A former record whose methods or nominal identity are important migrates to a class; pure operations may move to ordinary functions or traits. Tuples and enums remain value aggregates. Existing automatic record copy, equality, pattern binding, pointer-to-field, and rendering behavior is not recreated implicitly.

An old `Object` parameter that intentionally accepted any value migrates to a generic type parameter when the function should preserve the input type, or to `Dynamic` when runtime-tagged heterogeneous values are intended. An old interface declaration migrates directly to `contract`. Traits remain unchanged.

## Risks / Trade-offs

- **[Breaking migration across source, libraries, examples, and generated interfaces]** → Provide mechanical diagnostics and migration examples, migrate all repository fixtures, and keep the website/status update in the same delivery workstream.
- **[Dynamic property storage adds runtime allocation, lookup, and tag overhead]** → Keep declared class fields on their existing fixed layout, reserve dynamic storage for open properties, and measure the object-heavy and JSON paths before declaring performance status.
- **[Sealing is observable through aliases and can surprise callers]** → Make the successful conformance point explicit in docs and diagnostics; failed validation leaves the object unchanged.
- **[Type-erased properties can hide managed references from collection]** → Store values with runtime tags and descriptor-visible reference metadata; add native/interpreter parity fixtures for nested objects and closures.
- **[Object rendering may be mistaken for JSON serialization]** → Keep the two APIs distinct, bound rendering, and mark cycles or unsupported values rather than emitting purported JSON.
- **[Current compiler delivery may not match the final language model]** → Keep normative semantics, implemented subset, and roadmap status separate; update public status only after the implementation evidence exists.

## Migration Plan

1. Land the language-spec and OpenSpec changes for `contract`, `Object`, `Dynamic`, mutation, sealing, JSON, and migration semantics.
2. Implement the shared typed representation and runtime object operations, then wire parser/type-checker, IR, interpreter, native code generation, GC tracing, JSON codecs, formatter, and LSP support.
3. Migrate code and fixtures from `record`/`interface` to `contract`, `Object`, class, tuple, or enum according to their semantics. Replace universal-root `Object` signatures with generic or `Dynamic` types.
4. Run compiler and editor conformance coverage, including runtime errors, alias-visible sealing, exact JSON round trips, collection tracing, and native/interpreter parity.
5. Update the handbook, error reference, examples, feature-status catalog, limitations, and roadmap based on verified delivery. Commit source documentation before synchronizing `../zirk-lang-site`; review the site-owned status catalog and both repository diffs.

Rollback is a source-level revert before release. After release, migration requires restoring the previous compiler and documentation together because the syntax and type semantics are intentionally incompatible.

## Open Questions

None. The dynamic type, JSON representation, sealing transition, migration target, and rendering boundary are decided here so implementation tasks can proceed without changing the proposal's semantics.
