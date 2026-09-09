## Context

Zirk already lexes `...` and parses it in function and callable-type parameter declarations. The checker groups ordinary positional arguments into a variadic parameter, and the language documents those values as an ordered read-only iterable. There is no AST representation or parser path for `...value` in calls, collection literals, or destructuring; rest destructuring is explicitly deferred. This design completes the family without confusing explicit spread with the range-specific implicit expansion proposed separately.

## Goals / Non-Goals

**Goals:**

- Add explicit spread arguments and collection elements using `...expression`.
- Keep rest parameters as the receiving side of variadic calls and make their sequence semantics explicit.
- Add rest destructuring for ordered collections with a fixed prefix and remainder binding.
- Add record/object spread and rest destructuring for named fields.
- Source all spread values through `Iterable<T>` and preserve ordinary iteration copy/transfer rules.
- Share one expansion model with range expansion while keeping `Array(0..2)` (implicit range expansion) distinct from `Array(...range)` (explicit spread).
- Evaluate each spread expression once, expand in order, and report checked capacity/arity failures.

**Non-Goals:**

- A general macro or arbitrary iterable unpacking mechanism outside calls, collection literals, and approved destructuring patterns.
- Treating records/classes as generic iterables; object spread is field-based and collection spread is iterable-based.
- Replacing variadic parameters or changing named/default parameter rules unrelated to expansion.
- Implicit spreading of every iterable; `...` remains required for explicit expansion except the separately specified range constructor convenience.
- Nested `[[range]]` behavior or a new collection type.

## Decisions

### Explicit spread AST node

Add an explicit `SpreadExpr`/`SpreadElement` shape to arguments and collection elements rather than encoding spread as a unary operator. This preserves source intent, enables precise diagnostics, and prevents accidental spread in ordinary arithmetic or indexing expressions.

Alternative: desugar during parsing into synthetic calls/elements. Rejected because it loses spans and makes type/ownership diagnostics point at generated structure.

### Iterable-based source contract

The checker accepts spread only when the expression implements `Iterable<T>`. Expansion consumes the iterable in its documented order and uses the element type `T` for argument matching or collection element unification. Arrays, lists, strings, and ranges participate through their existing iterable contracts.

Alternative: special-case arrays/lists/ranges. Rejected because it duplicates behavior and would make user-defined iterable types second-class.

### Call argument matching

Expand spread arguments before final positional/variadic slot assignment, while retaining lazy evaluation of each expression until its source position is reached. A spread into a fixed-arity parameter list is valid only when the compiler can prove the expanded count matches; otherwise it requires a variadic target or produces a controlled compile-time diagnostic. Named arguments cannot be produced by spread and cannot follow a spread that leaves positional ordering ambiguous.

### Collection construction

Array/list literals and `Array(...)`/`List(...)` constructor calls use one ordered expansion pass. Scalar elements and spread elements are evaluated left to right; each iterable is consumed once. Arrays allocate exact expanded length, lists append expanded values, and checked capacity arithmetic prevents overflow. The range proposal's implicit `Array(0..2)` convenience remains separate from explicit `Array(...(0..2))`.

### Rest destructuring

Rest patterns are allowed only once, must be last, and bind an ordered collection of the remaining element type. A pattern such as `[first, ...remaining]` consumes an array/list (and any future explicitly approved ordered iterable) without mutating the source. Empty remainders are valid. Destructuring follows projection-copy rules, so reference-backed elements are independent values unless an explicit view/transfer API is used.

### Object/record spread and rest

Object spread is a separate AST form from iterable spread. In a record-typed expression context, `{ ...source, field: value }` copies the source record's named fields and applies explicit fields from left to right; a later explicit field overrides an earlier spread field. The checker requires all resulting fields to satisfy the expected nominal record shape and rejects unknown, duplicate, or missing required fields.

Object rest destructuring uses `{ field, ...rest }` in a record pattern. The fixed fields are projected from the source, and `rest` receives a record-compatible remainder containing every source field not selected by the fixed pattern. The source is not mutated and projected reference-backed fields follow normal deep-copy/Clone rules. Because braces also delimit blocks, object spread/rest syntax is accepted only in expression or destructuring contexts where a record/object type is known.

Alternative: model records as key/value iterables. Rejected because it loses nominal field names, weakens compile-time shape checking, and makes object spread order-dependent in a way Zirk records are not.

### Variadic parameter representation

Keep the existing ordered read-only `Iterable<T>` representation for `...values: T`. Spread calls feed the same representation, and a variadic parameter may itself be spread into another variadic call or collection. The checker must prevent a mutable alias from escaping through expansion.

## Risks / Trade-offs

- **[Risk]** Runtime expansion can consume an unbounded or expensive iterable. → **Mitigation:** require finite collection/range sources for eager array/list construction and document eager consumption; preserve lazy behavior only for ordinary iterable iteration.
- **[Risk]** Spread into fixed-arity calls makes arity depend on runtime length. → **Mitigation:** reject unknown counts unless the target has a variadic tail; allow statically sized arrays where count is known.
- **[Risk]** Multiple spreads can obscure evaluation order. → **Mitigation:** specify and test strict left-to-right evaluation with one consumption per expression.
- **[Risk]** Reference-backed elements could accidentally alias. → **Mitigation:** reuse iteration projection-copy semantics and require explicit transfer/view APIs for sharing.
- **[Risk]** `...` is already used in declarations and `...` is a lexer token for `DotDotDot`. → **Mitigation:** context-sensitive parser branches with dedicated AST nodes and diagnostics.
- **[Risk]** Existing docs claim variadic support is complete while spread/rest destructuring is absent. → **Mitigation:** split feature-status wording and update companion website content after verification.

## Migration Plan

1. Add AST/parser nodes and diagnostics while retaining existing variadic declarations.
2. Implement checker expansion typing and argument/collection rules.
3. Implement IR lowering and runtime iterator-driven expansion with checked capacity.
4. Add rest destructuring and migrate examples/fixtures.
5. Update normative docs, feature status, roadmap, and `../zirk-lang-site` from a committed source revision.

Rollback is a source-level revert; no persistent data migration is required.

## Open Questions (answered)

- Should spread into a fixed-arity call be permitted for statically sized arrays only, or for any compile-time-known finite iterable?
  - **Decision:** Any compile-time-known finite iterable. When the target has a fixed arity `n`, the expanded count must not exceed `n`; if it is smaller, the first expanded slots are occupied and the remaining fixed slots are filled by subsequent positional arguments or defaults.
- Which ordered iterable types beyond `Array`, `List`, `String`, and `Range` should be accepted by rest destructuring in the first release?
  - **Decision:** Object/record rest destructuring is also supported via field-based `{ ...rest }` in record patterns.
- Should `...` be allowed in named-argument positions through a mapping/object form, or remain strictly positional?
  - **Decision:** An inline record/object argument constructed at the call site may use `{ ...rest }` as the value of a named argument, e.g. `foo(profile: { ...base, active: false })`.

## Status

Implemented end-to-end. `examples/spread_rest_examples.zrk` compiles and runs;
`cargo test -p zirk-parser`, `cargo test -p zirk-sema`, `cargo test -p zirk-ir`,
and `cargo test -p zirk-cli` pass with the required LLVM 20.1 toolchain. The
OpenSpec main specs, `ZIRK_LANGUAGE_SPEC.md`, `CORE_LANGUAGE_SEMANTICS.md`, and
`ZIRK_FEATURE_STATUS.md` have been updated to remove stale unsupported claims.
