## Context

See proposal.md for motivation and scope. The current parser stores a flat list of union alternatives; the checker interns a normalized union, but emits NOT_LOWERED for real unions and rejects member access on every union. Boolean and enum matches have limited exhaustiveness. Type aliases are transparent and non-generic. Contracts provide named structural shapes and object literals can be checked against one contract. There is no intersection type, singleton type, general constant evaluator, structural object type expression, or flow-sensitive union narrowing.

The compiler is a Rust frontend with portable typed IR, an interpreter, an LLVM 20 backend, and a tracing managed-memory runtime. The design must keep those execution paths semantically identical, retain managed references in tagged payloads, preserve generic/package identity, and avoid exponential type expansion. Public syntax and compiler status are mirrored to the sibling website after the source change is committed.

## Goals / Non-Goals

**Goals:**

- Provide a native static type algebra for n-ary unions and intersections, singleton values from every deterministic compile-time-constructible value type, structural shapes, generic type computation, and sound control-flow narrowing.
- Cover the practical type composition and type-level programming used in TypeScript while preserving Zirk's declared numeric widths, value/reference distinctions, no-implicit-loss conversions, explicit Dynamic values, and native safety rules.
- Compile heterogeneous unions through the interpreter and supported native targets with correct tagging, ABI, GC tracing, and no per-value heap allocation in the ordinary inline case.
- Bound compile-time evaluation and type solving; make diagnostic results deterministic and useful in the CLI and editor.
- Document observable semantics, unsupported edge cases, performance limits, and the implementation evidence accurately.

**Non-Goals:**

- Reproduce JavaScript runtime semantics, implicit any, undefined, declaration merging, ambient JavaScript declarations, or TypeScript's unsound assignment exceptions.
- Add general runtime reflection, member enumeration, string-based dynamic dispatch, or a second dynamic cast system.
- Change class identity, contract dispatch, enum construction, ownership, permission, or Object identity rules.
- Permit arbitrary user code, I/O, clock reads, randomness, environment access, or unbounded recursion during compile-time evaluation.
- Promise unbounded type computation. Every recursive evaluator and expansion has explicit resource limits.

## Decisions

### D1. Separate parsed type expressions from canonical semantic types

The AST will preserve source structure and spans in a recursive TypeExpr tree: named and generic references, literals, constant expressions, structural object shapes, unions, intersections, and parenthesized groups. The checker will resolve this into an interned canonical type DAG. Aliases retain source names in diagnostic metadata while canonical identity follows the resolved target.

Union and intersection nodes are n-ary, flattened, sorted by a stable semantic key, deduplicated, and hash-consed. Normalization applies proven subtype absorption and literal widening. It will not eagerly distribute intersections over unions: relation checking and conditional types operate on the DAG and memoize pairwise results. Flat N-member forms therefore remain linear to parse and intern; nested transformations are protected by deterministic work, depth, and expansion budgets.

Type formatting uses spaces around `|` and `&`, with `&` binding more tightly and parentheses retained whenever they change precedence. Generic applications use `Name<A, B>`; `const(expression)` has no interior padding beyond the expression formatter. Singleton spellings retain their source value (including the distinction between string, character, duration, temporal, and null forms) rather than widening to a base type. Structural shapes use braces, one property per line when multiline, `name?: Type` for optionality, `inmut` or `mut` before the property name for mutability, and commas between properties. Callable and nested types use the ordinary function-type and delimiter spacing rules. Conditional types format as `Check extends Constraint ? Yes : No`, mapped types as `{ [Key in Keys as Remap] modifiers: Value }`, and template-literal types retain backticks with `${Type}` holes and no inserted whitespace inside literal segments. Formatting must preserve grouping and modifiers through parse-format-parse, and a second formatting pass must be idempotent.

The initial compile-time budgets are normative: constant evaluation is limited to 100,000 fuel steps, 32 call frames, aggregate nesting depth 64, and 1 MiB per canonical value; subtype and assignability solving is limited to 100,000 memoized pair visits per immutable analysis snapshot and recursion depth 256; each union or intersection normalization is limited to 256 visited source/flattened members, bounding pairwise normalization work to at most 65,536 candidate comparisons; template expansion is limited to 10,000 generated union members and 1 MiB of generated type-source bytes. The parser also rejects recursive nesting beyond 48 levels with `NESTING_TOO_DEEP` before recursive-descent frames can exhaust the host stack. The constant-call limit is intentionally below the general type-relation depth because recursive evaluator frames are large in debug builds; it is validated on Rust's default test-thread stack and must not depend on a larger process-main stack. Computation-budget exhaustion uses the stable diagnostic code `E0482`, includes the operation category and configured bound, and never returns a partial result or silently widens. Changing these limits requires stress evidence and a spec update.

