## Why

Zirk parses and normalizes a limited union type, but a real union still cannot pass lowering; intersections, singleton literal types, general union narrowing, and contextual selection between structural alternatives are unavailable. This prevents the language from expressing precise APIs and discriminated data models that developers expect from a modern static type system. Zirk needs one native, sound, and efficient type algebra that spans compile-time type computation, flow analysis, and executable representation rather than a parser-only approximation.

## What Changes

- Complete n-ary union and intersection types, grouped/nested type expressions, aliases, and generic type aliases throughout parsing, checking, IR, interpretation, native code generation, package boundaries, and diagnostics.
- Add singleton types for every supported compile-time constant value, including Boolean, numeric, String, Char, Duration, Date, Time, DateTime, enum, tuple, and user-defined immutable value types. Define a bounded, deterministic compile-time value evaluator and canonical equality for those types.
- Add structural object type expressions and contextual checking of object literals against union alternatives, including automatic selection by literal discriminants and explicit ambiguity diagnostics.
- Add flow-sensitive narrowing, common-member access, discriminated unions, exhaustive matching, stable custom type predicates, and sound invalidation after mutation or escape.
- Add TypeScript-class type composition and inference for generic aliases, constraints, `keyof`, indexed access, conditional and mapped types, finite template-literal types, recursive type aliases, and standard utility types, while retaining Zirk's checked `Dynamic` model and numeric-width semantics.
- Give heterogeneous runtime unions a native tagged representation with correct ABI, garbage-collector tracing, copying, and pattern dispatch; erase intersections where their constituents share one value and preserve zero-cost behavior where layout permits.
- Add type-complexity budgets, canonical type interning, memoized relations, compiler benchmarks, native/interpreter parity checks, and editor support for the resulting inferred types and diagnostics.
- Define normative precedence and parenthesization (`&` binds more tightly than `|`); make previously parsed unions executable. Preserve currently valid source behavior and give targeted migration diagnostics wherever new grammar or inference exposes an ambiguity.

## Capabilities

### New Capabilities

- `type-constant-evaluation`: bounded, pure compile-time evaluation and canonical value identity used by singleton types.

### Modified Capabilities

- `zirk-grammar`: recursive type-expression grammar, literal type syntax, grouping, generic type arguments, and type-level operators.
- `zirk-data-types`: complete unions/intersections, aliases, and singleton value types.
- `zirk-type-system`: canonical algebra, subtyping, assignability, inference, narrowing, and sound type-level computation.
- `zirk-contracts`: structural object type composition and contextual union-member selection.
- `zirk-generics`: generic aliases, type operators, conditional inference, variance, recursion, and complexity controls.
- `zirk-pattern-matching`: narrowing and exhaustiveness over unions, literals, structural discriminants, and intersections.
- `zirk-ir-lowering`: native union injection/projection and static intersection views.
- `zirk-native-codegen`: target-correct tagged-union layout, ABI, and dispatch.
- `zirk-memory-safety`: tag-aware tracing and safe copying of union payloads containing managed references.
- `zirk-lsp`: type-expression diagnostics, inferred singleton/composite types, and narrowing-aware editor results.
- `zirk-ide-query`: stable semantic type information for aliases, literal values, and composite types.

## Impact

This changes the parser and AST, semantic type representation and relation engine, generic/type-alias handling, constant evaluation, pattern checker, portable IR, interpreter, LLVM backend, garbage-collector descriptors, diagnostics, and LSP/IDE queries. It affects public syntax, normative type semantics, compiler implementation status, limitations, roadmap, handbook examples, and the generated type reference. The companion `../zirk-lang-site` repository must be synchronized after the source documentation is committed; its status catalog must be reviewed before publishing completion claims. No external runtime dependency is assumed; any cryptographic or compiler dependency introduced by implementation requires a separate review.
