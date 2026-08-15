# Language and Implementation Status

The Zirk documentation describes two related things that must not be confused: the language Zirk is specified to become, and the subset the current compiler can execute today.

This distinction is especially important early in a language's life. A handbook that documents only the current parser can make temporary omissions look permanent. A handbook that ignores implementation progress can make readers believe every example is runnable. Zirk documents both, labels the difference, and treats the normative specification as the stable contract.

## The normative target

The **normative target** is the public definition of Zirk 1.x. It includes source syntax, type rules, observable behavior, runtime contracts, standard-library modules, compiler requirements, project formats, and tooling expectations.

The master specification has the highest authority for overall scope and explicit exclusions. Specialized final specifications govern their areas:

- the language specification covers syntax and semantics;
- the runtime specification covers execution, memory, resources, concurrency, I/O, and shutdown;
- the standard-library specification covers public modules and operational contracts;
- the compiler specification covers the frontend, Syntax API, IR, LLVM backend, targets, diagnostics, and tools.

The handbook explains those contracts for readers. It does not replace them as the final arbiter of a semantic dispute.

## The current implementation

The **current implementation** is the compiler, runtime, library, and tools present in the repository at a particular revision. It advances in milestones and can lag behind the normative target.

A feature can therefore be:

- **Specified and implemented:** the final documents define it and the current toolchain supports it.
- **Specified, partially implemented:** the contract is stable, but only part of the syntax or behavior is available.
- **Specified, not yet implemented:** the handbook may teach the target behavior, but examples are not currently runnable.
- **Exploratory or historical:** an older document discussed it, but it is not part of the final Zirk 1.x contract.
- **Explicitly excluded:** the master specification says implementations must not add it freely as if it were an unspecified gap.

Status is never inferred merely because a phrase appears in an old design file.

## How chapters show status

When the distinction matters, a chapter contains a visible note near its first example:

> **Implementation status:** Normative in Zirk 1.x; parser support is not yet complete.

The wording should identify what is available and what is missing. “Planned” by itself is too vague: it could mean the language design is undecided, the design is final but unimplemented, or the feature is outside the current release.

Examples follow the normative syntax. If the compiler cannot run one yet, the note says so. The handbook does not substitute temporary syntax solely to make an example pass.

## Historical material and final decisions

[`docs/01_plantilla_zirk.md`](../../01_plantilla_zirk.md) is an unusually broad design inventory. It is valuable because it asks detailed questions about almost every part of the language, runtime, ecosystem, and tooling. It also predates the final consolidation and contains alternatives that Zirk 1.x later rejected.

For that reason, the template answers **what topics must be checked**, not **which answer is current**. The final specifications decide the latter.

Examples of explicitly excluded Zirk 1.x features include:

- WebAssembly and browser/DOM integration;
- public `@runtime`, `@target`, `@host`, or `@platform` directives;
- a standalone `worker` concurrency primitive;
- `async fn` and a public event loop;
- textual inline assembly;
- general `comptime {}` and general `defer`;
- multiple class inheritance and traditional function overloading;
- `?` as a `Result` propagation operator;
- ownership or reference counting as public language semantics.

Some of these ideas may be reconsidered in a future version through a new specification change. They are not hidden implementation opportunities in 1.x.

## Reading examples responsibly

A code block can answer different questions:

- **Normative example:** Is this valid according to the target language?
- **Executable example:** Does the current compiler accept and run it?
- **Invalid example:** Which rule should reject it, and what should the diagnostic explain?
- **Conceptual sketch:** Does this illustrate an architecture without claiming exact source syntax?

Handbook chapters identify invalid and conceptual blocks explicitly. Unmarked Zirk code is intended to be normative. As the implementation matures, executable examples should be tested automatically against the compiler revision used to publish the docs.

## Diagnostics are part of compatibility

Implementation completeness is not only whether valid code compiles. The Zirk completion contract requires each feature to have grammar, type rules, observable semantics, compile-time diagnostics, runtime errors, valid and invalid examples, interaction analysis, and conformance tests.

For example, implementing nullable types is incomplete if the compiler parses `String?` but silently permits an unsafe dereference. Implementing cross-compilation is incomplete if an incompatible native dependency fails only at the linker without explaining which package blocked the target.

The compiler specification requires diagnostics to carry a severity, stable code, source location, cause, and actionable help when a clear repair exists. Handbook invalid examples explain the expected cause and repair even before every final diagnostic code has been assigned.

## Versioning expectations

The handbook currently targets the initial normative Zirk 1.x design. Future language changes should state:

- the version or milestone in which behavior changes;
- whether existing source remains valid;
- whether package IR, ABI, manifests, or lockfiles require migration;
- how the compiler diagnoses old behavior;
- which handbook and reference chapters supersede the previous explanation.

Until such a change is accepted, the final Zirk 1.x specification remains the authority.

## Where to verify a claim

Use the [Handbook Source Map](../_editorial/source-map.md) to locate the governing document. For an implementation claim, inspect the repository's milestone or compiler tests associated with the feature; the normative documents alone cannot prove that code is already supported.

The future implementation-status reference will collect those checks into a single matrix. Until then, chapters must make local status statements conservatively and avoid unsupported “fully implemented” claims.

## Normative sources

- [Zirk Master Specification](../../ZIRK_SPEC_FINAL.md), especially §§3–4 and §8
- [Zirk Compiler and Tooling Specification](../../ZIRK_COMPILER_SPEC.md), especially §§2, 8, and 12
- [Handbook Source Map](../_editorial/source-map.md)

---

**Previous:** [← Why Zirk Exists](./02-why-zirk-exists.md) · **Next:** [Installation →](./04-installation.md)
