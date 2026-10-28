# Documentation audit ledger

Baseline source revision: `8318f0e3da8e6c29677b16d1c38da3a813b85de7`.
Audit date: 2026-10-28.

## Scope and method

- Maintained source handbook/docs inventory: 440 Markdown files under `docs/` and 3 top-level guidance files (total 443).
- Current normative specification inventory: 78 Markdown files under `openspec/specs/`.
- Current standard-library source inventory: 22 `.zrk` files under `stdlib/`.
- Companion-site owned status inventory: 6 source/generator/test/generated files listed below.
- Each path in these inventories has one outcome in the inventory tables. Discrepancies below identify the previous claim, its canonical owner/evidence and disposition. The full corpus was inventoried and checked with targeted searches for implementation/status language, removed declaration names, collection families, target qualifiers, package names, standard-library names and duplicated API signatures. Relevant current-status matches were reviewed in context; ordinary uses of “record” (for example, a permission record) were distinguished from the removed declaration form. Documents without a discrepancy are explicitly recorded.
- Historical dated audits, examples and decision snapshots are preserved as historical evidence and are not silently rewritten to present-day status. Archived OpenSpec changes, vendored manuals and generated build trees are classified separately.
- Evidence is not inferred from source-file presence or completed task checkboxes. Targeted executable validation: current `target/debug/zirk check` on a sequence API fixture and `node scripts/format-doc-examples.mjs --check target/debug/zirk` over all Zirk fences in `docs/`. Full strict OpenSpec/site validation is tracked in the change tasks and will be recorded below after running.

## Discrepancy register

