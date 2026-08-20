# Decorators

Decorators inspect declarations and generate or wrap validated syntax during compilation. Their public contract publishes configuration parameters, supported targets, ordering dependencies, generated API, inferred effects, diagnostics, and build-permission requirements.

```zirk
@Authorized(Role.Admin)
@Cached(5.minutes)
fn report(): Result<Report, ReportError> {
    // ...
}
```

Expressions evaluate top-to-bottom, while the closest decorator is the innermost wrapper. The example composes as `Authorized(Cached(report))`; the compiler never silently reorders it.

Every generated declaration is resolved, typed, checked for effects and safety, and source-mapped. A decorator cannot weaken a public signature or suppress compiler errors. Its application is erased after expansion unless it explicitly generates an ordinary runtime descriptor or registry.

---

**Previous:** [← Metaprogramming](README.md) · **Next:** [ fn dec](02-fn-dec.md)