The intersection operator binds more tightly than the union operator; parenthesized expressions are supported in aliases, function signatures, constraints, fields, and generic arguments. Nullable suffix syntax remains equivalent to union with Null. Generic constraints written as T from A & B resolve through the same intersection semantics. Anonymous object shapes use contract-like declarations, including property modifiers; generic mapped shapes use the form { [K in keyof T]?: T[K] } with key remapping through as. Conditional types use T extends U ? X : Y, and infer variables are scoped to the true branch. Type-level string templates use a dedicated backtick-delimited type form, distinct from runtime String interpolation.

**Alternatives considered:** retaining the current flat TypeRef union vector cannot represent nested mixed expressions. Expanding every expression into disjunctive normal form makes cases such as (A1 | ... | An) & (B1 | ... | Bm) grow multiplicatively. A binary tree without canonical interning makes equivalent type identity and repeated relation checks needlessly expensive.

### D2. Use a sound, memoized assignability relation

The relation engine defines subtype and assignability rules for aliases, literals, primitive families, structural shapes, nominal classes/enums, generic instantiations, nullable types, unions, intersections, and Dynamic. It checks all source union alternatives and every target intersection constraint. Intersections prove only capabilities present in their checked constituents; they do not manufacture unrelated nominal identity.

Mutable generic containers remain invariant unless a declaration proves a safe declared variance. Function input/result positions follow declared/intrinsic variance. Structural writes must be safe across every possible runtime alternative. Dynamic stays a runtime-tagged, explicitly narrowed value. The existing internal error-recovery Unknown remains a compiler sentinel, not a user-facing unchecked top type.

Each relation query is cached by canonical source/target IDs plus visibility and generic-substitution context. Cache lifetime is one immutable analysis snapshot, so edits cannot reuse facts from an older document generation.

**Alternatives considered:** copying TypeScript's bivariant exceptions and implicit any would weaken Zirk's existing safety model. Pure structural equality for classes would erase their documented nominal identity.

### D3. Make singleton types value-based and extensible

Primitive singleton syntax remains concise: Boolean, numeric, String, Char, Duration, and Null values can appear directly in type expressions. General values use const(expression), for example:

    type Holiday = const(Date(2026, 12, 25)) | const(Date(2027, 1, 1));
    type Wait = 250ms | 1s;
    type State = { kind: "ready"; completed_at: const(DateTime(Date(2026, 10, 26), Time(9, 0))) };

const(expression) is evaluated by the bounded evaluator defined in type-constant-evaluation. A type can participate when it supplies deterministic construction, canonical value identity, and a checked value-semantic contract. Built-ins include all numeric families, String, Char, Duration, Date, Time, DateTime, deterministic temporal values, enums, tuples, and supported immutable aggregates. User-defined immutable value types can participate through checked constant constructors and value equality. The design is not a fixed list of primitive literal kinds.

Compile-time constants must be deterministic and recursively immutable. Mutable reference identity does not become structural equality. Clock, environment, random, I/O, and platform-dependent operations are unavailable. Zoned values whose meaning depends on time-zone data are constant only when the relevant rules/version are fixed as part of the value.

Canonical identity is the pair of the resolved value type and that type's canonical value. Integer width/signedness is retained and range-checked. Decimal uses the runtime's normalized coefficient/scale representation, so `1.0` and `1.00` are equal without binary conversion. String and Char compare their exact UTF-8 value; normalization is explicit and does not alter singleton identity implicitly. Duration uses exact nanoseconds. Date, Time, and DateTime use their checked civil scalar representations. Enum identity includes the declared enum and variant plus recursively canonical payloads; tuples preserve ordered element types and values; user-defined values include the declared value type and recursively immutable fields, never object identity.

