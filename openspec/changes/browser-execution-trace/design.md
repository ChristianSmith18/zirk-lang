## Context

ADR-020 grants the interpreter exactly two host interactions: bounded `stdout` output and cooperative cancellation. The browser contract (`apiVersion: 1`) returns `output` events, a `completion` or `failure`, and `diagnostics`; its `Request` type denies unknown fields, so a new request option is an additive, versioned change rather than a free-form extension. Object and field names are needed to render events; `ObjectLayout` already carries the class `name` and each `ObjectField` carries its `name` and type, so the interpreter has what it needs to render events; a task still verifies this end to end.

## Goals / Non-Goals

**Goals:**
- Give consumers a deterministic, bounded, read-only record of object-level events that interleaves with output.
- Keep the program unable to observe, influence, or depend on the trace.
- Preserve `apiVersion: 1` and make the field absent by default.

**Non-Goals:**
- Letting programs cause host effects, taking input, or calling the DOM.
- Single-stepping, breakpoints, or live (streamed) tracing; the trace is returned with the run result.
- A full heap snapshot format. Events are sufficient to reconstruct state; snapshots can be a later change if events prove inadequate.

## Decisions

1. **Events, not snapshots.** The trace records `alloc`, `field-write`, `call`, `return`, and `throw` with a shared monotonically increasing sequence number that also stamps output events. A consumer can replay them to animate state or to reconstruct any intermediate object graph. Alternative considered: periodic snapshots — rejected because they are large, lossy about ordering, and cost heap-sized serialization per step.
2. **Opt-in through the request.** A new optional request field enables tracing and selects event kinds. Default runs do no extra work and carry no `trace` field. Alternative considered: always-on tracing — rejected because it adds cost and payload for every playground run.
3. **Stable per-run object ids.** Ids are assigned in allocation order starting at 1 and never reused within a run; they are unrelated to arena identifiers. This keeps traces reproducible across runs and across engine internals, and prevents leaking heap layout.
4. **Rendered values are bounded and typed.** A field write carries the field's declared type and a rendered string capped at a configurable byte length, with a truncation flag. Object-typed values render as a reference to the target's object id rather than recursing.
5. **Separate budget from program limits.** New limits `traceEvents` and `traceBytes` live beside existing limits, default small, and can only be tightened by a caller. Exhausting the trace budget stops recording and sets a truncation marker; it does not fail the program, because the trace is observation and must not change program behavior. Alternative considered: failing the run on trace exhaustion — rejected because it would make traced and untraced runs behave differently.
6. **Write-only from the program's view.** The interpreter has no instruction that reads the trace, and tracing adds no new extern or intrinsic. A conformance test asserts a traced and an untraced run of every fixture produce identical output, completion and failure.
7. **ADR first.** ADR-022 records that tracing is an observation channel inside ADR-020's authority model, so the support matrix and ADR text stay consistent.

## Risks / Trade-offs

- [Tracing perturbs semantics through cost] → Trace cost is not charged to the program's instruction budget; a test proves identical instruction-limit boundaries traced and untraced.
- [Payload size grows with long loops] → Hard event and byte maxima with explicit truncation; default caps sized for lesson-scale programs.
- [Layout names do not survive some lowering path, such as inherited or generic classes] → A first task verifies names end to end and fixes the gap without changing native codegen.
- [Contract drift for existing consumers] → The field is optional and absent by default; contract and declaration tests assert existing responses are byte-identical when tracing is off.
- [Consumers expect a game API] → Documented as a non-goal with the reason; it needs its own ADR because it extends host authority.
