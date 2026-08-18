# Decorator Expansion in the Frontend

Decorators are compile-time transformations over immutable validated syntax.
They are part of the frontend between initial semantic analysis and portable IR,
not runtime reflection and not unvalidated textual macros.

> **Implementation status:** decorator semantics and the public target model are
> specified, but the complete expansion engine is not implemented in the current
> compiler subset. This chapter describes the required compiler behavior. The
> language-facing API is owned by the [Metaprogramming](../06-metaprogramming/README.md)
> unit and [Decorator Semantics](../../DECORATOR_SEMANTICS.md).

## Expansion sequence

For each applicable source region the compiler performs:

```text
parse source
  → resolve and type original declarations
  → validate decorator graph/order
  → Inspect all applicable targets
  → commit compatible Augment operations
  → resolve/type/validate augmented declarations
  → compose Wrap operations
  → resolve/type/flow/effect/safety validation
  → repeat only for explicitly generated decorator applications
  → erase decorator machinery
```

`Inspect`, `Augment`, and `Wrap` are strict phases. Inspection cannot mutate a
target. Augmentation completes before executable wrapping so all wrappers see
the final checked signature and members.

## Target and transform dispatch

Only the five Zirk 1.x targets are accepted: class, attribute, function, method,
and parameter. A decorator selects its target block and transformation variant
through ordinary `match` syntax. Unsupported targets receive a targeted
diagnostic rather than falling into a generic parser error.

All compiler-provided values enter through explicit target, context, operation,
application, or local bindings. The engine does not invent implicit variables
inside decorator bodies.

## Ordering and dependencies

Applications evaluate top-to-bottom and compose outer-to-inner. `requires`,
`before`, and `after` validate visible source order; they never silently reorder
applications. A violation identifies both applications and the declared
constraint.

Self-dependencies and cycles are invalid. Cycle diagnostics show the complete
cycle. If the same decorator appears as its own `requires`, `before`, or `after`
dependency, the compiler rejects the graph before executing any application.

Contiguous uses of a repeatable decorator form one ordered group. Its explicit
`applications` payload contains each application and typed arguments. No
undeclared `application` singular binding appears by magic.

## Hygiene and generated API

Private generated declarations receive unforgeable compiler identities. Source
text cannot capture them by choosing the same spelling. Public generated names
are ordinary API and collisions fail with all source/generation origins.

Generated public declarations/descriptors enter `public.api`, documentation,
compatibility analysis and dependency invalidation. Private unused output
remains eligible for dead-code elimination.

## Revalidation

Every generated node passes the same parser/builder invariants, name resolution,
typing, flow/effect analysis, visibility, permission inference, resource safety,
concurrency safety and unsafe rules as handwritten code. The Syntax API cannot
fabricate an invalid node or mark an unchecked node as valid.

Wrappers must preserve parameters and successful return behavior unless the
decorator's public contract explicitly changes them. Added throwable, Result,
resource, permission or cancellation behavior becomes visible in checked
metadata and diagnostics.

## Build effects and permissions

Pure inspection/generation requires no external authority. Filesystem,
environment, network, process or other build effects require library `requires`,
application `permissions`, the `during: build` phase, and signed developer
approval. A decorator cannot edit `init.zrk`, approve itself, or inherit runtime
authority merely because the application has it.

Observed inputs enter the expansion fingerprint. Permission denial occurs before
the effect and does not leave a partially committed augmentation.

## Bounded rounds and cycles

Generated decorator applications may enter later expansion rounds only when
explicitly emitted. The compiler bounds rounds/nodes/work and detects structural
recursion using application and generated-origin identity. Exceeding a limit is
a deterministic diagnostic, not an infinite build or stack overflow.

## Incremental fingerprint

An expansion fingerprint includes the decorator implementation and package
integrity/version, typed arguments, typed target/public dependencies,
configuration, effective build grants, observed external inputs, and transitive
decorator dependencies. Unchanged fingerprints reuse validated output; a changed
input invalidates the affected expansion and semantic dependents only.

## Origin mapping and debugging

Each generated span records the source decorator application, generating
operation, and relevant target. An error primarily points to the user's useful
source location and adds an expansion trace. Debug information maps wrapped code
back to source while still allowing an opt-in expansion view for framework
authors.

## Erasure and runtime descriptors

Decorator functions, applications and arguments are erased after validated
expansion. Zirk has no automatic runtime retention or
`Reflection.decorators(...)`. Frameworks generate ordinary typed route,
serialization, injection or validation registries when runtime data is needed.

---

**Previous:** [← Type Checker](04-type-checker.md) · **Next:** [Diagnostic Recovery →](04b-diagnostic-recovery.md)
