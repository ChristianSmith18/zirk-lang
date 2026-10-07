# ADR-022: Browser execution trace is an observation channel

## Status

Accepted

## Context

ADR-020 grants browser-interpreted code exactly two host interactions: bounded
`stdout`/`stderr` output and cooperative cancellation. A browser-hosted
learning surface needs to show what object-oriented code does (an object being
created, a field changing, a method running on one instance) and had no source
for that other than text the program printed itself.

## Decision

The interpreter may record an opt-in, ordered trace of read-only events:
object allocation, field write, call, return, throw, and markers for accepted
output events. The browser `run` operation exposes it through an additive
request option (`trace`) and an additive response field (`trace`), keeping
`apiVersion: 1` and leaving the field absent unless requested.

The trace is an output-direction observation. It adds no input, no
filesystem, network, clock, random or DOM authority, has no instruction,
extern or intrinsic that lets interpreted code read or influence it, and does
not count against the program's instruction budget. Object ids are assigned per
run in first-seen order and never derive from heap identifiers. Event, byte and
rendered-value bounds can only be tightened by a caller; exhausting them stops
recording and marks the trace truncated, but never fails the program.

A host API that lets a program *cause* effects (for example moving a game
character) is a different decision: it extends host authority and needs its own
ADR.

## Consequences

- A traced and an untraced run of the same program produce identical output,
  completion and failure; a test enforces this over every conformance fixture.
- Object-valued fields render as a reference to the target's object id, never
  as contents, so a trace cannot grow with the size of the heap.
- The support matrix and package manifest describe tracing as `observation`,
  not as a host effect.
