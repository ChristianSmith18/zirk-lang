## Context

See `proposal.md` for motivation and scope. The delivered compiler has a non-moving mark-sweep collector, recursive descriptors, native LLVM emission, an IR interpreter, and Rust runtime helpers. Managed roots are currently emitted from type metadata; several runtime containers and side tables do not obey the same ownership model. Numeric behavior is implemented across runtime-core, the native backend, and interpreter, so boundary behavior must be made identical before enabling stronger LLVM optimization.

The existing capability specs define the normative contracts. This design supplies the cross-layer architecture; it does not declare every specified feature delivered until implementation evidence exists. The active `concurrency-completion` change owns the remaining Phase 5 surface. This change only owns cancellation cleanup and type-safety defects needed to meet existing resource/concurrency guarantees; it must coordinate its implementation and tests with that active change and must not duplicate its unfinished APIs.

The active `concurrency-completion` change owns `Concurrent.protect`, app-scope cancellation/drain, and cancellation behavior in `Concurrent.of(...).first/within`; this change does not add those public APIs. The requester explicitly directed this change to continue through its cancellation-cleanup tasks despite the earlier sequencing note. The cleanup-only implementation is therefore serialized in this worktree and must preserve the existing task-state contract: it may not duplicate or redefine the public concurrency surface. Shared files (`lower.rs`, `executor.rs`, `task.rs`, `resource.rs`, and `journal.rs`) are edited only after confirming no concurrent implementation changes are present, and both changes' specs are checked before integration.

## Goals / Non-Goals

**Goals:**

- Make every safe managed reference traceable from roots and from every runtime-owned edge.
- Ensure the collector never reads an object header from an address not verified as a live GC allocation.
- Align type acceptance, lowered representation, runtime ABI, native backend, and interpreter behavior.
- Convert invalid numeric boundaries into deterministic language failures instead of LLVM undefined behavior, Rust overflow, truncation, or backend-dependent results.
- Make graph traversal, cancellation cleanup, and regex retention robust under adversarial input sizes.
- Establish a repeatable correctness and performance baseline before turning on optimized LLVM code generation.

**Non-Goals:**

- Redesign source syntax, replace the memory strategy, or introduce public ownership/finalizer semantics.
- Implement pending zone-aware temporal types, all Phase 5 synchronization APIs, decorators, or a general package manager.
- Make arbitrary native pointers safe. Provenance-opaque pointer extents and FFI remain explicitly unsafe obligations.
- Claim performance improvements without benchmark evidence or change public semantics for speed.

## Decisions

### 1. Treat GC reachability as typed edge metadata, and validate candidates before dereferencing

All compiler-generated roots and heap descriptors SHALL describe only managed references. The runtime SHALL maintain an allocation-membership index updated at allocation and sweep; `mark_object` SHALL verify a non-null candidate is a current managed allocation before reading its header. Invalid candidates SHALL produce a deterministic internal-runtime failure in checked builds and SHALL never be dereferenced. Raw `Pointer<T>` values are never GC roots merely because their LLVM representation is a pointer.

**Why:** A non-moving mark-sweep collector cannot infer type or provenance from pointer-shaped bits. Checking membership provides a final containment boundary for compiler/runtime representation mistakes. A dedicated address index avoids scanning the entire heap for every edge; its cost is included in benchmarks.

**Alternatives considered:** (a) trust all generated pointer paths and retain the current unchecked header read — rejected because an inactive enum field or foreign allocation can corrupt memory; (b) linear search the allocation list for every candidate — safe but potentially quadratic under graph traversal; (c) accept arbitrary non-null words and probe memory — rejected as unsafe.

### 2. Make every managed collection handle a GC allocation with typed contents

`Map` and `Set` handles SHALL use the same managed object header and root rules as other identity-bearing values. Their key/value layout metadata SHALL distinguish scalar words from managed references and recursively describe inline fields where supported. Runtime-owned backing storage SHALL be released exactly once when its handle is swept; no user-observable finalizer is introduced. `Array` and `List` SHALL derive element reference paths recursively from the element type, and those paths SHALL remain correct after growth, slicing, cloning, and materialization.

**Why:** Treating container handles as raw `Box` pointers while registering them as GC roots is invalid; treating entries as untyped `i64` words loses object reachability. Runtime memory and GC-visible memory need one explicit contract.

