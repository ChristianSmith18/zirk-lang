# Why No Traditional Overloading?

Same-name overload sets make resolution, inference, generated code, diagnostics, and API evolution dependent on subtle candidate ranking. Zirk uses distinct names, generics, or unions so the relationship is explicit.

Contract-governed operator overloading remains narrow because arity and precedence are fixed by the language.

---

**Previous:** [← Why No `new`?](./05-why-no-new.md) · **Next:** [Why No General `defer`? →](./07-why-no-defer.md)
