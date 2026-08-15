## Context

The authorial handbook review is the newest source of language intent, while the current consolidated specifications and Phase 3 change were written from an older snapshot. The correction spans lexical syntax, parsing, type checking, objects, collections, iteration, matching, module resolution, and standard-library ergonomics. The handbook has already been corrected and committed separately on top of current `develop`.

## Goals / Non-Goals

**Goals:**

- Make the canonical Markdown specifications, main OpenSpec capabilities, and active Phase 3 artifacts agree with every authorial annotation.
- State enough syntax and semantics that later compiler phases can implement each decision without recovering intent from handbook prose.
- Keep specified behavior distinct from current implementation status.
- Preserve the documentation-only branch delta.

**Non-Goals:**

- Implement lexer, parser, type checker, IR, runtime, standard library, formatter, or editor support.
- Reintroduce the removed documentation website commit.
- Resolve unrelated open questions in Phase 3 or memory management.

## Decisions

### Treat the annotations as a new authority checkpoint

The 37 annotations are accepted decisions, including items that contradict an older final specification. The consolidated spec receives a dated checkpoint and the specialized specs receive the detailed contract. Retaining the older text as higher authority was rejected because it would knowingly make the public handbook wrong.

### Keep syntax families internally consistent

Match alternatives use commas and one `=>`; enum payloads use constructor-like patterns; nested destructuring composes inside those patterns. Ranges use `..`/`..=`, direction follows the bounds, `.step(n)` supplies positive distance, and slicing remains `[start:end:step]`. Regex uses `re'pattern'` as a typed literal and pattern.

### Make defaults ergonomic but observable

Class fields default to `public mut`, constructors alone may form signature-based sets, and named arguments participate in constructor selection. Ordinary functions remain non-overloaded. Optional parameters remain typed and trail required positional parameters; variadics expose a read-only iterable for the call duration.

### Separate safe customization from native implementation

User types implement reserved operator methods such as `_add` and `_subtract` in safe code. Application code cannot reopen native types. Unsafe remains required only for operations that cross memory or ABI guarantees, not for ordinary domain operators.

### Treat callable extraction as a bound-value operation

`clone(receiver.method)` creates a local callable retaining receiver and full callable contract without cloning the receiver or native resource. Capture collisions use `this.name`; ordinary local shadowing remains forbidden. A later specification may add writable function-type annotations without changing this local inferred behavior.

### Limit direct import convenience to the standard library

Importing a standard-library object may expose its declared convenience members directly when unambiguous. Qualification through the imported object resolves collisions. Package and local objects do not inject methods into file scope.

## Risks / Trade-offs

- **The compiler currently rejects several newly specified forms** → Mark this as target behavior and create follow-up implementation tasks in the owning phases.
- **`this` now has capture and instance roles** → Require lexical disambiguation and explicit diagnostics when both interpretations remain possible.
- **Constructor signature sets resemble overloading** → Restrict the rule to `construct`; keep ordinary functions and methods non-overloaded.
- **Direct stdlib convenience names can surprise readers** → Limit them to compiler-known standard APIs and require qualification on ambiguity.
- **Mapped enum values constrain ABI and serialization expectations** → Specify them as observable source values, not ordinal layout promises.

## Migration Plan

1. Update main OpenSpec deltas and the active Phase 3 planning artifacts.
2. Update consolidated language, specialized language, standard-library, and historical checkpoint documents.
3. Audit handbook/spec examples for exact syntax agreement.
4. Validate all OpenSpec artifacts strictly and commit the specification update separately from the handbook.

Rollback reverts the specification commit without removing the already archived handbook change; a correction would then require an explicit follow-up because the handbook records accepted authorial intent.

## Open Questions

None for this documentation/specification alignment. Implementation scheduling remains owned by the relevant compiler phases.
