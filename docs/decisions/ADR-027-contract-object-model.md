# ADR-027 — Unified contracts and dynamic objects

- **Status:** accepted
- **Date:** October 23, 2026
- **Phase:** 7
- **Supersedes:** the record/interface assumptions in ADR-012 and ADR-013; module declaration inventory in ADR-016

## Context

Zirk had two parallel abstractions for named data shape: immutable inline `record` values and method-only `interface` declarations. That split duplicated structural modeling, made JSON objects a separate representation, and forced object-like data into value-copy semantics. The language needs one structural contract that describes both properties and operations, while preserving traits for reusable behavior and classes for nominal identity and state.

The accepted design and implementation requirements were tracked in the archived OpenSpec change [`unify-objects-and-contracts`](../../openspec/changes/archive/2026-10-23-unify-objects-and-contracts/). Its scoped tasks are complete; the feature-status catalog records current delivery evidence and remaining boundaries. This ADR remains the durable language and representation decision.

## Decision

### Structural declarations

`contract` is the only structural declaration. It may require typed properties with mutability requirements and operations with signatures. A contract has no storage, property initializer, or operation body. `trait` remains the mechanism for reusable operation bodies and default-conflict resolution. `abstract class` remains nominal and may require class attributes and abstract methods.

Classes may explicitly implement contracts and are checked against required properties and operations. Ordinary `Object` values may conform structurally. A contract view is a typed view of the same object reference; it does not allocate a wrapper or change identity.

### Dynamic objects and values

`Object` is an identity-bearing reference object with an open, string-keyed property set. Class instances share the common `Object` view and may carry additional statically declared fields and methods. `Object` is not a universal root type: scalars, tuples, and enums do not widen to it.

`Dynamic` is a tagged value for properties whose concrete type is not statically known. It preserves the runtime Zirk type and must be narrowed by a checked cast, type test, or `match` before concrete use. It does not introduce JavaScript coercion or unchecked access.

Object literals use bare identifier keys, quoted string keys, or quoted keys containing ordinary string interpolation, for example `{ name: "Ada", "{dynamicKey}": value }`. Computed access uses `object[key]` with a string key. New keys append in deterministic insertion order; replacing an existing key keeps its position. Object spread/rest are shallow at the top level and preserve nested reference identity.

### Conformance, aliases, and mutability

A statically known class or literal shape is checked at compile time. When only `Object` is known, conformance checks at runtime and raises a catchable contract-violation error on mismatch. Failed checks leave the object unchanged. Successful conformance seals the complete existing key set on the shared allocation, visible through every alias. Extra keys remain stored but are unavailable through the contract view. A second contract may validate a sealed object but cannot extend or reopen its keys.

Sealing constrains key addition/removal; it does not freeze existing values. Value writes follow the property's mutability through a contract view and the normal `mut`, `inmut`, and `inmut::strict` alias rules. Strict references still prevent reachable mutation.

Objects have reference identity and no inferred structural equality or copy semantics. Deep `clone()` remains explicit and requires cloneable reachable values. Tuples and enums retain their value semantics.

### JSON

JSON object nodes materialize as ordinary `Object` values, arrays as `List<Dynamic>`, and JSON scalars as their ordinary Zirk values. Parse returns `Dynamic`, not a parallel public JSON tree. Exact JSON number tokens must be preserved until a checked conversion is requested. Stringification traverses ordinary values and reports cycles or unsupported values through catchable errors without returning partial output. Typed codecs validate contracts without retained decorators or runtime reflection.

### Breaking migration

`interface` declarations migrate to `contract`. `record` declarations and record-only inline layouts are removed. Data shapes migrate to a contract plus an object literal; nominal identity and behavior migrate to a class; pure value aggregates migrate to tuples or enums. Code that used `Object` as an any-value type migrates to a generic type parameter or `Dynamic`, depending on whether the input should retain its static type.

## Consequences

- The AST, type checker, IR, interpreter, native runtime, GC tracing, JSON APIs, formatter, LSP and examples must adopt the same representation and diagnostics.
- Dynamic property access and contract validation may allocate and perform string lookup; declared class fields retain their fixed layout.
- Existing aliases observe key sealing, so conformance is an observable state transition on the shared object.
- Runtime tags and managed-reference metadata must remain visible to both collectors and both execution pipelines.
- Source compatibility is intentionally broken and requires migration diagnostics and migrated fixtures.

## Implementation evidence

Implementation evidence is recorded in the archived change's tasks and verification artifacts. The feature-status catalog is the current source for implementation status.
