## Context

See `proposal.md` for motivation and the delta specifications for the required
behavior. The parser already records an optional label on each call argument;
the checker centralizes most binding in `match_arguments`; and free-function
lowering has an argument holder that restores declaration order after
source-order evaluation. Method, construction, and `super` paths instead zip
written arguments with parameter types. Constructor selection also uses an
arity-only lookup twice, once in checking and once in lowering.

The implementation must preserve the normative Zirk 1.x named-call contract
without treating source presence as evidence that the behavior is delivered.
The public reference and handbook already describe the target semantics, so
they need a delivery-status and example review once executable verification is
complete.

## Goals / Non-Goals

**Goals:**

- Establish one parameter-binding model for every ordinary callable form.
- Preserve left-to-right argument evaluation while materializing call operands
  in declaration order.
- Make constructor overload resolution deterministic, type-directed, and
  impossible for lowering to reinterpret differently.
- Cover both accepted and rejected behavior with parser, semantic, lowering,
  and executable CLI regressions.

**Non-Goals:**

- Add ordinary free-function or method overloading.
- Change record construction, which already binds named fields directly.
- Redesign the AST solely to retain shorthand provenance; parser desugaring is
  sufficient because ordinary name resolution produces the required error for
  a missing variable.
- Change default, optional, variadic, dispatch, or inheritance semantics apart
  from making them work with the same named binding.

## Decisions

### Desugar shorthand in the parser

After consuming a simple identifier label and `:`, the parser will synthesize
the corresponding path expression when the next token closes the argument or
continues it with a comma. The generated expression uses the identifier's
source span so name resolution and diagnostics remain anchored at the call
site.

This avoids an AST `shorthand` flag and duplicate semantic paths. A flag was
considered for a dedicated missing-shorthand diagnostic, but it adds state
without changing the observable acceptance rule; the existing unknown-name
diagnostic is adequate.

### Centralize argument lowering around a supplied signature

Refactor the existing held-argument lowering to accept the resolved parameter
list and the declaration source needed for defaults, rather than looking up
only a free-function name. Method, constructor, and both `super` call paths
will pass their already-resolved signatures through that helper.

The helper will lower each written expression once in source order, hold values
across later block-opening expressions, and reload them in parameter order.
It will keep current optional/default and variadic behavior. Repeating local
zip loops was rejected because it recreates the silent-miscompilation class and
does not preserve the established cross-branch holding rule.

### Reject positional-after-named during argument matching

`match_arguments` will track the first named argument, including parser-
desugared shorthand. A later non-spread positional argument will receive a
targeted diagnostic rather than filling the next available slot. This belongs
in shared matching so free functions, methods, constructors, and `super` calls
all enforce it consistently.

Parser-only rejection was rejected because the grammar deliberately keeps
labels in the AST and the checker owns callable-specific diagnostics and spread
validation.

### Select and record constructor overloads in the checker

For every arity-compatible constructor candidate, checking will bind arguments
against its parameters and retain candidates that have no binding or type
errors. Constructor declarations whose types admit the same positional call
are rejected before calls are checked; labels do not distinguish positional
signatures. Exactly one valid candidate is selected; none reports the relevant
call errors, and more than one reports ambiguity only where distinct declared
types still both accept the written argument. The checked-program metadata
will record both the class identity and selected constructor index at the call
span. Lowering will read that recorded index instead of scanning constructors
by arity.

The alternative of keeping arity selection in lowering was rejected because it
cannot account for labels or types and permits checker/codegen disagreement.

## Risks / Trade-offs

- [Candidate checking can emit diagnostics from rejected constructor overloads] → Probe candidates with recoverable diagnostic collection, retaining only
  diagnostics for the final no-match or ambiguity result.
- [Reordered lowering can accidentally change side-effect order] → Add runtime
  regressions whose arguments visibly evaluate left-to-right while received
  parameters prove declaration-order binding.
- [Block-opening argument paths can invalidate IR operands] → Reuse and extend
  the existing hold/reload helper; run IR verification-focused tests alongside
  end-to-end CLI cases.
- [Public documentation can overstate delivery] → Update status wording only
  after tests pass, then commit Zirk documentation before executing the
  companion-site synchronization workflow.

## Migration Plan

1. Implement and test the parser, checker metadata, overload selection, and
   shared lowering changes in one compatible compiler update.
2. Add negative tests for the newly enforced positional-after-named error; no
   valid documented program requires source migration.
3. Update the language reference and handbook examples to describe verified
   behavior, commit the Zirk documentation, and run the documented website
   synchronization in `../zirk-lang-site`.
4. If a regression appears, revert the implementation and documentation commit
   together; the change has no persisted data or wire-format migration.
