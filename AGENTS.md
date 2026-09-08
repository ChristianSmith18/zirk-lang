# Zirk — rules for agents

## Commits

- **Never** add `Co-Authored-By`, `Generated with`, or any other trailer or
  agent/tool co-author to commit messages. Commits carry only the repo
  author's message.
- Message style: English, prefixes like `docs(scope):`, or short descriptions
  of the change (see `git log` for examples).

## Verification

- Tests: `cargo test -p <crate>` (zirk-lexer, zirk-parser, zirk-sema,
  zirk-ir, zirk-cli).
- VS Code extension: after editing `editors/vscode/*`, copy to
  `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/` and validate the
  JSON/JS.
- Docs sync to the website: `./scripts/sync-website-content.sh` (requires
  committed docs and a clean `../zirk-lang-site` repo; use `--audit-date
  YYYY-MM-DD` if project-status changed).
