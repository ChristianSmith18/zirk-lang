# Glossary

- **Capability:** finite permission declared for runtime or compile-time effects.
- **Portable IR:** typed target-independent package implementation.
- **Resource:** external handle with typed acquisition and exactly-once close.
- **Structured concurrency:** child work bounded by a parent scope.
- **Value class:** distinct value abstraction without observable identity.
- **Normative:** required by the final specification rather than historical discussion.
- **Implementation status:** evidence-based availability in a repository revision.
- **Compiler primitive:** compiler-recognized closed type whose public behavior still follows contracts.
- **Native reference type:** built-in shared-reference type such as `String` or `List<T>`.
- **Value semantics:** assignment yields an independent logical value and identity is unobservable.
- **Reference semantics:** assignment shares an identity-bearing referent until `clone()` explicitly separates it.
- **Strict alias:** an `inmut::strict` view that forbids both mutation and creation/coexistence of a mutable alias.
- **Grapheme:** one user-perceived Unicode text element; the unit represented by `Char` and used by String indexing.
- **Contextual conversion:** an explicit outer constructor, such as `Float(...)`, that supplies a conversion context to a compatible contained operator tree.
- **Controlled error:** specified failure that cannot become undefined behavior or silent corruption.
- **Instant:** an absolute timeline position independent of presentation zone.
- **Duration:** signed exact elapsed nanoseconds; unlike a `Period`, it has context-free magnitude and ordering.
- **Period:** calendar quantity in years, months, weeks, and days whose exact elapsed length needs an anchor and calendar/zone context.

---

**Previous:** [← Appendices](README.md) · **Next:** [ Language Feature Matrix](02-language-feature-matrix.md)
