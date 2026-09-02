# ADR-003 — Phase 4 Research: real evidence to close the memory strategy

- **Status:** research input (not a decision; does not change ADR-003 status)
- **Date:** August 20, 2026
- **Phase:** 4

## Purpose

ADR-003 left the concrete choice of memory strategy open until Phase 4, and explicitly said that closing it requires "a language with closures and real objects to measure" — evaluating it earlier, on toy programs, is the very error ADR-003 set out to avoid since Phase 0.

With Phase 3 (objects, classes, closures, generics, contracts) and Phase 4a/4b/4c (`Result<T,E>`, `throw`/`try`/`catch`/`finally`, `Resource<E>`/`match with`) already in `develop` and actually running, that language exists. This document does not make the strategy decision — it is too large to delegate without direct supervision — but gathers real evidence (Zirk programs that compile and run today) so that the decision is made with data instead of speculation.

**Nothing in `crates/*/src/*.rs` was modified for this document.** It is pure observation: five test programs, compiled and executed with the compiler as it stands, plus reading of the relevant source code.

## Method

1. Full read of `docs/decisions/ADR-003-memoria.md`, `crates/zirk-runtime/src/memory.rs`, and the three normative sections that ADR-003 cites or that turned out to be relevant: `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` (public memory model, `Weak<T>` semantics, `clone()` contract) and `docs/ZIRK_LANGUAGE_SPEC.md` §6 (functions and closures).
2. `grep` for `ADR-003`, `reference count`, `GC`, `arena`, `region` across all of `crates/` — to confirm which other parts of the compiler already anticipate (or leave room for) a concrete strategy.
3. Five Zirk programs (`.zrk`) written to exercise genuinely different memory patterns, compiled and executed with `cargo run -p zirk-cli -- run file.zrk`. The full programs are shown below because the details of each are the finding itself.

## What the compiler already anticipates

`crates/zirk-runtime/src/memory.rs` is, as its own comment declares, the only point where the strategy materializes: `zirk_rt_alloc` allocates zeroed memory and **never frees it**, deliberately, until this decision is closed. `crates/zirk-ir/src/ir.rs` and `crates/zirk-ir/src/lower.rs` confirm that the IR only says `alloc <type>` — no IR operation names malloc, reference counting or collection. `crates/zirk-codegen-llvm/src/runtime.rs` cites ADR-003 in the same sense.

`ADR-012-layout-de-objetos.md` (Phase 3, already accepted) already reserved the space: an object's header is separated from its fields precisely "for whatever Phase 4 needs for memory — flags, counters, whatever the chosen strategy asks for" — without shifting the indices of the real fields. It is the only structural preparation that exists today; it commits to no concrete strategy.

No trace of reference counting, GC, arenas or regions already implemented or partially modeled anywhere else in `crates/` was found.

## The five programs and what they showed

The full files remained in this session's scratchpad (not in the repository, to avoid interfering with other agents' parallel work); what is relevant from each is summarized here.

### Probe 1 — a real reference cycle

Two `Node` objects linked by a `mut next: Node?` field, built with `a.next = b; b.next = a;` after `construct()` (there is no way to tie the cycle inside the constructor itself because there is no deferred initialization or forward references). The program traverses the cycle twice and confirms it returns to the same object.

**Result:** the cycle is perfectly constructible in Zirk as implemented today, using only classes and nullable fields — nothing special or forced. This answers affirmatively, with evidence and not by spec inference, ADR-003's constraint "cycles must be freed correctly": cycles are not a rare hypothetical case; they are the natural result of two objects that mutually reference each other, a common design pattern (observer, bidirectional parent/child, doubly-linked lists).

### Probe 2 — closures capturing an object (and an unlooked-for finding)

When trying to write the scenario ADR-003 explicitly asks to measure — "a closure that escapes its frame and keeps a captured object alive" — the program did not compile. The reason is a design decision already made and documented in the compiler itself: **decision D9** (`crates/zirk-sema/src/checker.rs:7519`, `crates/zirk-parser/src/parser.rs:1396`, confirmed in `crates/zirk-sema/tests/typing.rs:3019` by the test `invalid_closure_returned_from_a_function`) deliberately forbids annotating `Fn(...) => R` as a return type, parameter type, field type or generic argument. The compiler's own error message says it unambiguously: *"function types exist in the language, but this phase's parser and checker reject the annotation on purpose"*.