| Topic | Files / sections | Previous claim or conflict | Canonical owner and evidence | Required edit and disposition | Outcome |
|---|---|---|---|---|---|
| Removed declaration model | Getting started; learning paths; built-in types; grammar; type checker; pattern matching; `docs/ZIRK_LANGUAGE_SPEC.md`; dated checkpoint in `docs/ZIRK_SPEC_FINAL.md`; ADR-027; project status | Claims or examples treated former `record`/`interface` declarations as current constructs, or used “record” ambiguously for old declaration forms. | `docs/decisions/ADR-027-contract-object-model.md`; current language specification; migration diagnostics and parser/type-system implementation. Dated examples include `docs/browser-oop-coverage.md` and the August checkpoint in `docs/ZIRK_SPEC_FINAL.md`. | Reframed current model around nominal classes, structural `contract`, open `Object`, tuples and enums; qualified historical decision/support snapshots; clarified `Record<K, V>` as a mapped-object utility. | Resolved; historical records retained with explicit scope. |
| Type and collection status | Everyday types; algebraic enums; unions; intersections; Map/Set; current limitations; feature matrix; feature status; init status; roadmap | Generic/unions/intersections and Map/Set were described as uniformly supported or uniformly missing; some claims omitted target/evidence boundaries. | `docs/init/ZIRK_FEATURE_STATUS.md`; `docs/handbook/11-reference/12-feature-status.md`; current type/collection specs; fixtures `generic_enum_either.zrk`, `generic_enum_nested_payload.zrk`, `clone_enum.zrk`, `ordered_map_set_basics.zrk`, `map_aggregate_values.zrk`; focused type-algebra CI run 40497897350/40497901428. | Documented generic substitution/enum dispatch gaps, supported-vs-unsupported alternative families, target-matrix limits, Map/Set admitted key shapes, Decimal? and the 4096-byte inline payload contract. Updated normative spec wording to remain scoped. | Resolved to evidence-supported scope; broad matrices remain open work. |
| Enum Clone and generic dispatch | Cloning; enums; feature status; current limitations; roadmap; site status | Clone of enums conflicted with a blanket limitation; generic enum dispatch and its matrix lacked a clear boundary. | Feature-status catalog, clone/type-lowering code and current focused fixtures/CI cited by status owner. | Marked derived enum Clone delivered for its implemented composition and preserved recursive/layout and generic-dispatch limits separately. | Resolved for declared tested composition; remaining matrix explicit. |
| Sequence collections | Collections overview; Array/List/Range pages; std.collections page; standard-library indexes; roadmap | Deque/Queue/Stack/PriorityQueue and several List/Array/Range operations were still called absent; reduce signature/order, iterator/eager pipelines and package surface were conflated. | `docs/init/ZIRK_FEATURE_STATUS.md`; `docs/handbook/11-reference/12-feature-status.md`; `crates/zirk-cli/tests/corpus/valid/collection_sequence_pipeline.zrk`; current compiler-known API. | Corrected delivered sequence families/operations and examples; marked remaining adapters/conversions specified. Clarified compiler-known surfaces versus importable `std.collections`, including the open design comparison. | Resolved; package choice remains explicitly undecided/incomplete. |
| Map/Set structural equality | Map/Set chapters; standard-library index; type status; site catalog | Some rows implied broad aggregate key/element support and missed tuple String/Char content equality, structural derivation, union variant consistency and Decimal?. | `openspec/specs/map-collections/spec.md`; `openspec/specs/set-collections/spec.md`; runtime `crates/zirk-runtime/src/map.rs` and `set.rs`; focused CLI fixtures `map_aggregate_values.zrk`, `ordered_map_set_basics.zrk`, and rejected `map_unrepresentable_word_values.zrk`; feature status. | Updated admitted shape and missing combinations; kept byte-canonical tuples/scalars/enums that are already covered separate from missing content-derived equality/hash. | Resolved; missing combinations remain explicit implementation work. |
| Temporal API | std.time; standard-library indexes; limitations; roadmap; site catalog | Date/Time/DateTime/Duration were mixed with unimplemented timezone, zone-aware and locale types. | `stdlib/std` source inventory; temporal compiler/runtime fixtures including `datetime_collection_keys.zrk`; feature-status catalog. | Separated delivered civil temporal types from Instant, ZonedDateTime, TimeZone, Period, TimeShift and locale gaps. | Resolved. |
| JSON API | std.json; standard-library indexes; limitations; roadmap; site catalog | The index called JSON wholly unimplemented despite delivered parse/stringify and manual codecs. | `stdlib/std/json/*.zrk`; CLI fixtures `json_contract_codec.zrk` and `json_stringify_dynamic.zrk`; current status catalog. | Separated basic parse/stringify and manual codecs from generated codecs, configurable limits, JSONPath/Pointer, streaming, pretty/canonical output, schema validation and JSON5. | Resolved. |
| Network/HTTP evidence | std.net; std.http; standard-library indexes; status snapshots; site catalog | Network/HTTP was described as wholly unavailable or supported by one clean all-target pass, omitting optional browser/HTTP3 and a Windows temp executable cleanup failure. | `docs/init/ZIRK_FEATURE_STATUS.md`; workflow runs 39739088146, 39702042337, 39748669844, 39752131334; source/runtime test evidence. | Dated the 2026-10-22 evidence snapshot and described the evidence as composite; separated browser outbound Fetch and optional HTTP/3/QUIC/WebSocket boundaries. | Resolved with a historical evidence date; no claim of a clean all-target green run. |
| Concurrency primitives | `docs/handbook/02-handbook/02-bindings-and-values/01-mut.md`; std.thread; std.sync; concurrency handbook; feature status; site catalog | Mutex was listed as absent even though scoped `Mutex<T>.with` is delivered; general Thread handles and other synchronizers were incorrectly grouped with it. | `docs/init/ZIRK_FEATURE_STATUS.md`; CLI fixtures `mutex_with_mutates_payload.zrk` and `mutex_with_exception_unlock.zrk`; Mutex source/compiler/runtime tests and current CI evidence. | Marked scoped Mutex.with and Thread.run/scalar Atomic delivered; left general Thread<T>, RwLock, Semaphore, Barrier, Once and ConditionVariable unavailable. | Resolved. |
| Resource cleanup and errors | Resource chapters; errors chapters; feature status; current limitations; site catalog | Delivered grouped acquisition, transfer, cancellation-aware cleanup and ResourceFailure result composition were reported as pending; suppression/trace capture needed a tighter boundary. | `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`; CLI fixtures `resources_match_with_closes_on_every_exit.zrk`, `resources_match_with_closes_on_thrown_exception.zrk`, and `transfer_resource.zrk`; resource/error implementations and status catalog. | Marked delivered slices and left automatic close-error suppression during an already-propagating exception and actual stack-frame capture incomplete; noted placeholder trace behavior. | Resolved. |
| Standard library module delivery | Standard-library indexes; module chapters; feature status; site catalog | Importable source packages were conflated with compiler-known APIs, and JSON/network modules were mislabeled absent. | `stdlib/std/**`; package manifests; current module resolution and CLI tests. | Aligned delivered/partial/specified statuses and explicitly stated that collections remain built-in compiler surfaces without an importable `std.collections` package. | Resolved. |
| Feature matrix and roadmap | Feature matrix; feature-status page; current limitations; construction roadmap; init status/roadmap; dated planning snapshot `docs/decisions/proximos-pasos-fase-4.md`; site catalog | Multiple roadmap/status surfaces retained contradictions about shipped sequence operations, static enum members, regex/string split, resource lifecycle and generic dispatch. | Canonical feature-status catalog and dated source/CI evidence. | Reconciled current claims and retained remaining work in specific terms; avoided upgrading features from proposal/task status alone. | Resolved. |
| Cross-links and examples | Reference indexes; type pages; collection pages; onboarding; tutorials | Some links and code examples pointed at legacy record labels or contained stale API order/signatures and claims about iterator collection pipelines. | Current handbook route tree; compiler parser/formatter; `collection_sequence_pipeline.zrk` fixture. | Updated labels/status and corrected runnable sequence examples; duplicate stale tail section in List chapter removed. | Resolved where discovered; route validator will verify all links. |
| Site-owned status catalog | `../zirk-lang-site/tools/project-status/catalog.ts`; generated status/content inputs | Public status copy duplicated stale type, resource, sync, collection, JSON/time and network evidence claims. | Audited source catalog and source docs at committed revision; site generator assertions. | Aligned owned catalog now; source descriptors and generated output will be refreshed from the committed source revision. | Pending sync/build/commit until later tasks complete. |

