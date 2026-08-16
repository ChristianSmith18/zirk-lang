# CLI

The official CLI covers `new`, `init`, `run`, `build`, `check`, `test`, `bench`,
`format`, `lint`, `prepare`, dependency commands, packaging, publishing, docs,
and `permissions show|diff|approve|revoke|history`. Output is deterministic and
`--json` supports automation.

Authority-bearing commands perform an incremental signed fingerprint check
before executing build/runtime code. Interactive approval shows exact scope,
phase, call path, requester and manifest diff. CI never prompts; broad `all`
grants require reinforced project-name confirmation and ignore generic `--yes`.

---

**Previous:** [← Diagnostics](07-diagnostics.md) · **Next:** [ Formatter](09-formatter.md)
