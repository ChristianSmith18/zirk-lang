# Attributes and Decorators

Normative built-in test attributes include `@test`, `@e2e`, and `@bench` in their required file contexts. User decorators are declared with `fn dec`, typed configuration, and one or more target blocks.

Decorators operate through validated Syntax API builders, may request reflection retention, and require explicit compile permissions for external effects. Applying one to an unsupported target is a compile-time error.

---

**Previous:** [← Grammar Summary](./05-grammar-summary.md) · **Next:** [Diagnostic Codes →](./07-diagnostic-codes.md)