## Per-file outcome inventory

Outcome legend: `Updated` means a discrepancy was corrected; `No discrepancy found` means the maintained file was scanned/reviewed and no current unsupported contradiction was found; `Historical preserved` means dated material remains clearly a historical snapshot. Companion-site rows record whether each source changed or was reviewed unchanged; the completed regeneration and validation evidence is recorded below.

### Maintained project guidance

| Path | Audit outcome |
|---|---|
| `README.md` | No discrepancy found; scoped to contributor/project instructions. |
| `CONTRIBUTING.md` | No discrepancy found; scoped to contributor/project instructions. |
| `AGENTS.md` | No discrepancy found; scoped to contributor/project instructions. |

### `docs/` Markdown corpus (440 files)

| Path | Audit outcome |
|---|---|
| `docs/01_plantilla_zirk.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/COMPILER_IMPROVEMENT_ROADMAP.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/CORE_LANGUAGE_SEMANTICS.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/DECORATOR_SEMANTICS.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/MIGRATING_FROM_RECORD_INTERFACE.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/RELEASE_NOTES.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/SYNTAX_HIGHLIGHTING.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/TOOLCHAIN.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/ZIRK_COMPILER_SPEC.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/ZIRK_LANGUAGE_SPEC.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/ZIRK_RUNTIME_SPEC.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/ZIRK_SPEC_FINAL.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/ZIRK_STDLIB_SPEC.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/browser-interpreter-support-matrix.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/browser-oop-coverage.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/browser-package-handoff.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/2026-08-20-auditoria-documentacion-y-specs.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/2026-09-15-project-status-audit.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-001-pin-llvm.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-002-runtime-staticlib.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-003-investigacion-fase-4.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-003-memoria.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-004-portabilidad.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-005-representacion-string.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-006-language-of-the-codebase.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-007-forma-de-la-ir.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-008-color-en-diagnosticos.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-009-eliminacion-de-codigo-muerto-al-enlazar.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-010-ubicaciones-multiarchivo.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-011-identidad-e-igualdad-de-string.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-012-layout-de-objetos.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-013-forma-del-despacho.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-014-representacion-de-char.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-015-declaracion-extern.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-016-module-system.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-017-modelo-de-suspension.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-018-concurrency-surface.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-019-parallel-and-multithreaded-gc.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-020-browser-ir-interpreter.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-021-package-and-module-boundaries.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-022-browser-execution-trace.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-023-concurrency-completion-contracts.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-024-contextual-result-value-projection.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-025-unified-result-and-nullable-stdlib-outcomes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-026-result-statement-effects.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/decisions/ADR-027-contract-object-model.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/decisions/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/decisions/proximos-pasos-fase-4.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/development/HTTP3_DEPENDENCY_REVIEW.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/development/SECRET_REDACTION_CHECKLIST.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/development/STDLIB_SOURCE_AND_HOST_BOUNDARY.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/00-getting-started/01-what-is-zirk.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/00-getting-started/02-why-zirk-exists.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/00-getting-started/03-language-status.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/00-getting-started/04-installation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/00-getting-started/05-first-program.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/00-getting-started/06-how-zirk-compiles.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/00-getting-started/07-next-steps.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/00-getting-started/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/01-learning-paths/01-zirk-for-new-programmers.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/01-learning-paths/02-zirk-for-typescript-programmers.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/01-learning-paths/03-zirk-for-python-programmers.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/01-learning-paths/04-zirk-for-rust-c-cpp-programmers.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/01-learning-paths/05-zirk-for-java-csharp-programmers.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/01-learning-paths/06-tooling-in-five-minutes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/01-learning-paths/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/01-source-files.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/02-lexical-rules.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/03-comments-and-documentation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/04-statements-and-semicolons.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/05-blocks-and-scope.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/06-naming-conventions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/01-program-structure/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/01-mut.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/02-bindings-and-values/02-inmut.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/03-strict-immutability.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/04-type-inference.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/05-default-values.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/06-definite-initialization.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/07-destructuring.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/07a-multiple-bindings-and-assignment.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/08-shadowing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/09-value-and-reference-semantics.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/02-bindings-and-values/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/00-how-values-live-and-share.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/01-object-and-type-hierarchy.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/01a-type-categories.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/03-everyday-types/01b-value-and-reference-behavior.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/03-everyday-types/01c-contracts-and-capabilities.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/01d-conversions-and-context.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/01e-native-operators.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/01f-choosing-a-type.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/02-signed-integers.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/03-unsigned-integers.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/04-decimals.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/05-numeric-literals.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/06-overflow-and-arithmetic-safety.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/07-boolean.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/08-char.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/09-string.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/09a-triple-quoted-strings.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/10-void-never-null-object.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/11-duration.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/12-regex.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03-everyday-types/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/01-choosing-a-temporal-type.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/02-date.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/03-time.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/04-date-time.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/05-instant.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/06-time-zone.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/07-zoned-date-time.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/08-duration.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/09-period.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/10-composition-matrix.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/11-temporal-arithmetic.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/12-parsing-and-formatting.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/13-zones-and-dst.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/14-temporal-errors.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/03a-temporal/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/04-nullability/01-nullable-types.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/04-nullability/02-safe-access.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/04-nullability/03-null-coalescing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/04-nullability/04-flow-analysis.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/04-nullability/05-common-nullability-errors.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/04-nullability/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/01-arithmetic.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/02-comparison.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/03-structural-equality.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/04-object-identity.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/05-operators-and-expressions/05-logical-operators.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/06-ternary.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/07-increment-and-decrement.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/08-compound-assignment.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/09-pipe-operator.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/10-ranges.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/11-operator-overloading.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/05-operators-and-expressions/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/01-if-and-else.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/02-if-expressions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/03-match.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/04-exhaustiveness.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/05-for.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/06-for-in.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/07-while.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/08-loop.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/09-break-and-continue.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/06-control-flow/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/01-declaring-functions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/02-parameters-and-return-types.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/03-optional-parameters.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/04-default-parameters.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/05-named-arguments.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/06-variadic-functions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/07-lambdas.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/08-closures-and-capture.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/09-mutability-in-parameters.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/07-functions/10-no-traditional-overloading.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/11-never-returning-functions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/12-function-types-and-callable-values.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/07-functions/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/01-declaring-classes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/02-fields-and-properties.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/03-constructors.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/04-instantiation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/05-this.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/06-visibility.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/07-inheritance.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/08-classes-and-objects/08-abstract-classes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/09-static-members.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/08-classes-and-objects/10-object-identity.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/08-classes-and-objects/11-cloning.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/08-classes-and-objects/12-nested-classes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/08-classes-and-objects/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/09-interfaces-and-traits/01-interfaces.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/09-interfaces-and-traits/02-implementing-contracts.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/09-interfaces-and-traits/03-traits.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/09-interfaces-and-traits/04-default-implementations.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/09-interfaces-and-traits/05-interface-vs-trait-vs-class.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/09-interfaces-and-traits/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/00-tuples.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/01-records.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/03-traditional-enums.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/04-algebraic-enums.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/10-data-types/05-associated-values.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/06-type-aliases.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/07-union-types.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/10-data-types/08-class-or-record.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/10-data-types/09-intersection-types.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/10-data-types/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/11-generics/01-generic-functions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/02-generic-types.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/03-constraints-with-from.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/04-multiple-type-parameters.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/05-inference.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/06-specialization.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/07-monomorphization.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/08-variance-recursion-and-runtime.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/09-type-operators.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/11-generics/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/12-collections/01-arrays.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/12-collections/02-fixed-arrays.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/12-collections/03-lists.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/12-collections/04-maps.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/12-collections/05-sets.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/12-collections/06-ranges.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/12-collections/07-indexing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/12-collections/08-slicing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/12-collections/09-safe-access.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/12-collections/10-collection-contracts-and-complexity.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/12-collections/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/01-iterable.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/02-iterator.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/03-generators.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/04-map.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/05-filter.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/06-reduce.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/07-lazy-operations.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/08-pipelines.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/13-iteration-and-functional-style/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/01-value-patterns.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/02-multiple-patterns.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/03-type-patterns.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/04-enum-patterns.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/05-union-patterns.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/06-destructuring-patterns.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/14-pattern-matching/07-match-as-statement.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/08-match-as-expression.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/14-pattern-matching/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/15-errors/01-result.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/15-errors/02-handling-result.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/15-errors/03-error-propagation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/15-errors/04-exceptions.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/15-errors/05-typed-catch.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/15-errors/06-finally.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/15-errors/07-fatal-error.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/15-errors/08-assertions.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/15-errors/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/16-resources/01-resource-contract.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/16-resources/02-match-with.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/16-resources/03-resource-transfer.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/16-resources/04-close-and-flush-errors.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/16-resources/05-cancellation-and-cleanup.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/16-resources/06-why-no-defer-or-destructors.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/16-resources/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/17-memory-and-safety/01-memory-model.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/02-stack-and-heap.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/03-automatic-memory-management.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/04-safe-references.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/05-pointers.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/06-dereferencing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/07-unsafe-blocks.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/08-bounds-safety.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/09-null-safety.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/10-use-after-free.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/11-undefined-behavior.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/12-transactional-unsafe-and-commit.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/17-memory-and-safety/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/18-concurrency/01-concurrency-vs-parallelism.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/04-structured-concurrency.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/05-cancellation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/06-timeouts.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/07-channels.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/08-parallel.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/09-parallel-for.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/10-threads.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/11-workers-by-composition.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/12-sync-and-mutex.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/13-atomics.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/14-data-race-prevention.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/18-concurrency/15-timer.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/16-job.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/17-transfer-sharing-and-captures.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/18-concurrency/README.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/19-modules/01-file-modules.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/19-modules/02-share.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/19-modules/03-import.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/19-modules/04-import-aliases.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/19-modules/05-use-globals.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/19-modules/06-public-api.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/02-handbook/19-modules/07-circular-dependencies.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/19-modules/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/02-handbook/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/01-project-layout.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/02-zkinit.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/03-applications.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/04-libraries.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/05-entry-point.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/06-globals.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/07-runtime-permissions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/08-compile-permissions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/09-library-requirements.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/10-build-targets.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/11-dependencies.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/12-lockfile.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/13-reproducible-builds.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/14-permission-approval-and-audit.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/15-migrating-zkinit.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/03-projects/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/01-common-api-contracts.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/02-std-io.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/03-std-fs.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/04-std-path.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/05-std-process.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/05a-std-text.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/06-std-collections.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/07-std-time.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/09-std-thread.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/10-std-sync.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/10a-std-parallel.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/11-std-net.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/12-std-http.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/13-std-json.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/14-std-crypto.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/15-std-testing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/16-std-reflect.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/17-std-system.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/18-std-environment.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/04-standard-library/19-standard-library-indexes.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/04-standard-library/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/01-c-abi.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/02-importing-c.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/03-exporting-zirk.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/04-dynamic-libraries.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/05-name-mangling.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/06-native-permissions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/07-intrinsics.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/08-portable-simd.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/09-auto-vectorization.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/10-why-no-inline-assembly.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/05-native-and-low-level/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/01-decorators.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/02-fn-dec.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/03-decorator-targets.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/04-syntax-api.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/05-compile-time-reflection.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/06-runtime-reflection.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/07-reflection-retention.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/08-generated-diagnostics.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/09-security-and-permissions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/06-metaprogramming/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/01-compiler-pipeline.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/02-lexer-and-parser.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/03-name-resolution.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/04-type-checker.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/07-toolchain/04a-decorator-expansion.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/04b-diagnostic-recovery.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/05-portable-ir.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/06-llvm-backend.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/07-diagnostics.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/08-cli.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/09-formatter.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/10-linter.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/11-lsp.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/12-debugger.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/13-debug-and-release.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/14-optimizations.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/15-incremental-compilation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/16-cross-compilation.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/17-abi-and-ir-compatibility.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/18-performance-goals.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/19-release-notes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/07-toolchain/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/01-unit-tests.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/02-e2e-tests.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/03-assert-and-expect.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/04-test-permissions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/05-concurrent-test-safety.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/06-test-runner.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/07-benchmarks.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/08-testing/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/01-package-anatomy.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/02-public-api.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/09-packages/03-portable-ir.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/04-add-remove-install.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/05-version-resolution.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/06-lockfile-verification.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/07-publishing.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/08-native-dependencies.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/09-package-security.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/09-packages/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/01-build-a-cli.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/02-build-a-file-processor.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/03-build-a-concurrent-worker-pool.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/04-build-an-http-service.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/05-build-and-publish-a-library.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/06-call-a-c-library.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/07-write-a-decorator.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/10-tutorials/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/01-keywords.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/02-operators-and-precedence.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/03-built-in-types.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/11-reference/04-literals.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/05-grammar-summary.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/11-reference/06-attributes-and-decorators.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/07-diagnostic-codes.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/08-cli-commands.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/09-target-matrix.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/10-permission-catalog.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/11-standard-library-index.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/11-reference/12-feature-status.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/11-reference/13-type-member-index.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/14-temporal-reference.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/11-reference/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/01-why-static-types.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/02-why-structured-concurrency.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/12-explanations/03-why-result-and-exceptions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/04-why-match-with.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/12-explanations/05-why-no-new.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/06-why-no-traditional-overloading.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/07-why-no-defer.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/08-why-no-inline-assembly.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/09-why-c-abi.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/10-why-permissions.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/11-why-portable-ir.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/12-class-or-record.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/12-explanations/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/13-appendices/01-glossary.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/13-appendices/02-language-feature-matrix.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/13-appendices/03-valid-and-invalid-examples.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/13-appendices/04-differences-from-typescript.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/13-appendices/05-differences-from-python.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/13-appendices/06-differences-from-rust.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/13-appendices/07-current-limitations.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/13-appendices/08-roadmap.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/handbook/13-appendices/09-normative-sources.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/13-appendices/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/README.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/SUMMARY.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/_editorial/editorial-guide.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/_editorial/short-chapter-review.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/_editorial/source-map.md` | No discrepancy found in the corpus/status scan and contextual review. |
| `docs/handbook/_editorial/standard-library-coverage.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/init/ZIRK_AGENT_PROMPT.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/init/ZIRK_FEATURE_STATUS.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/init/ZIRK_ROADMAP.md` | Updated; see discrepancy register for the reconciled claim. |
| `docs/security/SECURITY_CATASTRO_2026-10-24.md` | No discrepancy found in the corpus/status scan and contextual review. |

