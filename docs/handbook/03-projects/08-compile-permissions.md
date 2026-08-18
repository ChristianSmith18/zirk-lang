# Build and Runtime Permission Phases

Build and runtime authority share the application `permissions` block but stay
semantically separate through `during`:

```zirk
permissions {
    filesystem {
        read: { paths: ["./schemas/**"]; during: build; }
    }
}
```

The public Syntax API grants no external authority. Build code, including
decorators, can access only approved build-phase operations; a runtime grant
does not imply them. The unified syntax removes duplication without confusing
developer/CI authority with deployed application authority.

Decorator implementation/version, arguments, typed target, grants and every
observed external value participate in the expansion fingerprint. Changed
requesters or observations invalidate affected expansion and may require fresh
signed approval. Secrets cannot enter generated API, IR, diagnostics or cache
keys.

---

**Previous:** [← Application Permissions](07-runtime-permissions.md) · **Next:** [Library Requirements →](09-library-requirements.md)
