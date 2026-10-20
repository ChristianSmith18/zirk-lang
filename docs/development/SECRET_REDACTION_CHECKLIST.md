# Secret Redaction Maintainer Checklist

This checklist is a review gate for every output path that can inspect or
serialize a runtime value. It applies to ordinary formatting, structured
diagnostics, execution traces, debugger views, heap snapshots, test reports,
and tool output.

## Before adding or changing an output sink

- [ ] Identify every path from a `SecretString` to the sink, including nested
  lists, maps, records, captured values, and thrown errors.
- [ ] Route rendering through the shared secret-aware formatting or
  serialization boundary. Do not read or serialize the private payload as an
  ordinary string.
- [ ] If the sink cannot preserve redaction, make it reject or omit the
  secret-bearing value before producing output.
- [ ] Add a sink-specific executable test containing a direct secret, nested
  secrets, a captured secret where closures are supported, and a
  secret-bearing error.
- [ ] Assert that the expected redaction marker is present and the original
  secret bytes are absent from the complete output, including structured
  fields and secondary diagnostic lines.
- [ ] Exercise both human-readable and structured modes when the sink supports
  both.
- [ ] Record the sink and its tests in the relevant standard-library or
  runtime documentation.

## Review gate

Do not merge a new output sink until every applicable item above has evidence
in code and tests. A declaration in a specification or handbook is not
evidence that a sink exists or is redaction-safe. If a debugger, snapshotter,
test reporter, or tool serializer is not implemented, keep its status marked
as unavailable or unverified and retain this gate for its future
implementation.