Binary Float identity follows Zirk's existing ordered floating equality (`OEQ`) used by `==` and `is`: positive and negative zero are one singleton, infinities retain their sign and width, and NaN is rejected as a constant because it is not equal to itself and safe Zirk operations raise on NaN results. Float width remains part of the value type. Zone-aware temporal values use their owner-defined equality and include the pinned time-zone rule-data identity; `TimeZone` uses canonical IANA identity (fixed offsets remain distinct), `Instant` uses timeline nanoseconds, `ZonedDateTime` uses instant plus canonical zone and rule-data version, and `Period` compares its calendar components without conversion to Duration. Every constant type contributes canonicalization and match semantics through a closed compiler capability, not arbitrary runtime callbacks.

**Alternatives considered:** hard-coding Boolean/String/numeric literals contradicts the Date/Time requirement and blocks user value types. Evaluating unrestricted functions at compile time introduces effects and nontermination. Treating mutable object addresses as singleton values is unstable across aliases and GC.

### D4. Add structural object type expressions while reusing contracts

Named contract declarations remain the primary reusable structural declaration and retain current visibility, sealing, identity, and dispatch rules. Anonymous object type expressions provide composable property shapes with required/optional and read-only/writable modifiers; read-only fields use Zirk's inmut::strict property modifier. Width subtyping is supported for already-typed values. Fresh literals receive excess-key diagnostics to catch mistakes while unknown spreads contribute only statically known properties. An explicit exact-shape form can be added only if a real use case requires stronger closure than contracts already provide.

Object literals checked against a union are evaluated against every alternative. The checker propagates expected property types into nested objects and lambdas, filters candidates by required fields and singleton discriminants, accepts exactly one viable candidate, and diagnoses zero or multiple candidates. It never chooses by declaration order. A successful selection injects the value into the selected union member representation.

Discriminant properties used for stable flow facts must be read-only or otherwise proven unaliased and unchanged. Writes, mutable aliases, and unknown calls invalidate affected path facts.

**Alternatives considered:** enum-only unions are straightforward but do not support the requested automatic structural-object selection. Inline shapes alone would duplicate contract semantics and weaken existing reusable declaration rules.

### D5. Use one bounded constant evaluator

The evaluator accepts literals, deterministic operators, immutable aggregate construction, and explicitly constant functions whose effects are statically restricted. const(expression) is its source-level entry point for value singleton types. The evaluator returns canonical typed values or a typed compile-time diagnostic. It enforces instruction fuel, recursion depth, aggregate depth, and value-size budgets. Evaluation does not execute native code or use platform services.

Built-in temporal constructors and duration constants are evaluated from their existing exact APIs. Parsing that returns a Result can participate only when the input is constant and the result is successful; invalid construction becomes a compile-time diagnostic at the expression. User-defined constant functions are checked for allowed effects before evaluation. Their functions and value constructors are versioned with their package/compiler semantic metadata.

**Alternatives considered:** a second evaluator in the type checker risks disagreement with decorators and the interpreter. Reusing arbitrary decorator expansion would expose too many syntax operations and effects for constant identity.

### D6. Perform flow-sensitive narrowing over stable paths

The semantic flow state records a declared type and an optional refined type for locals and supported property paths. Conditions refine true and false edges for literal equality, Boolean tests, type tests, property presence, negation, conjunction/disjunction, null tests, and verified user predicates. Match arms subtract their covered members/literals and receive the remainder's exact type. Joins union the facts from live predecessors. Loops iterate the transfer function to a fixed point under a deterministic budget.

Refinements are invalidated by direct assignments, writes through aliases, escape to calls that may mutate a path, and other operations whose effects make a fact stale. Read-only discriminants retain facts across unrelated calls. User predicates state a subtype relationship; the checker verifies any proof form the language permits and otherwise requires an explicitly trusted boundary that cannot be forged by ordinary safe code.

**Alternatives considered:** type-only match narrowing misses ordinary if control flow. Narrowing by syntax without mutation invalidation becomes unsound as soon as a discriminant can change. Runtime type checks on structural contracts are not a replacement for value-flow reasoning.

### D7. Represent general runtime unions as inline tagged sums

Every union value that cannot be recovered from one unambiguous existing runtime representation uses a compiler-generated tag plus an aligned payload sized for the largest alternative. Injection writes one valid tag and its typed payload; projection checks the tag; match dispatch reads the tag once. This representation is an internal value, not a public reflection object. It is passed and returned through target-aware internal ABI lowering. Direct C ABI exposure remains rejected unless an explicit interoperable wrapper is provided.