**Alternatives considered:** (a) make Map/Set permanent external roots — rejected because it leaks every container and still needs entry tracing; (b) keep arbitrary raw boxes but skip root registration — rejected because containers can outlive their storage and their contents; (c) reject every managed-reference element — retained only as a temporary compatibility fallback if typed tracing cannot be delivered, with explicit diagnostics and documented scope.

For `String` and `Char` Map keys and Set elements, the runtime snapshots normalized text at insertion. `String` is a mutable shared reference, so hashing its live handle would let a mutation move the key to a different bucket without re-insertion. The snapshot is owned by the collection body, participates in content equality/hash, and is used when producing a key/element snapshot for rendering. The original text handle is not retained as a collection edge. Mutating the source string after insertion therefore does not mutate the stored key value; lookup compares the current query text with the inserted snapshot.

### 3. Initialize enum storage before it can become a root

Enum construction SHALL initialize the discriminant and every payload storage slot to a valid zero/null representation before inserting the active variant's fields. This keeps existing flattened layouts safe with the current fixed-offset collector. The collector will validate candidate references before dereferencing them. A compact tagged-union layout is a later optimization and is not required for this safety change.

**Why:** The current collector scans fixed field offsets and does not consult a variant tag. Initializing inactive reference slots avoids interpreting LLVM `undef` as a pointer without introducing a new descriptor interpreter in the first safety patch.

**Alternatives considered:** (a) add tag-aware descriptors now — offers less scanning but changes descriptor format, collector, clone, and ABI together; (b) zero-initialize all slots — chosen as a smaller, auditable correctness fix; (c) exclude enum-contained references from GC — rejected because it creates use-after-free.

### 4. Keep exception cache edges with the exception's managed lifetime

Cached stack traces and suppressed exceptions SHALL be treated as edges owned by the exception object. The collector SHALL visit those edges only while their owner is marked and SHALL remove side-table entries when the owner is swept. A side table SHALL NOT root every exception forever, and address reuse SHALL NOT return metadata for an unrelated exception.

**Why:** Strongly rooting the whole thread-local cache would fix stale pointers by retaining every exception indefinitely. Owner-conditioned tracing and sweep cleanup preserve existing cache behavior without turning it into a leak.

**Alternatives considered:** (a) permanently register the entire cache as roots — rejected due unbounded retention; (b) attach fields to the public Throwable layout — possible but changes every subclass layout and needs compatibility analysis; (c) use owner-conditioned side-table edges with sweep pruning — chosen to preserve public object layout.

### 5. Use iterative worklists for graph marking and deep clone

The collector and deep-clone graph traversal SHALL use explicit worklists. A visited/identity table SHALL preserve cycle handling and shared-child semantics. Worklist growth SHALL use checked allocation and existing runtime allocation-failure behavior.

**Why:** Recursion depth currently consumes native stack proportional to user-created graph depth. Iteration makes depth a heap/resource-limit concern and avoids stack exhaustion while preserving graph semantics.

**Alternatives considered:** impose a maximum recursion depth — rejected as an arbitrary semantic limit; enlarge native stacks — does not bound hostile inputs; iterative traversal — chosen.

### 6. Make representability an explicit semantic boundary

The checker SHALL accept a collection key/value/element type only when its equality/hash behavior, LLVM representation, runtime word/layout, and GC metadata are all defined. Where this change implements typed storage, previously accepted combinations remain accepted. Any remaining unsupported combination SHALL fail during semantic analysis with the exact type and unsupported operation; accepted source SHALL never reach a `panic!`/`unreachable!` in code generation because of a missing representation.

**Why:** A compile-time type guarantee is useful only if all later pipeline stages can honor it. This avoids silent pointer truncation and backend crashes.

**Alternatives considered:** (a) silently truncate or reinterpret aggregates — rejected; (b) keep late backend panics — rejected; (c) implement typed layouts or issue a clear semantic diagnostic — chosen, preferring implementation for existing advertised behavior and a narrow, documented diagnostic only where necessary.

### 7. Define one checked numeric boundary shared by native and interpreted execution

Float-to-integer conversions SHALL check finiteness and the exact destination range before conversion. `Decimal.pow` SHALL not narrow an `i128` exponent to `i64`; checked exponentiation SHALL report non-representable results deterministically. Float-to-Duration conversion SHALL reject non-finite, out-of-range, or semantically disallowed precision loss. Civil date/time calculations SHALL use checked wide intermediates; `Time ± Duration` SHALL use a wide intermediate before modulo-day normalization. The interpreter and native runtime SHALL agree on outcome category and value.