### Current OpenSpec specifications (78 files)

| Path | Audit outcome |
|---|---|
| `openspec/specs/async-runtime-core/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/background-and-services/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/canonical-formatter/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/compiler-workspace/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/concurrency-combinators/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/concurrent-scopes/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/diagnostics-format/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/editor-cli-bridge/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/editor-tooling/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/enum-static-members/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/exact-decimal-arithmetic/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/exponentiation-operator/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/handbook-editorial-system/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/language-documentation-information-architecture/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/map-collections/spec.md` | Updated; normative limitation now matches the audited admitted type/collection boundary. |
| `openspec/specs/parallel-regions/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/parallel-runtime/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/project-status-integrity/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/range-collection-expansion/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/raw-threads/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/return-type-inference/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/set-collections/spec.md` | Updated; normative limitation now matches the audited admitted type/collection boundary. |
| `openspec/specs/spread-rest-operators/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/synchronization/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/target-matrix/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/timer-operations/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/toolchain-bootstrap/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/triple-quoted-strings/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/type-constant-evaluation/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/universal-string-rendering/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-browser-compiler/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-browser-networking/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-callables/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-check/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-classes/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-cli-commands/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-collections/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-contracts/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-data-types/spec.md` | Updated; normative limitation now matches the audited admitted type/collection boundary. |
| `openspec/specs/zirk-debug-adapter/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-decorators/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-environment/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-errors/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-feature-phasing/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-filesystem-advanced/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-filesystem/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-generics/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-globals/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-grammar/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-http/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-ide-query/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-ir-interpreter/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-ir-lowering/spec.md` | Updated; normative limitation now matches the audited admitted type/collection boundary. |
| `openspec/specs/zirk-lexical-syntax/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-lsp/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-memory-safety/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-modules/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-native-codegen/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-nested-classes/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-networking/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-object-memory/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-packages/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-pattern-matching/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-permissions/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-project-model/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-regex/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-resources/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-runtime-io/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-scalar-members/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-scalars/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-standard-library/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-standard-streams/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-structured-concurrency/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-temporal-members/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-temporal-types/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-text-members/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |
| `openspec/specs/zirk-type-system/spec.md` | Updated; normative limitation now matches the audited admitted type/collection boundary. |
| `openspec/specs/zkinit-editor-recognition/spec.md` | No discrepancy found against the current behavior evidence and authority chain. |

### Standard-library source modules (22 files)

| Path | Audit outcome |
|---|---|
| `stdlib/std/environment/environment.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/environment/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/fs/fs.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/fs/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/http/http.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/http/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/io/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/io/stderr.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/io/stdin.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/io/stdout.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/json/json.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/json/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/address.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/dns.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/interfaces.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/local.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/tcp.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/tls.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/net/udp.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/path/package.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |
| `stdlib/std/path/path.zrk` | Source/package inventory checked against the module index; importability and implementation are separately stated in status documents. |