The practical consequence: **today a closure in Zirk can only live in an inferred `mut`/`inmut` local binding, in the scope where it was created or a nested one**. It cannot be returned from a function, stored in an object field, passed to a typed parameter or to a generic parameter. The test program was rewritten to show what is possible today: two independent closures over two independent `Counter` objects (isolated mutation, correct), and two closures over the *same* `Counter` object (visible mutation across both — capture by shared reference, as `MEMORY_AND_UNSAFE_SEMANTICS.md` §1–2 requires), all inside `main`'s frame.

**Result:** the scenario "a closure that escapes its creating frame and keeps alive an object that would otherwise be garbage" — the one ADR-003 §criterion 1 explicitly asks to measure — **is not constructible in the language as implemented today**. It is not that the runtime handles it badly; it is that the language surface still does not allow building the case. This is not a defect of the probe: it is real information about Phase 3's state that ADR-003 needs so as not to overshoot its own decision criterion.

### Probe 3 — a hand-rolled linked list (and several real compiler bugs)

Because the roadmap leaves native collections (`Array`, `List`, `Map`, `Set`) for Phase 7 — confirmed by grep: that type does not exist in `crates/zirk-runtime` or `crates/zirk-sema` — this probe builds a simple linked list with classes and a `next: Node?` field, the form that will carry almost all "real" memory pressure until Phase 7 arrives.

Writing this probe exposed several genuine compiler bugs, unrelated to the choice of memory strategy itself, but relevant because they block precisely the kind of program needed to evaluate it:

- There is no "forced unwrap" operator over `T?`. `?.` cannot appear on the left-hand side of an assignment (`E0301`, *"the left-hand side of an assignment must be a place"*). `match nullable { null => ..., binding => ... }` **does not narrow** the type of `binding` to non-null in the branch that is not `null`. The classic `previous`/`cursor` pointer walk of a linked list needs to write `previous.next = ...` where `previous` may or may not be null, and today there is no direct way to express it — the sentinel-node technique (a dummy head so `previous` is never null) is the workaround this probe ended up using.
- `nullable_thing ?? null` — a redundant but natural expression ("keep this nullable") — **panics compilation**: `crates/zirk-ir/src/lower.rs:5415`, *"the type of this expression comes from the value it produced"*.
- The finding with the most weight: **`object.field = <expression containing `?.`>` reproducibly produces invalid IR**, with error `E0508` *"uses value defined in another block; values do not cross blocks in this IR"*. It was reproduced in its minimum form with ten lines (`a.next = b?.next;`, no loop, no match, no recursion, no `this`). Assigning the value derived from `?.` first to a local variable, and then that local to the field, avoids the problem — the workaround used by `remove_front` in the final probe. This probably explains several of the intermediate failures found while writing this probe (the two-pointer walk, removing an intermediate node): they all wrote a value derived from `?.` directly into a field.
- A recursive function over `Node?` that matches null/non-null and combines a recursive call with a `?.` read of the discriminated value in the same arm produces the same `E0508` — that is, **recursive traversal of a linked structure, the natural way to walk it without native iteration helpers, is broken today**, regardless of the field-assignment bug above.

Because of these bugs, the final probe was limited to building the list (`push_front`) and removing only the head node (`remove_front`), instead of removing an intermediate node. With that limitation, the program ran correctly: after `remove_front()`, the node that held the removed value is left with no references reachable from the rest of the program — exactly the kind of acyclic garbage that any real collection strategy should reclaim immediately.

**Result:** beyond the memory finding itself (mutating linked structures produces acyclic garbage naturally, without incidental cycles), this probe leaves evidence that **building "real" Zirk programs with object graphs today trips over genuine and non-trivial compiler bugs**, concentrated in the interaction between nullable object types and field assignment. This is relevant for ADR-003 indirectly but really: any future measurement over more elaborate programs (larger graphs, recursive traversals) will need these bugs fixed first — they are not an obstacle of the memory strategy, they are an obstacle to *measuring* it.