**Why:** LLVM conversion instructions and Rust casts have different exceptional behavior from explicit Zirk errors. Sharing range rules prevents target-dependent results and allows optimization without relying on undefined behavior.

**Alternatives considered:** wrapping or saturating conversions — rejected because they contradict checked scalar semantics; panic/abort — rejected because it is not a controlled language failure; explicit checked helpers and wider intermediates — chosen.

### 8. Bound expensive caches and preserve exact text/numeric semantics

The native Regex cache SHALL have explicit entry/byte limits and deterministic eviction or rejection behavior. Regex parsing SHALL reuse the compiled instance it already created rather than compiling twice. Grapheme indexing may cache an index only if invalidation follows String identity/backing mutation and parity tests preserve Unicode segmentation and negative indexing. Decimal digit estimation SHALL remain exact and SHALL NOT route through `f64`.

**Why:** A process-wide cache with unbounded user-controlled patterns creates cumulative memory exhaustion. Repeated grapheme scans and decimal float estimation are avoidable overhead/correctness risk.

**Alternatives considered:** no cache — safe but may regress repeated-pattern workloads; unbounded cache — rejected; bounded cache with deterministic eviction — chosen. For graphemes, eager indexing versus cached indexing remains benchmark-driven, with no cache accepted if invalidation cannot be proven.

### 9. Deliver cancellation through compiler cleanup edges

Cancellation SHALL be represented to generated code as a controlled exit state. Function entry and every call return check cancellation before pending exceptions, so a queued task can stop before its body and cancellation is not catchable. A call-return edge enters a runtime shield, runs the same innermost-first lexical cleanup ladder used by return and exception propagation, closes resources, and rolls back uncommitted unsafe journals exactly once. Exceptions raised by cleanup calls are consumed while cancellation is active, so they cannot skip outer cleanup or contaminate the next task; cancellation remains the terminal outcome. A dedicated IR `Cancel` terminator returns through the existing native ABI; the executor then records the terminal `Cancelled` state. The interpreter maps the same terminator to its cancellation result. The shield is left after cleanup calls, and the executor normalizes it if cleanup itself returns early. Rust panic unwinding SHALL NOT be the mechanism that skips language cleanup through generated frames. Timer wrappers SHALL stop invoking callbacks once their task's token is deliverable.

**Why:** Cancellation is part of the structured concurrency contract and must not bypass deterministic resource cleanup.

**Alternatives considered:** catch arbitrary Rust panic at the executor boundary — rejected because it cannot reconstruct nested language cleanup; compiler-visible cancellation state routed through cleanup ladders — chosen.

### 10. Gate LLVM optimization on correctness parity and reproducible benchmarks

The current unoptimized LLVM path remains the correctness baseline. After the safety tasks pass, add an explicit optimization-level policy (debug baseline, optimized release candidate) and compare native output/failure behavior against the interpreter and unoptimized native build on the full valid corpus plus boundary fixtures. Adopt a higher release level only if semantic parity and representative runtime/compile-time benchmarks pass; otherwise keep the prior level and report the blocker.

**Why:** Optimization can expose latent undefined behavior and there is no current evidence that stronger LLVM optimization preserves the intended edge semantics.

**Alternatives considered:** enable aggressive optimization immediately — rejected; keep `None` forever — safe but leaves a measurable opportunity unexplored; staged, benchmarked enablement after hardening — chosen.

### Cross-layer ABI contract for parallel implementation

The implementation starts from the existing runtime ABI and extends it compatibly within this change; it does not invent a second header format. Every managed allocation returned by `zirk_rt_alloc` has three pointer-sized words: descriptor at byte offset `0`, intrusive next pointer/mark bit at one word, and allocation size at two words. Descriptors retain the existing leading count-and-byte-offset representation consumed by `gc_field_offsets`; every offset is relative to the allocation base and names an initialized pointer-sized managed edge. Static class descriptors may retain their existing class metadata prefix, provided the GC offset table remains at the location expected by the runtime parser. Weak-cell and channel descriptors keep their dedicated runtime handling.

IR `gc_reference_paths` remains the source of truth for root slots and recursively nested inline `Value`, `Enum`, `Closure`, and `Nullable` fields. An aggregate element layout passed to Array/List/Map/Set runtime construction is a set of byte offsets relative to one element/entry; offsets must be aligned, in bounds, and describe only managed references. The LLVM emitter must compute those offsets from the same LLVM struct layout used to store values. Raw `Pointer<T>` and opaque FFI addresses are excluded. An enum's currently flattened storage is zero-initialized before the active variant is written, so every scanned inactive pointer slot is null; the collector validates every non-null candidate against the current heap allocation index before reading its header.

