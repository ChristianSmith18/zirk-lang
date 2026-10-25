# Application globals

An application may declare named, typed global values in `.zkinit`. Packages
cannot declare application globals. Keys are lowercase-friendly and do not
need to use uppercase.

```text
globals {
    retry_limit: UInt32;
    settings: {
        type: Settings;
        from: "src/config.zrk";
    }
    cached_profile: {
        type: Profile;
        default: Profile(name: "guest");
        readonly: true;
    }
}
```

The shorthand stores a value of the declared type's default. Expanded entries
require `type`; `default`, `from`, and `readonly` are optional. `from` points to
the source file that declares a custom type, relative to the project root.
Without `from`, normal type and package resolution applies. If no explicit
default is supplied, built-in types use their normal default. A custom object
uses the selected constructor's parameter defaults; a parameter without an
explicit default receives its type default. For example, a `Person(name:
String)` can initialize with an empty name. The compiler diagnoses unresolved
types or defaults it cannot derive.

`readonly` defaults to `false`. When it is `true`, `Global.get` remains
available, while `Global.set` is rejected. Values are replaced as snapshots;
mutating a retrieved copy does not change the stored global. Setting an equal
value is a no-op and does not publish a change.

```zirk
inmut profile = Global.get("settings"); // inferred as Settings
Global.set("retry_limit", 4);           // checked against UInt32

mut changes = Global.watch("retry_limit");
mut snapshot = changes.snapshot();
match changes.next() {
    Ok(change) => {
        // change.previous, change.current, change.revision
    }
    Error(_) => { /* closed or lagged subscription */ }
}
```

The compiler infers the type from the literal key in `.zkinit`; callers cannot
override it with an explicit generic argument or pass a value of another type.
Unknown or dynamic keys are rejected where static key inference is required.
`watch` captures an atomic initial snapshot and does not emit an initial
change. Later events contain the previous and current values and a monotonically
increasing revision. Equal replacements emit nothing. Subscriptions have
bounded delivery, can be closed explicitly, and are cleaned up with their
owning scope; a slow observer receives an explicit lag error and can resume
from a fresh snapshot.

Global initialization occurs before imported module bodies and the application
entry. A constructor failure prevents those bodies from running and produces
a compiler/runtime diagnostic.

---

**Previous:** [← Entry point](05-entry-point.md) · **Next:** [Requirements](07-runtime-permissions.md)