The normal representation is inline and performs no allocation for union injection. Canonical unions whose member identity is redundant may use a representation-elision optimization only after the optimizer proves the same narrowing and GC behavior. Large aggregate unions may use a boxed fallback only when target layout evidence shows inline storage is unsafe or impractical; the fallback is measured and documented.

IR carries a verified union layout and stable alternative mapping. Package IR records the canonical type fingerprint and schema version so separate compiler units cannot disagree about a tag. Native codegen selects each target's correct aggregate calling convention. The interpreter implements the same abstract tag/payload operations.

**Alternatives considered:** lowering all unions as Dynamic would box and conflate distinct language semantics. Restricting unions to equal machine representations would exclude common cases such as String | Int32. Eagerly allocating a wrapper object for every value would add avoidable heap pressure.

### D8. Make union payload tracing tag-aware

The collector receives a verified layout descriptor that dispatches on the active union tag and traces only managed fields for that alternative, recursively. Root slots, task outcomes, closures, tuples, collections, and nested unions use the same descriptor. Invalid tags fail before payload projection or pointer tracing. Copy/move operations preserve tag and payload atomically with respect to safepoints.

Intersections remain static views over the original value. They do not duplicate objects or add a runtime header.

**Alternatives considered:** scanning the maximum payload as if all alternatives were active can mistake scalar bits for pointers and retain or inspect invalid memory. A side table keyed by object address creates lifetime and relocation hazards.

#### Union layout and package-IR decisions

- **Logical layout identity:** normalize alternatives first, then encode the ordered canonical member type encodings under the domain tag `zirk.union.layout.v1`. The package fingerprint is SHA-256 over that encoding and the package-IR schema major version. It excludes alias spelling, source order, compiler-local intern IDs, target triple, and physical byte offsets. Recursive members use their own canonical type encodings and a recursion marker tied to the canonical declaration identity. A package consumer rejects a missing schema version, unknown layout version, or fingerprint mismatch before linking.
- **In-memory tag and payload:** the initial portable layout uses a 32-bit unsigned discriminant and one aligned payload area sized and aligned for the largest alternative. The payload starts at `align_up(4, max_payload_alignment)` and total size is rounded up to `max(4, max_payload_alignment)`. Per-alternative field offsets and managed-reference paths are part of verified IR metadata, not reconstructed from source order by codegen.
- **Internal ABI:** Zirk-to-Zirk calls pass and return the logical union value through target-specific LLVM aggregate ABI lowering. When the target ABI classifies an aggregate as indirect, codegen uses caller-provided aligned storage and hidden by-reference parameters/results; this does not add a managed allocation. Union values remain forbidden at a raw C ABI boundary unless an explicit wrapper defines that ABI.
- **Large payload policy:** the first release has no boxed fallback. Keep payloads inline and issue a stable source diagnostic when the aligned payload exceeds 4096 bytes. Target ABI indirect passing alone does not select rejection: it changes only how the inline value crosses a call boundary. A future managed-box fallback requires task 10.9 target-layout and allocation measurements before it can replace this bounded rule; changing storage strategy does not change source semantics or logical package type identity.
- **GC descriptor:** each union layout carries one reference-path list per valid tag. Collection reads and validates the tag, then visits only the selected alternative's paths recursively. Scalar alternatives have an empty path list even when their bits could resemble an address. Nested unions dispatch recursively. A tag outside the declared alternatives fails closed before any payload pointer is read. Injection initializes the payload and publishes the matching tag last; safepoints cannot observe a mixed tag/payload pair.

These decisions replace the prototype's flattened enum fields as the target representation. The prototype remains useful for proving source injection, call transport, and basic GC reachability, but it is not evidence that this final layout, package fingerprint, large-box path, or tag-aware descriptor has been implemented.

### D9. Add TypeScript-class type computation with Zirk rules

Generic aliases support parameters, defaults, and constraints. Type operators include keyof, indexed access, conditional types with infer, mapped types with key remapping/modifier control, finite template-literal types, and a standard utility-alias prelude. The prelude includes property transforms, Pick/Omit/Record, union filtering, null removal, callable parameter/result extraction, task-result unwrapping where applicable, and intrinsic string transforms. Conditional distribution is defined for a naked type parameter and can be suppressed by grouping. Template and mapped expansions are memoized and budgeted.