The collector owns allocation membership, mark/sweep, and final release of runtime backing storage. Container runtime modules own their body layout and expose non-allocating trace/drop hooks to the collector; descriptor metadata and backing buffers are released once on sweep. Native LLVM and IR changes must preserve this header, descriptor, and element-offset contract. If a given Map/Set key-value representation cannot be expressed by the agreed offset list and word ABI, semantic analysis rejects that shape before lowering rather than silently reinterpreting it.

For the word-based Map/Set ABI, constructors receive a `string_content` boolean derived from the IR key/element type. It is true for `String` and `Char` and false for all other supported key/element types. In this mode, the runtime snapshots normalized text on insertion and uses that immutable snapshot for content equality, hashing, and rendering. The original mutable handle is not a GC edge of the entry. The typed constructors retain the GC layout arguments and have these C signatures: `zirk_rt_map_new_with_layout(key_offsets, key_count, value_offsets, value_count, string_content_keys)` and `zirk_rt_set_new_with_layout(element_offsets, element_count, string_content_elements)`. LLVM passes null/zero key/element layout metadata in text-key mode and null/zero metadata only when other corresponding types have no managed-reference paths. The legacy zero-argument constructors remain available to existing C callers and retain word identity semantics.

## Risks / Trade-offs

- **[Risk] More descriptors and membership checks increase allocation/GC cost.** → Use typed metadata, an allocation-membership index, and phase-specific GC benchmarks; do not weaken validation to recover speed without evidence.
- **[Risk] Narrowing collection types can break programs that previously type-checked.** → Preserve combinations that can be represented safely; diagnose only the remainder before codegen and document the compatibility impact.
- **[Risk] Internal backing cleanup during sweep could double-free or re-enter the collector.** → Keep the cleanup hook runtime-private, non-allocating, idempotence-tested, and invoke it once after marking and before raw deallocation.
- **[Risk] Cancellation cleanup can interact with exceptions, nested resources, unsafe journals, and `Job` completion.** → Use one lexical cleanup ordering model and test all exit combinations; coordinate with `concurrency-completion` without editing its owned feature scope.
- **[Risk] Optimized LLVM output may diverge from the interpreter in unsupported corners.** → Maintain O0 as an oracle during rollout and compare both result and failure categories.
- **[Risk] Benchmarks may be noisy or not representative.** → Pin workloads, record compiler/runtime versions and host, report variance, and make no optimization claim without repeatable improvement.
- **[Risk] `concurrency-completion` changes the same cleanup and executor paths.** → Keep cancellation implementation serialized behind that change's overlapping tasks, review shared files against both specs, and do not claim either change complete while cleanup behavior is split or duplicated.

### Current optimization-policy implementation status

`zirk-codegen-llvm` now exposes an explicit machine-code optimization level for tests and controlled experiments. Existing `compile_to_object`, host emission, and triple emission APIs remain fixed at `None`, so production builds continue to use the unoptimized oracle. `Less`, `Default`, and `Aggressive` can emit objects through the opt-in API, but no CLI/release setting selects them yet. This is intentional: full interpreter/native/candidate parity and the representative benchmark matrix remain release gates.

The grapheme-index microbenchmark compares the former materialized-offset strategy with the iterator strategy on short ASCII, long combining-mark, and emoji/flag/ZWJ text, with fixed checksums. This is focused evidence for that operation; it does not substitute for whole-runtime allocation/GC, map/set, clone, decimal, regex, or compile-time baselines.

### Runtime hardening benchmark checkpoint

The opt-in release harness was run three times on the same macOS 27.0.1 arm64 host with rustc 1.94.1, release profile, system allocator, fixed workloads, and matching checksums. Medians from the pre-change and hardened sources are:

| Workload | Pre-change median | Hardened median | Change |
| --- | ---: | ---: | ---: |
| GC graph marking under allocation pressure | 0.168 ms | 0.314 ms | +87% |
| Deep clone, 4,096-node chain | 1.006 ms | 1.207 ms | +20% |
| Map insert/remove/lookup | 1.224 ms | 1.394 ms | +14% |
| Set insert/remove/contains | 1.135 ms | 1.358 ms | +20% |

Observed three-run spans (baseline / hardened) were GC 0.167–0.182 / 0.308–0.327 ms, clone 0.951–1.013 / 1.162–1.262 ms, Map 1.196–1.249 / 1.347–1.432 ms, and Set 1.097–1.145 / 1.304–1.561 ms.