### Probe 4 — `Resource<E>` holding an object graph, with an exception in flight

A class `Connection implements Resource<ConnectionError>` that holds a reference to another heap object (`lastEntry: LogEntry?`) as its own state. Inside a `match ... with`, the resource registers two entries, also creates a purely local `LogEntry` object (never stored anywhere reachable), and throws an exception (`BoomError`). `close()` reads `this.lastEntry` — a reference inside the resource's own graph — while the exception is actively unwinding.

**Result, with real output:**

```
closed secondary last saw: only entry
1
never stored anywhere reachable
closed primary last saw: second entry
caught: boom while using the connection
done
```

The line `closed primary last saw: second entry` is the central data: it is printed *during* exception unwinding, and `this.lastEntry` correctly points to the last entry registered before the `throw`. This confirms with evidence (not just spec reading) that the `Resource<E>`/`match with` from Phase 4c keeps the resource's own object graph — the resource object itself and everything it directly references — alive and correct throughout the whole unwind, until `close()` finishes. It is exactly what ADR-003 §criterion 3 ("interaction with `Resource<E>`") asks to verify, and here it is verified over a program that actually executes, not over a reading of the spec.

The local object (`scratch`, the "never stored anywhere reachable" entry) has no observable effect once the `match` ends — neither through the normal path nor the exception path — consistent with being immediately collectible garbage under any real strategy.

### Probe 5 — quantified cost of "never freeing"

A loop of 10,000,000 iterations, each building the two-node cycle from probe 1 and immediately discarding all references to it. Under any real collection strategy, each iteration would produce garbage before the next one begins.

**Measured result** (`/usr/bin/time -l`, macOS arm64, unoptimized development build):

| Iterations | Allocated objects | Max RSS | Real time |
|---|---|---|---|
| 2,000,000 | 4,000,000 | ~130 MB | 2.09 s |
| 10,000,000 | 20,000,000 | ~644 MB | 2.38 s |

Growth is linear with the number of live-and-never-freed objects (~32 effective bytes per object, consistent with a descriptor header plus two pointer/`Int32` fields for a `Node` of this size), and the program finishes without failing within the tested range. This confirms exactly what the `memory.rs` comment asserts without having measured it: "a program of this phase terminates and the operating system reclaims everything — that is the limit, and it is written instead of assumed". A short-lived program (CLI, script, batch) tolerates the current "never frees" without issue; a long-lived program (server, event loop) would not tolerate it beyond minutes or hours depending on allocation rate — data no prior measurement had put into numbers.

## Compiler bugs found (not fixed here)

To have them recorded in one place and not lost among the five probes:

1. `is` between two operands of type `T?` (object) panics LLVM codegen: `crates/zirk-codegen-llvm/src/emit.rs:1740`, *"Found StructValue but expected the IntValue variant"*.
2. `is` between an operand `T` and one `T?` produces invalid IR: `E0508`, *"Identical between object? and object"*.
3. `nullable ?? null` panics IR lowering: `crates/zirk-ir/src/lower.rs:5415`.
4. `object.field = <expression with `?.`>` produces invalid IR (`E0508`, *"values do not cross blocks"*) — minimum 10-line repro, no `this`, no loop, no match.
5. A recursive function that matches a `T?` and combines the recursive call with a `?.` read of the discriminant in the same arm produces the same `E0508`.

They are left documented because they directly block the kind of program Phase 4 needs to keep writing to finish closing this decision — not because this document should resolve them.

## Which ADR-003 questions are answered

