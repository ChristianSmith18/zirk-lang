# Diagnostics

Every diagnostic carries severity, stable code, location, cause, and actionable help when available. Generated-code errors show original and expansion spans. Warnings never alter semantics and may be promoted with `--warnings-as-errors`.

```text
error[E1234]: precise description
  src/users.zrk:18:12
   |
18 |     problematic expression
   |            ^ localized explanation
   = cause: semantic reason
   = help: concrete action
```

Human output may use color only when the destination supports it. `--json`
emits deterministic structured records without ANSI escapes, including stable
code, severity, primary/related spans, cause, help, and expansion provenance.
Codes are never reused for a different meaning.

Recovery may continue after an error to find independent problems, but recovered
or error-typed nodes never reach code generation. Batch commands bound and
deduplicate output; interactive tooling publishes a smaller actionable set and
reports truncation explicitly.

Safety diagnostics identify the owner and escape path. Concurrency diagnostics
show both conflicting accesses. Decorator diagnostics connect generated code to
the declaration, application, target, and expansion path. Toolchain failures
identify the missing target component rather than collapsing into a generic
linker message.

---

**Previous:** [← LLVM Backend](06-llvm-backend.md) · **Next:** [ CLI](08-cli.md)
