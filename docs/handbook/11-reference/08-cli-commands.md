# CLI Commands

| Command | Purpose |
| --- | --- |
| `new`, `init` | Create or initialize a project |
| `run`, `build`, `check` | Execute, produce artifacts, or analyze only |
| `test`, `bench` | Run correctness or performance suites |
| `format`, `lint` | Canonical formatting and static analysis |
| `prepare` | Audit permissions, targets, and publication |
| `add`, `remove`, `install`, `update` | Manage and lock dependencies |
| `package`, `publish`, `doc` | Produce packages, publish, and generate docs |
| `permissions show`, `diff`, `approve`, `revoke`, `history` | Inspect and manage signed external consent |
| `cache show`, `clean` | Inspect or remove reusable artifacts without affecting correctness |

Commands provide deterministic output and `--json` where automation requires
it. `--dry-run` resolves and displays a plan without executing build/runtime
code. `check` stops before LLVM. CI never prompts, and `build` never treats a
manifest edit as permission consent.

---

**Previous:** [← Diagnostic Codes](07-diagnostic-codes.md) · **Next:** [ Target Matrix](09-target-matrix.md)
