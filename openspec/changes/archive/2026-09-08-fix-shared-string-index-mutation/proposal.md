## Why

Whole `String` assignment is specified to share a reference, but indexed
assignment currently creates a replacement handle and stores it only in the
written binding. As a result, aliases observe different text after a valid
place mutation, contradicting Zirk's reference and `inmut` semantics.

## What Changes

- Make indexed `String` assignment mutate the shared string referent rather
  than rebind one local slot.
- Accept indexed writes through a non-strict `String` reference, including an
  `inmut` binding, while preserving the rejection for
  `inmut::strict` reachable mutation.
- Preserve grapheme validation, bounds failures, Unicode replacement behavior,
  and garbage-collector reachability when a replacement changes byte length.
- Add checker, IR, runtime, and end-to-end coverage for alias-visible string
  mutation and strict rejection.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-standard-library`: String indexed-write semantics must preserve shared
  reference identity and follow the binding mutability model.
- `zirk-ir-lowering`: indexed String writes must lower as a referent mutation,
  not as an unsupported write or a local-slot replacement.

## Impact

The change affects the semantic checker, IR lowering, the runtime String
representation and collector descriptor, and compiler corpus tests. It
restores an existing published language guarantee rather than changing public
syntax or normative semantics; no handbook, status, roadmap, or companion
website content change is expected unless implementation work reveals a
published statement that is inaccurate.
