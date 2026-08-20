# Type Checker and Flow Analysis

After names resolve, the semantic frontend validates what values and effects
each expression can produce and how facts change along control-flow paths. It
generates typed information for portable IR; it does not choose native layout.

> **Implementation status:** the current checker covers the implemented scalar
> subset, inference, nullability basics, assignments, calls/named arguments,
> returns, definite initialization, loops, match foundations, lambdas/captures,
> enums, classes, constructors, visibility, inheritance, method calls, generics
> with `from` constraints, interfaces/traits, `Result<T,E>`'s seven structural
> methods, `throw`/`try`/`catch`/`finally` with "capture or declare" effect
> analysis, and `Resource<E>`/`match with` closing on a single acquisition.
> `Result`'s generic combinators (`map`, `and_then`, …), catch variant patterns,
> grouped resource acquisition, `Fn` type annotations, strict immutability,
> concurrency, and later standard types remain staged work.

## Inputs and output

The checker consumes resolved semantic AST plus source/module information. Its
checked result records expression/binding types, callable signatures, class and
enum metadata, selected members/constructors, lambda captures, and facts needed
by lowering.

No inferred type becomes public API accidentally: exported declarations follow
the language's annotation/inference rules and `public.api` records their checked
contract.

## Inference and compatibility

Local inference begins from initializers and contextual expectations. It is not
dynamic typing:

```zirk
mut count = 1;       // inferred Int
count = 2;           // valid
count = "two";       // type mismatch
```

Numeric promotion, casts, contextual conversion, nullable widening, class
subtyping, generics, unions, callable variance, and protocol conformance use the
rules owned by their type-system chapters. The checker never inserts an
undocumented implicit conversion merely to make a call succeed.

Named arguments bind by declared name; shorthand `url:` means `url: url`.
Candidate selection checks names, arity, optional/default/variadic positions,
types, and ambiguity in a deterministic order.

## Path-sensitive flow

The checker builds or traverses a control-flow representation for conditions,
branches, loops, match arms, returns, throws, and termination. Facts are joined
at merge points.

```zirk
mut user: User? = load_user();

if user != null {
    stdout.println(user.name); // narrowed on this path
}
```

A narrowing is invalidated when mutation, aliasing, an unknown call/effect, or a
concurrent boundary can change the observed value. Reassigning the binding and
mutating a reference's internals follow their distinct reference rules.

## Definite initialization and returns

A binding or field must hold a value on every path before read. Constructors
must initialize required fields, including inherited state, according to the
object model. Branch assignment counts only when every continuing branch
establishes the fact.

Every reachable path of a non-`Void` callable returns or terminates. `Never`
paths do not require a value and do not create a false missing-return error.

## Nullability and exhaustiveness

`T` and `T?` remain distinct. Null checks, matching, and coalescing can narrow a
value only within the path justified by the condition. A nullable result cannot
silently become non-null because an earlier alias was checked.

Match checking computes the remaining domain for booleans, nullability, enums,
unions, records/payloads, results, and other accepted patterns. Unreachable or
non-exhaustive arms receive targeted diagnostics. Regex and open-domain patterns
require a catch-all where exhaustiveness cannot be proven.

## Functions, lambdas, and captures

Callables use `Fn(P...) => R` (`Function` is the long alias). The checker
validates parameters, return and Result/error contracts, generic constraints,
effects, and variance.

Closure capture mode is selected automatically under the established value/
reference semantics. Capturing a projection obtains its new value; capturing a
whole reference can share identity. Mutation, strict immutability, escaping
lifetime, task/thread transfer, and data-race rules are checked at the capture
and use sites.

## Resources, effects, and permissions

The target checker tracks:

- resource ownership, dependent references, transfer and scope escape;
- ordinary error/Result flow and throwable effects;
- permission effects inferred through direct and higher-order calls;
- task/thread scope, cancellation edges and transfer/share safety;
- unsafe transaction, rollback eligibility and irreversible `commit` effects.

Effects remain callable/compiler metadata rather than extra syntax in `Fn`.
Application grants remain in `init.zrk`; successful type/effect checking never
grants authority.

## Decorator rechecking

Typed targets feed decorator `Inspect`. `Augment` and `Wrap` output is resolved,
typed, flow/effect checked, and safety validated as ordinary syntax. A wrapper
must preserve the callable contract or expose its added error/permission effect.
Generated diagnostics retain both generated and source application origins.

## Recovery type

After reporting an error, the current checker uses an internal unknown/error
type compatible enough to suppress derivative mismatches. It is not the public
`Any`, `Object`, a union, or an inference result.

The recovery type cannot justify member availability, conformance, permission,
safety, overload choice, or code generation. Any frontend error prevents
lowering; the IR verifier rejects recovery artifacts if a compiler bug leaks
one through.

## Diagnostics and determinism

Type diagnostics point to the operation and relevant declarations/candidates,
show actual and expected checked types, and explain invalidated flow facts where
useful. Independent errors continue up to the global diagnostic bound. Ordering
does not depend on hash iteration or thread scheduling.

---

**Previous:** [← Name Resolution](03-name-resolution.md) · **Next:** [Decorator Expansion →](04a-decorator-expansion.md)
