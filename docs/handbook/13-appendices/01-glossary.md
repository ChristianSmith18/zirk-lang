# Glossary

- **Capability:** finite scoped authority requested by a library and granted by
  an application for build, runtime, or both.
- **Approval fingerprint:** signed external consent for one project name,
  canonical location, exact permission set, phases, and requesting dependency
  graph; manifest source alone never grants authority.
- **Runtime error:** typed implicit exception from a safe runtime check; it is
  catchable without mandatory `throws` declaration.
- **Throwable:** deeply immutable identity-bearing recoverable exception with
  stable code/message, cause, suppressed failures, and lazy stack trace.
- **Resource responsibility:** the compiler-tracked obligation to close,
  re-manage, or explicitly transfer one external handle.
- **Portable IR:** typed target-independent package implementation.
- **Resource:** external handle with typed acquisition and exactly-once close.
- **Structured concurrency:** child work bounded by a parent scope.
- **Task settlement:** ordered record of a task as `Fulfilled`, `Rejected`, or
  `Cancelled`; a returned `Result.Error` is still fulfilled.
- **Selection:** fair waiting for one ready task, channel operation, timer, or
  cancellation signal while leaving losing operations alive.
- **Transfer:** compiler-derived permission to move a value or responsibility
  into another concurrent execution context without retaining a mutable alias.
- **Share:** compiler-derived permission for concurrent contexts to use the same
  referent safely, normally through strict immutability or synchronization.
- **Weak reference:** non-owning `Weak<T>` observation that upgrades through
  `T?` and does not keep its referent alive.
- **Dependent reference:** view or handle whose safe lifetime is bounded by an
  owner and checked internally without public lifetime syntax.
- **Unsafe transaction:** an unsafe block whose managed and validated-range
  writes commit together or roll back on controlled pre-commit failure.
- **Commit boundary:** explicit region that publishes reversible writes before
  an external, volatile, native, or otherwise irreversible effect.
- **Value class:** distinct value abstraction without observable identity.
- **Normative:** required by the final specification rather than historical discussion.
- **Implementation status:** evidence-based availability in a repository revision.
- **Compiler primitive:** compiler-recognized closed type whose public behavior still follows contracts.
- **Native reference type:** built-in shared-reference type such as `String` or `List<T>`.
- **Value semantics:** assignment yields an independent logical value and identity is unobservable.
- **Reference semantics:** assignment shares an identity-bearing referent until `clone()` explicitly separates it.
- **Strict alias:** an `inmut::strict` view that forbids both mutation and creation/coexistence of a mutable alias.
- **Callable type:** `Function(P...) => R`, conventionally `Fn(P...) => R`, the
  signature contract shared by compatible functions, lambdas, methods, and
  explicitly callable objects.
- **Place:** an expression that denotes writable original storage, such as an
  attribute or index path on the left of assignment.
- **Projection:** a read inside a composite value. Reference-backed projections
  are deep independent copies and require `Clone`.
- **Whole-reference alias:** another binding to the same referent, created only
  by assigning, passing, returning, or capturing the complete variable.
- **Requirement class:** a state-free `abstract class` adopted through
  `implements`; unlike a concrete base, it contributes no layout or body.
- **Iteration step:** `Iteration<T>.Item(T)` or `Iteration<T>.Done`, which keeps
  completion distinct from nullable values and declared failures.
- **Grapheme:** one user-perceived Unicode text element; the unit represented by `Char` and used by String indexing.
- **Contextual conversion:** an explicit outer constructor, such as `Float(...)`, that supplies a conversion context to a compatible contained operator tree.
- **Controlled error:** specified failure that cannot become undefined behavior or silent corruption.
- **Instant:** an absolute timeline position independent of presentation zone.
- **Duration:** signed exact elapsed nanoseconds; unlike a `Period`, it has context-free magnitude and ordering.
- **Period:** calendar quantity in years, months, weeks, and days whose exact elapsed length needs an anchor and calendar/zone context.

---

**Previous:** [← Appendices](README.md) · **Next:** [ Language Feature Matrix](02-language-feature-matrix.md)
