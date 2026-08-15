# Static Members

Static members belong to the class rather than to one instance. They suit named factories, constants, and behavior whose contract does not need `this`.

Static mutable state is not a substitute for application globals and remains subject to concurrency safety. Prefer explicit dependencies over hidden process-wide state.

Call static members through the type so ownership stays visible at the call site.

---

**Previous:** [← Abstract Classes](./08-abstract-classes.md) · **Next:** [Object Identity →](./10-object-identity.md)