- **Can the language build real cycles today?** Yes, trivially, with classes and nullable fields (probe 1). The "cycles must be freed correctly" constraint is not hypothetical: it applies to an ordinary design pattern, not an extreme case.
- **Does `Resource<E>`/`match with` interact well with an object graph, even during exception unwinding?** Yes, verified with real execution (probe 4): the resource itself and what it directly references remain correct until `close()` finishes.
- **What shape does the garbage produced by ordinary structure mutation (not deliberately created cycles) take?** Acyclic and immediately scoped: removing a node from a list, or leaving a local object uncaptured, produces garbage with no references reachable from anywhere in the instant the scope that held it ends (probes 3 and 4).
- **How much does "never free" cost in concrete terms?** Linear, measured: ~32 effective bytes per object of this size, without failing up to 20 million live-and-discarded objects in this session (probe 5). Enough for any test program of this project so far; not representative of a server load.

## Which questions remain open

- **ADR-003 criterion 1 ("behavior with capturing closures") cannot be measured yet**, because the scenario that gives it meaning — a closure escaping its creating frame — is not expressible in the language while decision D9 remains in place. This is not a limitation of this document: it is a real gap between what ADR-003 asks to measure and what Phase 3 implemented. Closing this part of ADR-003 with real evidence requires, as a precondition, that `Fn(...) => R` stop being blocked as an annotatable type — or at least that some other path exist by which a closure escapes its frame (a field, a collection) against which to measure.
- **Criterion 2 ("cost of the barrier in `parallel for`") cannot be measured at all**: no `parallel` or `thread` keyword was found implemented in the lexer/parser (`crates/zirk-lexer`, `crates/zirk-parser`). Structured concurrency is, evidently, a phase after this one.
- **Criterion 3 ("interaction with the C ABI boundary")** was not exercised in this document — the five probes are pure Zirk, with no `unsafe`, `Pointer<T>` or native calls. `MEMORY_AND_UNSAFE_SEMANTICS.md` §5, §7 and §8 (automatic pins, `Pointer<T>`, validated native views) describe the surface, but it could not be confirmed against real execution whether that surface is already implemented; that would be the next natural probe.
- **Criterion 4 ("measured pauses against a budget")** does not apply yet: there is no collector implemented over which to measure pauses.
- **`Weak<T>` and `Clone` (`MEMORY_AND_UNSAFE_SEMANTICS.md` §4 and §6), both cited by the public memory model, have no trace in `crates/zirk-sema` or `crates/zirk-runtime`** — no `Weak` was found in any compiler `.rs`. They are part of the contract that the chosen strategy will have to satisfy, but today they are aspirational, not implemented. It is worth whoever closes ADR-003 knowing that designing against that API is designing against something that does not yet exist.

## Informed leaning (not binding)

With the evidence gathered — not as a replacement for the decision criterion, but as a reading of what can already be observed — two things look relatively clear and one remains genuinely open:

**Pure RC remains ruled out**, and now with evidence in addition to spec deduction: probe 1 shows that a two-object cycle is exactly the result of an ordinary design pattern (two classes that mutually reference each other), not a laboratory case. Any final strategy needs tracing, a cycle collector, or both.

**The probable direction ADR-003 already proposed — tracing with escape analysis to promote to the stack + inline value types — remains reasonable**, and probe 5 does not contradict it: the current "never frees" sustains without effort loads of thousands to millions of objects in short-lived programs, so there is no performance urgency pushing toward RC (simpler to implement but worse for cycles) instead of tracing. That said, probe 2 means that **this direction still cannot be validated against the case that would most justify it** — closures that capture and escape, exactly where escape analysis would have the most to say between stack and heap — because that case is not constructible in the language yet. The leaning toward tracing + escape analysis remains reasonable by elimination and by what could be measured, but **it is not confirmed by the case that would most test it**, and whoever closes ADR-003 should know that this gap exists before treating the probable direction as already validated.

## Limitations of this research

- There are no native collections (Phase 7): every data structure in these probes is a hand-written class. A real `Array<T>`/`List<T>` could exhibit different allocation patterns (contiguous blocks instead of individual nodes) that these probes do not cover.
- No interaction with concurrency (`parallel`/`thread` not implemented) or the C ABI/`unsafe` boundary was exercised.
- Probe 5 measurements are from an unoptimized development build, on a single machine (macOS arm64), and should not be read as a definitive benchmark — they are order-of-magnitude evidence, not a reference number.