### Companion website status sources (6 files)

| Path | Audit outcome |
|---|---|
| `../zirk-lang-site/tools/project-status/catalog.ts` | Updated for the audited statuses and committed with the generated site synchronization. |
| `../zirk-lang-site/tools/project-status/generate.ts` | Reviewed; no generator edit was needed. The supported sync path regenerated the imports and the production build passed. |
| `../zirk-lang-site/tools/project-status/generate.spec.ts` | Reviewed and exercised by the site tool test suite (12 files, 82 tests passed); no spec edit was needed. |
| `../zirk-lang-site/content/project-status-source.json` | Regenerated from the committed compiler documentation and updated with the reviewed source revision and checksums. |
| `../zirk-lang-site/content/handbook-source.json` | Regenerated from the committed compiler documentation and updated with the reviewed source revision and digest. |
| `../zirk-lang-site/src/generated/project-status.generated.ts` | Regenerated from the reviewed catalog and source descriptor; committed with the site synchronization. |

## Exclusions and separate classifications

- `openspec/changes/archive/`: archived change history, 905 files including 793 Markdown files; preserved as immutable historical artifacts, not treated as current documentation.
- Active OpenSpec changes other than this audit: `openspec/changes/complete-data-types-and-standard-library-coverage/` is a separate active proposal and is intentionally excluded from this change and commit.
- Vendored manuals: `vendor/` (55 files, 2 Markdown files) are third-party source and are not edited as Zirk-owned documentation.
- Build/generated directories (`target/`, `dist/`) are not maintained source documentation. Generated website imports/content are updated only by the supported sync command.
- Dated decision/audit files and snapshots remain historical even where they discuss now-changed behavior; current handbook/status owners carry present-day claims.