The membership index is required to validate pointer candidates before header reads; it uses an address-specific integer hasher while retaining exact `HashSet` equality. The second map/set regressions are reduced by storing scalar entries directly as `i64` and using a separate text-snapshot representation. These checksums match the baseline. GC remains materially slower because safe allocation membership adds work to every traversed edge; the evidence does not justify weakening that boundary. The CLI therefore remains at LLVM `None`. The additional baseline comparisons and their limitations are recorded below.

The remaining fixed-workload comparisons used the same detached-HEAD source, candidate source, host (macOS arm64), and rustc 1.94.1:

| Workload | Pre-change | Hardened | Change |
| --- | ---: | ---: | ---: |
| Decimal, 250k batches of add/sub/mul/div/rem | 136.453 ms | 96.228 ms | -29.5% |
| Regex validation/compile, 100k patterns | 729.298 ms | 737.883 ms | +1.2% |
| Reused regex match, 500k inputs | 4.536 ms | 4.372 ms | -3.6% |
| Small scalar compile, 11 samples | 69.659 ms | 70.183 ms | +0.8% |
| Medium aggregate compile, 11 samples | 71.160 ms | 71.939 ms | +1.1% |

Compile-time sample ranges were 68.954–71.345 ms / 68.861–75.666 ms (small scalar) and 70.117–73.698 ms / 70.567–76.002 ms (medium aggregate), baseline / candidate. Output SHA-256 values match for each fixture. Executable size changed from 520,080 to 538,672 bytes (small scalar) and 538,816 to 693,152 bytes (medium aggregates); code size is a separate cost from build latency.

Across four independent three-round benchmark invocations, Decimal median-of-medians spans were 135.595–137.593 ms / 94.457–97.230 ms, baseline / candidate. The gain comes from binary-search digit counting over a fixed powers-of-ten table and reusing the exact coefficient digit counts during division; it avoids both floating-point order estimation and per-division string allocation. Decimal checksums matched in every round. Regex validation spans were 728.667–737.170 ms / 721.544–756.443 ms, with a later candidate median of 737.883 ms; repeated Regex match spans were 4.484–4.634 ms / 4.178–4.501 ms. The Regex measurements indicate similar compile cost and a small matching improvement, with run-to-run noise.

The grapheme benchmark compares materialized offsets and the iterator on three rounds of 10,000 positive/negative Unicode queries. Current medians were 9.454 / 7.244 ms for short ASCII, 194.388 / 181.020 ms for combining text, and 363.660 / 346.083 ms for emoji/ZWJ/flags; checksums matched. The respective ranges were 9.452–9.580 / 6.850–7.261 ms, 193.507–195.169 / 178.908–182.357 ms, and 357.901–365.845 / 340.402–350.902 ms (materialized / iterator). These are paired algorithm comparisons inside the same process, not allocations-per-operation measurements.

For LLVM levels, a mixed scalar-loop plus Map/Set fixture ran nine times per level in three independent rounds. The median of the three per-round runtime medians was 3.051 ms (`None`), 3.024 ms (`Less`), 2.634 ms (`Default`), and 2.784 ms (`Aggressive`); all output checksums and executable sizes matched. The `Default`/`Aggressive` medians suggest a possible gain on this one fixture, but execution ranges overlap and runs had host pauses above 130 ms. That result is not broad or stable enough to select an optimized level for release. The full corpus parity gate is run separately across all four levels.

## Migration Plan

1. Add and review behavior deltas and adversarial regression tests before changing runtime representation.
2. Implement the GC/runtime and compiler metadata contracts in parallel work packages, then integrate at the existing IR/LLVM ABI boundary.
3. Fix semantic representability and numeric behavior; add cancellation cleanup after shared cleanup ordering is agreed.
4. Run focused tests, the full workspace suite, formatting, Clippy, corpus parity, browser support audit where behavior is shared, and targeted benchmarks.
5. Update handbook, feature status, limitations, roadmap, and examples from verified results. Commit the compiler repository, sync and review `../zirk-lang-site`, and keep the two repository revisions separate.
6. If a regression blocks optimized codegen, retain the unoptimized path. If a type combination cannot be made safe in this change, retain a compile-time diagnostic rather than a fallback representation.

Rollback is limited to compiler/runtime release selection: revert the code change or keep the previous optimization level. Do not roll back to an unsafe collector representation while retaining claims that these safety guarantees hold.
