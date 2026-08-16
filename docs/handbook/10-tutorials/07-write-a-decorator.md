# Write a Decorator

Declare `fn dec` with typed configuration and explicit class or method target blocks. Inspect only the public Syntax API, build typed syntax, and emit diagnostics attached to the user's source span.

Return generated code through parse, resolution, type, and safety validation.
Request structural reflection retention only when runtime behavior needs it,
and declare the narrowest library `requires` with `during: build`; the consuming
application grants it through `permissions`. Test valid expansion, invalid
target, malformed configuration, generated diagnostics, and absence of
undeclared external access.

---

**Previous:** [← Call a C Library](06-call-a-c-library.md) · **Next:** [ Reference](../11-reference/README.md)
