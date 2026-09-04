# Rules for Devin in this project

## Restricted permissions

**Do not sync, archive, or commit changes to `zirk-lang` or `zirk-lang-site` without the user's explicit permission.**

Before performing any of the following actions, I must ask and wait for clear confirmation:

- `git commit` (in any repository)
- `git push`
- `openspec sync-specs`
- `openspec archive`
- `./scripts/sync-website-content.sh`
- Any other command that modifies the repository or OpenSpec state in a way that is not reversible with `git checkout`.

If the user says "sync", "archive", or "commit" without specifying what or when, I must confirm the exact scope before acting.

## Build environment

This project needs `LLVM 20.1` via Homebrew (`llvm@20`). Before any command that builds or tests `zirk-codegen-llvm` or the full workspace, set:

```
export LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20
```

If `cargo build` fails with `llvm-config not found`, run the command again with this variable.