Object shapes, tuples, callable types, and generic instantiations compose with the algebra. The type system uses Zirk's explicit visibility and variance rules. Optional property reads follow Zirk's null/presence model, not JavaScript's undefined. No implicit any is introduced. Callable unions/intersections are checked from all possible call signatures; an ambiguous result is a diagnostic rather than runtime overload dispatch.

The target is equivalent type-authoring power for common structural modeling and generic transformations, not source compatibility with every TypeScript/JavaScript feature.

**Alternatives considered:** supporting only unions/intersections leaves generic APIs dependent on manual duplicate aliases. Implementing unrestricted recursive conditional evaluation before budgets risks exponential compilation and editor freezes.

### D10. Optimize with canonical DAGs and bounded analysis

Type interning uses structural hashes derived from stable canonical encodings. Relation caches, conditional evaluation caches, and template expansion caches are scoped to a compilation snapshot. The checker does not materialize distribution products unless a result requires them. Every user-controlled recursion or expansion has independent counters and source-located diagnostics.

The implementation establishes performance baselines for flat N-ary unions, nested intersections, structural candidate selection, recursive aliases, conditional types, template unions, and LSP reanalysis. Compiler memory and latency are measured on representative projects and generated stress fixtures. Budget defaults are selected from those measurements and reported consistently across CLI, interpreter, native builds, and LSP.

### D11. Preserve source and editor observability

Type displays show aliases when they help comprehension and canonical details when needed to distinguish a singleton or composite. Diagnostics name alternatives, conflicting properties, failed constraints, constant-evaluation effects, and exhausted budgets. LSP hover, completion, inlay hints, go-to-definition, references, and semantic queries operate on the current analysis snapshot and cannot return stale narrowed types.

## Risks / Trade-offs

- [Tagged unions add payload and ABI complexity] → Prototype a scalar/string union and a reference-bearing aggregate before broad semantic work; reuse enum layout primitives where sound; require IR verification and target-specific ABI evidence.
- [GC descriptors could trace an invalid alternative] → Make tag/layout pairs verifier-owned, test scalar bit patterns that resemble pointers, and reject invalid tags before trace or projection.
- [Type distribution can grow exponentially] → Keep canonical DAGs, avoid eager distribution, memoize relation/evaluation pairs, and enforce deterministic budgets with diagnostics.
- [Flow narrowing can become stale after mutation] → Track stable paths, require stable discriminants for durable facts, conservatively invalidate on writes and escaping calls, and test alias-heavy cases.
- [Universal singleton types require equality semantics for many values] → Gate each type through deterministic construction and stable value identity; make temporal rules explicit; add each value family with canonical round-trip conformance cases.
- [Constant evaluation can execute expensive or effectful code] → Check effect restrictions before evaluation and enforce independent fuel, recursion, and value-size limits.
- [TypeScript terminology can imply behavior Zirk does not intend] → Document semantic differences: no implicit any/undefined, explicit Dynamic, Zirk numeric families, nominal class identity, and checked variance.
- [Editor type queries become expensive on every keystroke] → Reuse immutable canonical type IDs per snapshot, cancel superseded work, cap computation, and benchmark incremental edits separately from clean compilation.
- [Portable package ABI changes] → Version the IR/type schema, include canonical type fingerprints, reject incompatible cached packages, and avoid exposing native union layout to C without wrappers.

## Migration Plan

1. Land the type semantics and constant-evaluation contract before enabling executable union syntax.
2. Add canonical algebra and diagnostics while preserving an explicit NOT_LOWERED failure until IR/runtime support is ready.
3. Implement and verify portable IR, interpreter behavior, native layout, and GC tracing for representative payload classes.
4. Enable source features in dependency order: singleton types, union execution, contextual structures, flow narrowing, intersections, and advanced generic operators.
5. Update examples, limitations, feature status, roadmap, and compiler compatibility metadata from verified evidence.
6. Commit the source repository, then synchronize the companion website from that exact clean commit and review site-owned status claims.

Rollback before release consists of reverting the compiler change and invalidating packages built with its IR schema. After a compiler release, incompatible cached packages SHALL be rejected with a rebuild diagnostic; source-level aliases and existing valid source remain portable.

## Open Questions

None. The scope, syntax entry points, soundness boundaries, compile-time evaluation model, runtime union strategy, and performance constraints are specified for implementation.