## Validation results

- `node scripts/format-doc-examples.mjs --check target/debug/zirk`: passed after canonical formatting; 481 complete Zirk fences already canonical, 150 fragment/pseudocode fences skipped, 0 needing formatting.
- `target/debug/zirk check /tmp/zirk_docs_sequence_audit.zrk`: passed. The fixture covers typed List/Array map, filter, reduce, sort, endpoint/pop, and Range.map/collect examples.
- `openspec validate audit-and-align-zirk-documentation --strict`: passed. `openspec validate --specs --strict`: 78 passed, 0 failed; validator emitted advisory long-requirement warnings on pre-existing specs.
- `git diff --check`: passed.
- Source documentation commit: `a6fd71f9a147134efcda768719dd834847ec6d07`. Website synchronization completed from that exact commit with reviewed audit date `2026-10-28`; the handbook descriptor pins the revision and digest `46c28e3c49f50a12f0943f9edac4feaa5f9f02eb45ddd22deb4317303ca3cdc0`, and the status descriptor pins the same revision/date and refreshed checksums.
- Site checks used the bundled Node.js 24 runtime because `yarn` is not installed in the shell: handbook `validate` and `verify` each passed (374 documents, 374 routes); browser compiler package verification passed; tool tests passed (12 files, 82 tests); production Angular build passed and prerendered 380 routes; sitemap generation passed (374 handbook routes); shell verification passed (7 shell routes and 374 handbook routes). The build emitted existing bundle/style budget warnings for the initial chunk and two stylesheets; it exited successfully.
- Site synchronization commit: `c59b7ae7283c6cb8437b8f8cc350964bf78378f2`. Both source and site repository revisions were checked after their separate commits; the site working tree is clean.
- Final targeted scan found no stale claims that Deque/Queue/Stack/PriorityQueue, JSON, `Thread.run`, scoped `Mutex<T>.with`, or `Range<T>` are unavailable. Remaining unavailable/specified-only hits describe general thread handles or other synchronization primitives; removed declaration forms and dated status notes are explicitly labeled. All inventoried paths have an outcome in this ledger.
- `openspec validate audit-and-align-zirk-documentation --strict`: passed. `openspec validate --specs --strict`: 78 passed, 0 failed; validator emitted advisory long-requirement warnings on pre-existing specs.
- `git diff --check`: passed in both repositories.
