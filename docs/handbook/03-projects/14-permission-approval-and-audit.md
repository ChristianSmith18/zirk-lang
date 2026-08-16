# Permission Approval and Audit

Manifest declaration is not developer consent. Zirk stores consent outside the
repository in a signed OS-protected record bound to project name, absolute
canonical location, user/device, exact scopes/phases, and every requesting
dependency's version, integrity, and transitive path.

Moving or renaming a project, widening authority, adding/updating a requester,
or changing requester integrity/path/phase requires approval. Narrowing does
not. A library that edits `init.zrk` gains nothing because it cannot produce the
matching signature. Deleted or corrupt state grants nothing and triggers
reconstruction or reapproval.

## Incremental fast path

Authority-bearing commands compare signed project-location, manifest,
permission, lockfile, requester-subgraph and approval fingerprints. An exact
match continues without another prompt or full scan. A change recomputes only
affected dependency-graph segments.

## Interactive approval

The CLI displays the grant, phase, call path, requester, dependency path and
manifest diff, then offers allow once, approve this exact set, or deny. Accepted
changes use the `init.zrk` parser and formatter. Dynamic targets require a
developer-chosen pattern. An `all` grant shows a critical warning and requires
typing the project name; Enter or generic `--yes` cannot approve it.

CI uses a protected explicit policy and fails noninteractively on widening.
Deployed applications never prompt or modify their source.

```text
zirk permissions show
zirk permissions diff
zirk permissions approve
zirk permissions revoke
zirk permissions history
```

History shows project name/location, requester, scope, phase, approver and time
without secrets. Revocation applies before the next privileged execution.

---

**Previous:** [← Reproducible Builds](13-reproducible-builds.md) · **Next:** [Standard Library →](../04-standard-library/README.md)
