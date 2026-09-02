## Why

`fase-4b-excepciones` left `Resource<E>`/`match with` explicitly out of
scope because they depended on `throw`/`try`/`catch`/`finally` actually
running, not just type-checking — they now do. `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
section 4 describes the full mechanism: `Resource<E from Error>` as a
generic contract with `close()`/`is_closed()`, grouped left-to-right
acquisition with right-to-left close, close failures preserved in
`ResourceFailure<BodyError,CloseError>` and in `suppressed` during
propagation, explicit transfer via `TransferableResource`, and an
escape/use-after-transfer analysis that prevents a resource from
outliving its managed scope.

That full mechanism is, again, too large for a single change: the
escape/transfer analysis is its own simplified borrow-checker,
`ResourceFailure` needs to combine two independent results into a
third type, and `suppressed` needs `List<T>` (Phase 7, does not exist).
This change builds the checkable and executable core — the
`Resource<E>` contract, the `match ... with` grammar, and real automatic
close on every exit path of the arm that acquires the resource
(normal completion, `return`, propagated exception, `break`,
`continue`) — wholesale reusing `fase-4b-excepciones`'s `finally`
mechanism (D3 of its own `design.md`): an acquired resource is, for
purposes of when it closes, exactly a `try { <arm> } finally { binding.close(); }`
with no `catch` of its own.

## What Changes

- `interface Resource<E from Error> { fn close(): Result<Void,E>; fn is_closed(): Boolean; }`,
  registered the same way `Iterable<T>`/`Iterator<T>` are — injected directly,
  not parsed from a user declaration — but with the `from Error` restriction
  expressed with the same `constraints` machinery a user `interface`
  already has.
- An application class adopts the contract with `implements Resource<MyError>`,
  the same syntax as any other `implements`.
- `match scrutinee with binding { ... }`: the `scrutinee` must be
  `Result<R,Err>`; `binding` must be the name that one (and only one)
  arm destructures in its own pattern (typically `Result.Ok(binding)`), and
  `R` must implement `Resource<E>` for some `E`.
- That arm closes the resource — calls `close()`, discarding its `Result` —
  on every exit path: normal completion, `return`, `break`,
  `continue`, or an exception propagating through it.
- `parse_match` was restructured: the `match with` guard that emitted
  `NOT_IMPLEMENTED` checked for `with` *before* parsing the scrutinee, in the
  wrong position for the real grammar (`match <scrutinee> with
  <binding> { ... }`, `with` comes after). Now the scrutinee is parsed
  first and `with binding` is optional after it.
- A pre-existing, unrelated gap, discovered while exercising `close():
  Result<Void,E>` for the first time in the compiler: `zirk-codegen-llvm`
  had never built the LLVM type of an enum with a `Void` field
  (`Result<Void,X>` had no prior use in the language). Fixed
  narrowly — a `Void` field occupies a zero-sized struct slot
  instead of panicking — without touching `value_struct`/`object_struct`/closures,
  which no program exercises with a `Void` field yet.

### Explicitly out of scope

- **Grouped acquisition** (`match a with x, b with y { ... }` or equivalent)
  and its right-to-left close when a later acquisition fails —
  this change covers a single acquisition per `match ... with`.
- **`ResourceFailure<BodyError,CloseError>`** — a close failure is not
  combined with the body's result; `close()`'s own `Result` is
  discarded after being called. Documented as an explicit narrowing, the same
  way `fase-4b-excepciones` narrowed `suppressed`.
- **`suppressed` populated from a close failure during exception
  propagation** — depends on `List<T>` (Phase 7) and on `ResourceFailure`, both
  out of scope here as well.
- **`TransferableResource`, `transfer()`, and the
  escape/use-after-transfer analysis** — a bespoke data-flow analysis,
  out of scope for a first cut; a resource may, today, escape its
  scope without the compiler detecting it (left as the programmer's
  responsibility, unverified).
- **Dependent resources that do not outlive their parent**, **`take` on a
  non-cloneable resource inside a container** — need collections (Phase 7).
- **Cancellation** (`docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`) — structured
  concurrency does not exist yet (Phase 5).

## Impact

- Specs affected: `zirk-resources` (implements the first of its five
  requirements, partially — without grouped acquisition), `zirk-grammar`.
- No breaking changes: nothing that compiles today stops compiling.
