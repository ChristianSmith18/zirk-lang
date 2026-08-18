# Diagnostic Recovery

Recovery lets one invocation or editor update report several independent
problems. It never means accepting malformed syntax or inventing semantics.
Each stage owns recovery appropriate to its representation and carries explicit
error state to the next safe boundary.

> **Implementation status:** continued lexing, parser synchronization, semantic
> unknown-type recovery, stable diagnostic codes, deduplication, deterministic
> rendering and a global 100-diagnostic limit exist today. Lossless recovered
> nodes, incremental/LSP publication and decorator-origin recovery are target
> behavior.

## Diagnostic shape

Every compiler diagnostic uses the common structure:

```text
severity + stable code + message
location + source snippet/label
cause
actionable help when a reliable correction exists
```

Do not introduce a competing format for pending features or generated code.
Private-development roadmap information belongs in `cause` or `help`. Warnings
never change language semantics and central policy alone can promote them with
`--warnings-as-errors`.

## Lexer recovery

After a lexical failure, the lexer consumes enough input to guarantee progress
and resumes at a safe boundary. Unterminated constructs point to their opening
and retain a recoverable invalid token/node for tooling. One malformed token
should not make every later character a second copy of the same error.

## Parser synchronization

The parser synchronizes at context-specific points such as top-level
declarations, class/contract members, statements, match arms, delimiters and
EOF. It tracks nesting and stops descending beyond 128 levels with one targeted
diagnostic.

Recovery nodes preserve the source region and expected/found context. The
semantic AST may omit an unusable child while retaining enough structure to
check independent declarations. The lossless syntax tree retains invalid input
for formatter/LSP display.

## Resolution and type recovery

An unresolved name produces one primary diagnostic and an internal error
binding/type. The checker uses the recovery value only to avoid cascades. It may
continue checking unrelated operands, branches, functions and files.

Recovery does not:

- add a declaration to real scope;
- prove a member, contract, overload or conversion exists;
- narrow nullability;
- grant a permission or suppress an effect;
- establish initialization, ownership, thread transfer or unsafe validity;
- enter portable IR.

## Module and project recovery

The loader continues following reachable imports after recoverable parse errors
so `zirk check` can report problems across the source graph. An unreadable import
is reported at its import span. Cyclic traversal loads each canonical file once.

After loading/parsing completes, an error prevents semantic stages that require
a trustworthy structure for the affected unit and prevents all IR/backend work.
Independent units may still be analyzed when doing so cannot fabricate cross-
module meaning.

## Decorator recovery

Decorator dependency/order validation occurs before executing applications. An
`Inspect` failure commits no augmentation. Compatible augmentations are
transactionally collected; invalid generated syntax or revalidation failure
does not leave half-applied public API. Wrap failures retain original and
generated origins and prevent IR.

Permission denial is an expected build-effect failure and cannot be recovered by
pretending the external input was empty. Generated recursion/limit errors stop
the affected expansion deterministically.

## Bounds, deduplication, and ordering

Batch compilation retains at most 100 diagnostics and reports how many were
suppressed. Diagnostics with the same code, message and location are emitted
once. Final ordering is deterministic by source location/stage rather than hash
or worker scheduling.

The LSP should publish at most 20 actionable diagnostics per file by default,
with explicit truncation, while the batch command retains the broader project
view. Limits protect developers and tools from cascades or hostile input; they
are not a reason to discard the earliest root cause.

## Internal compiler errors

If a verified invariant fails—such as malformed portable IR—the compiler emits
an internal-error category rather than a user syntax/type error. Safe metadata
includes compiler version/profile, target, pipeline stage and local correlation
identifier/reproduction guidance.

Zirk never automatically uploads source, environment, credentials, approvals,
paths, or machine fingerprints. Crash reporting would require a future explicit
opt-in design.

## Editor behavior

An editor commonly observes incomplete source. Incremental recovery should keep
unchanged syntax/semantic results, replace only affected diagnostics, and avoid
flicker from nondeterministic ordering. UTF-16 LSP positions derive from the same
byte-span source snapshot used by terminal diagnostics.

Code completion and navigation may use valid surrounding/recovered context, but
must label uncertain results and never present an error binding as a real public
symbol.

## Testing recovery

Every stage needs tests for multiple independent errors, one-error-no-cascade,
forward progress, EOF/unclosed delimiters, deep nesting, Unicode locations,
deterministic order, limits/truncation, JSON/human parity, and the hard no-codegen
boundary. Snapshot tests should preserve stable codes while allowing carefully
reviewed prose improvement.

---

**Previous:** [← Decorator Expansion](04a-decorator-expansion.md) · **Next:** [Portable IR →](05-portable-ir.md)
