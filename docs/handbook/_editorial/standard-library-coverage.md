# Standard Library Coverage Map

This editorial inventory maps every normative area in `ZIRK_STDLIB_SPEC.md` to its handbook owner. A file's existence does not imply completion. Status here describes documentation depth, not compiler implementation.

| Normative area | Handbook owner | Required contract | Initial gap before Block One |
| --- | --- | --- | --- |
| Principles and common contracts | `04-standard-library/01-common-api-contracts.md` | imports, `Result`, exceptions, resources, cancellation, blocking, allocation, permissions, limits, portability | Outline only |
| `std.io` | `02-std-io.md` | streams, formatting, bound methods, EOF algebra, byte/char/line I/O, async behavior | Partial examples; missing complete signatures/errors |
| `std.terminal` | `02-std-io.md` | styles, capability fallback, cursor, progress and coordinated live regions | Added during author review; missing before task 1.2 |
| `std.encoding` | `02-std-io.md` and `03-std-fs.md` | strict UTF-8 default, explicit encoding enum/codecs and lossy boundaries | Added during author review; missing before task 1.2 |
| `std.fs` | `03-std-fs.md` | typed modes/options, `File` resource lifecycle, text/bytes/lines, metadata, async, scoped paths | Partial; missing API and platform table |
| `std.path` | `04-std-path.md` | lexical vs system operations, components, join, normalization, canonicalization, platform rules | Outline only |
| `std.process` | `05-std-process.md` | run/spawn, child resource, streams, environment, cwd, timeout, cancellation, shell separation | Partial |
| `std.text` and native text contracts | `05a-std-text.md` plus type-system handbook | builder, format, safe regex, Unicode String/Char operations | Added and deepened in task 1.3 |
| `std.collections` | `06-std-collections.md` plus collections handbook | types, projection copies, mutation, iteration invalidation, views, equality, complexity, allocation | Deepened in task 1.3 |
| `std.time` | `07-std-time.md` plus temporal handbook | sealed temporal family, clocks/timers/sleep, errors, exact vs calendar arithmetic | Deepened in task 1.3 |
| `std.task` | `08-std-task.md` plus concurrency handbook | handles/scopes, aggregation, settlement, cancellation, channels, blocking bridge | Deepened in task 1.3 |
| `std.thread` | `09-std-thread.md` | thread creation/join/transfer and distinction from task/parallel | Deepened in task 1.3 |
| `std.sync` | `10-std-sync.md` | locks, semaphore, barrier, once, atomics, ordering and race guarantees | Deepened in task 1.3 |
| `std.parallel` | `10a-std-parallel.md` | ordered CPU mapping, reductions, settlement, cancellation, pool behavior | Added and deepened in task 1.3 |
| `std.net` | `11-std-net.md` | addresses, DNS, TCP/UDP, reactor, cancellation, permissions, limits | Deepened as the third task 1.4 sub-block |
| `std.http` | `12-std-http.md` | typed requests/responses, validated headers, streaming, backpressure, TLS, limits, server tasks | Deepened as the final task 1.4 sub-block |
| `std.json` | `13-std-json.md` | exact JSON tree, parse/stringify/typed conversion, generated codecs, streaming, path-aware errors, hostile-input limits | Deepened as the first task 1.4 sub-block |
| `std.crypto` | `14-std-crypto.md` | audited catalog, typed secrets/keys, CSPRNG, password/KDF/MAC, AEAD, signatures/KEM, profiles/providers | Deepened as the second task 1.4 sub-block |
| `std.testing` | `15-std-testing.md` plus testing unit | test decorators/files, runner commands, assertions, reports, benchmarks | Deepened in task 1.5; dedicated testing unit remains Block Two work |
| `std.reflect` | `16-std-reflect.md` plus metaprogramming unit | basic identity and explicit ordinary descriptors; no retained decorators | Deepened in task 1.5 |
| `std.system` | `17-std-system.md` | process/target/signal information and immediate exit boundary | Deepened in task 1.5 |
| `Environment` / `Env` | `18-std-environment.md` | typed reads, nullable/default/required/secret access, listing, permission and redaction | Deepened and signature-audited in task 1.5 |
| Exclusions | module pages and appendices | no framework/ORM/template expansion into core; no unsafe shortcut APIs | Scattered, needs cross-links |

## Documentation rules for this block

- Do not invent a concrete signature where the normative specifications define only a capability. Explain the contract, show explicitly illustrative syntax, and record the unresolved API surface for later language-author review.
- Link detailed type semantics instead of duplicating them in module pages.
- Every waiting API documents whether it blocks a thread or suspends a task, how cancellation is observed, and which resource owns cleanup.
- Every privileged API names its permission family and the typed denial path.
- Complexity is stated where the contract or data structure makes it meaningful; implementation-dependent costs are labeled accordingly.
- Each substantive module page carries target-language implementation status and its normative source.

## Block One completion evidence

This map becomes complete when every row is either documented to its applicable contract or explicitly records a normative API decision still required. Such a decision blocks the affected chapter rather than inviting an invented API.

Task 1.2 completed the common-contract, `std.io`, `std.terminal`,
`std.encoding`, `std.fs`, `std.path` and `std.process` rows. Their owning pages
now cover the accepted signatures and semantics, failures, permissions,
resource/cancellation behavior, limits and platform boundaries. Later tasks
will add the cross-module indexes and final block audit.

---

**Editorial:** [Handbook Source Map](source-map.md) · **Handbook:** [Standard Library](../04-standard-library/README.md)
