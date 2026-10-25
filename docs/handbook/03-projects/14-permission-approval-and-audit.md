# Permission approval and audit

`requires` declares possible operations. User consent is stored separately in
the current user's Zirk security directory, outside the repository. A signed
record binds exact approved resource scopes to the canonical project identity
and the locked dependency/requester graph. Editing a manifest or installing a
package cannot create consent. Moving a project, changing its identity, or
changing the locked requester graph makes the previous record stale.

## Inspecting consent

```text
zirk permissions inspect .
zirk permissions diff .
zirk permissions history .
zirk permissions revoke .
```

`inspect` shows direct and transitive requirements, requesters, dependency
paths, and currently saved exact resources. `diff` compares the requirement
tree with signed consent. `history` verifies signed approval/revocation events
and shows requester, resource, actor, and timestamp. `revoke` removes the
project's approval record; a later operation must request or receive consent
again. `zirk permissions approve` no longer creates a project-wide grant; it
reports that consent is requested for the exact resource when code reaches the
operation.

## First use

At an unapproved protected operation, an interactive terminal displays the
project, operation, capability, concrete resource, requesting package, and
dependency path, then asks for `y` or `n`. `y` persists only that exact scope
and permits the current operation. `n` stores no consent and denies that
operation. A non-interactive run denies the operation immediately and does not
hang or infer approval.

The approval record is user-owned, private to the operating-system user, and
authenticated with a local signing key. Scope changes and requester graph
changes invalidate the affected approval. The signed history is append-only
for audit purposes; failure to write a history event does not undo a grant
that was already durably saved.

---

**Previous:** [← Reproducible builds](13-reproducible-builds.md) · **Next:** [Standard library](../04-standard-library/README.md)
