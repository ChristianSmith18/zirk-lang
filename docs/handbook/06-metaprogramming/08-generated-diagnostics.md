# Generated Diagnostics

Decorators emit `target.error`, `target.warning`, and `target.note` against a concrete target or argument span. They cannot suppress compiler diagnostics.

An error from generated code identifies the generated declaration, decorator declaration and application, decorated target, complete expansion path, and actionable original span. Dependency errors name missing decorators; order errors suggest moving source; cycle errors print every edge; public conflicts identify both origins.

Stable diagnostics retain severity, code, location, cause, and repair guidance. Debug source maps connect generated wrappers to both the user declaration and decorator expansion.

---

**Previous:** [← Decorator Erasure and Generated Descriptors](07-reflection-retention.md) · **Next:** [ Security and Permissions](09-security-and-permissions.md)
