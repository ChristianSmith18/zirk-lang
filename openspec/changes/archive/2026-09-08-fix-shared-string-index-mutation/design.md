## Context

`String` is a managed reference whose complete assignments share identity, and
an indexed assignment is a place mutation. The current checker instead treats
`s[i] = c` as a binding re-assignment, while lowering calls `zirk_str_set` to
allocate a replacement string and stores that new handle only in `s`'s local
slot. This makes aliases stale and incorrectly rejects a valid write through
an `inmut` reference.

The runtime stores text inline in a collector-managed object. A replacement
grapheme can change the byte length, so the original allocation cannot be
resized. The collector also requires every managed reference reachable from a
live object to appear in that object's descriptor. Finally, writes inside an
`unsafe` transaction must be recoverable on rollback.

## Goals / Non-Goals

**Goals:**

- Preserve the observable identity of a `String` handle across indexed writes.
- Make all complete aliases observe the same replacement, including aliases
  held through fields or collections.
- Permit non-strict referent mutation through `mut` and `inmut`, while
  rejecting a mutation reached from `inmut::strict`.
- Preserve Unicode grapheme bounds checks, single-grapheme replacement checks,
  garbage-collector safety, and unsafe journal rollback.

**Non-Goals:**

- Changing whole-assignment, extraction, slice, or string-method copy
  semantics.
- Adding concurrent mutation, a public mutable-buffer API, or a new source
  language feature.
- Revising handbook wording that already states the intended semantics.

## Decisions

### Keep a stable outer String handle and redirect its backing text

The runtime will keep the original `ZirkString` allocation as the stable
handle. On a length-changing indexed replacement, it will allocate a new
immutable backing String object and update a managed backing-reference field
on the stable handle. Reads and ASCII metadata will resolve the active backing
object. The String descriptor will trace that backing field, so the collector
retains it while the stable handle is reachable.

Returning a replacement handle and updating every alias is not viable: aliases
are ordinary managed references and cannot be exhaustively discovered. Growing
the inline allocation is impossible with the non-moving allocator. Storing a
raw external byte allocation would evade collector ownership and leak or
outlive its source, so it is rejected.

### Lower indexed writes as effects on the receiver handle

Lowering will evaluate and spill the receiver, index, and replacement in
source order, retain the existing bounds branch, then call `zirk_str_set` as
an effect on the stable handle. It will not write a replacement into a local
slot. This permits any checker-approved String place rather than requiring an
`Expr::Path` receiver.

The runtime entry point may continue returning the input handle for ABI
compatibility, but lowering will not use that result as an assignment value.

### Use referent mutability for checker acceptance

For a String indexed place, checker validation will distinguish a binding
reassignment from reachable referent mutation. `mut` and `inmut` roots are
accepted; a root that is `inmut::strict` emits the existing strict-alias
diagnostic. The value must remain a `Char` or a single-grapheme `String`.

This reuses the root-binding mutability model used by field writes rather than
giving String a one-off interpretation of `inmut`.

### Journal the backing-reference update

Before a String write inside any active uncommitted `unsafe` frame, lowering
will request a runtime journal snapshot of the stable String handle's backing
reference. Restoring that pointer makes rollback expose the pre-transaction
text to every alias, while the collector later reclaims an unreachable
replacement backing object. This mirrors field-write journaling and avoids a
silent rollback regression caused by removing the old slot store.

## Risks / Trade-offs

- [A backing object can be collected while a String handle remains live] → The
  String descriptor records the backing field as a strong collector edge, with
  runtime tests that force allocation pressure after a write.
- [An `unsafe` rollback leaves aliases changed] → Add a journal entrypoint and
  end-to-end rollback coverage for a shared String.
- [A stale direct metadata read bypasses redirected text] → Route all runtime
  String content and ASCII queries through the active backing accessor.
- [A control-flow branch strands operands before the runtime call] → Retain
  the existing spill/reload pattern around the bounds branch and test IR
  verification.

## Migration Plan

1. Add runtime stable-backing representation, descriptor tracing, mutation,
   and journal support with unit tests.
2. Align checker and lowering with referent mutation and emit the journal
   snapshot before the runtime effect.
3. Add semantic, IR, and CLI corpus regression coverage; run focused crates
   and the full compiler path where LLVM 20 is available.
4. Validate the OpenSpec change. No source migration is required: existing
   valid programs receive the documented alias-visible behavior.

Rollback is source-compatible: reverting the implementation restores the old
compiler behavior but would reintroduce the known semantic defect, so it is
not a compatibility-preserving fallback.

## Open Questions

None. The existing language documentation already establishes the intended
reference and strictness rules.
