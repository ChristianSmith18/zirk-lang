# CLI

The official CLI covers `new`, `init`, `run`, `build`, `check`, `test`, `bench`,
`format`, `lint`, `prepare`, dependency commands, packaging, publishing, docs,
and `permissions show|diff|approve|revoke|history`. Output is deterministic and
`--json` supports automation.

Authority-bearing commands perform an incremental signed fingerprint check
before executing build/runtime code. Interactive approval shows exact scope,
phase, call path, requester and manifest diff. CI never prompts; broad `all`
grants require reinforced project-name confirmation and ignore generic `--yes`.

## Everyday workflow

```bash
zirk check
zirk test
zirk build --target aarch64-macos
zirk run -- arg1 arg2
```

`check` stops after frontend validation and must not require LLVM, a linker, or
the runtime archive. `build` writes artifacts without executing them. `run`
builds incrementally and then starts the application; arguments after `--`
belong to the program. `--dry-run` prints the resolved plan and authority checks
without executing build scripts, native tools, or application code.

Common operational flags include `--json`, `--verbose`, `--target`, profile
selection, `--warnings-as-errors`, and cache inspection/cleaning. Structured
output and exit codes are stable enough for editors and CI; prompts are never
written in JSON or noninteractive mode.

Dependency and publishing commands update `init.zrk` and `zirk.lock` only
through reviewable deterministic changes. Permission commands inspect or revoke
the signed external approval record; editing the manifest alone never grants
authority.

> **Implementation status:** `build` and `run` exist for the current compiler
> subset. The complete command surface is the Zirk 1.x target and each command
> must advertise partial availability rather than silently omitting stages.

---

**Previous:** [← Diagnostics](07-diagnostics.md) · **Next:** [ Formatter](09-formatter.md)
