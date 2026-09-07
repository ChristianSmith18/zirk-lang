# Design: `.zkinit` file icon

## Context

The VS Code extension currently registers a single language, `zirk`, with
`extensions: [".zrk"]`, `filenames: ["init.zrk"]`, and a shared `icon`
pointing to `images/logo.svg`. The new asset `docs/assets/zirk-init.svg`
(Zirk mark + configuration gear) is designed for init/configuration files.
VS Code file icons are assigned **per language**, not per filename — a
single language cannot carry two different icons, so the `.zkinit` surface
needs its own language contribution.

## Goals / Non-Goals

**Goals:**

- Files named `.zkinit` get the `zirk-init.svg` icon in the explorer and
  editor tabs.
- `.zkinit` files keep Zirk syntax highlighting and language configuration
  (comment delimiters, bracket pairs) — they are Zirk-flavored files.
- Zero impact on `.zrk` files, which keep `logo.svg`.

**Non-Goals:**

- Renaming or redefining the manifest format (`init.zrk` vs `.zkinit` is a
  language decision, out of scope here).
- CLI-bridge diagnostics, build, or run commands for `.zkinit` files (the
  init file is declarative configuration, not compilable code).
- A VS Code `iconThemes` contribution (full icon theme) — overkill for one
  file kind.

## Decisions

### D1 — Separate language contribution `zirk-init`

Register a second language in `contributes.languages`:

```json
{
  "id": "zirk-init",
  "aliases": ["Zirk Init", "zirk-init"],
  "filenames": [".zkinit"],
  "configuration": "./language-configuration.json",
  "icon": { "light": "./images/zirk-init-gear.svg", "dark": "./images/zirk-init-gear.svg" }
}
```

`filenames` (exact match) is used because `.zkinit` is the whole file name,
not an extension of a longer name (`foo.zkinit` would also match via
`extensions`; decide at implementation whether to include
`extensions: [".zkinit"]` too — including it is harmless and covers both
usages).

*Alternative considered:* a `contributes.iconThemes` file-icon theme.
Rejected — it would need to override the user's chosen icon theme and adds
packaging complexity for a single icon.

### D2 — Reuse the Zirk grammar under the `zirk-init` language

Add a second `contributes.grammars` entry mapping language `zirk-init` to
the same `syntaxes/zirk.tmLanguage.json` (`source.zirk`). Init files get
full Zirk highlighting without duplicating the grammar. Snippets can also
be scoped to `zirk-init` if useful; initially reuse the same snippet file.

*Alternative:* a dedicated `source.zirk.init` scope. Rejected for now — the
manifest DSL has no separate grammar yet, and duplicating scopes would
require doubling every palette rule.

### D3 — Extension logic treats `zirk-init` as non-code

`extension.js` gates diagnostics, build/run commands, and the provisional
formatter behind `document.languageId === 'zirk'`. `zirk-init` documents
must NOT trigger `zirk-check` (the CLI does not compile init files), so no
code change is required — the existing `zirk` gate already excludes them.
The `zirk.brandColors` palette already applies because all emitted scopes
end in `.zirk`.

### D4 — Asset placement

Copy `docs/assets/zirk-init.svg` to `editors/vscode/images/zirk-init-gear.svg`
(single source of truth remains `docs/assets/`; the extension bundles its
own copy for packaging). Update the installed extension copy under
`~/.devin/extensions/christiansmith.zirk-lang-*/` with the new
`package.json`, grammar mapping, and icon.

## Risks / Trade-offs

- [VS Code may not show per-language file icons unless no icon theme
  overrides it] → document that icon themes take precedence; the default
  (None/seti-less) themes fall back to language icons.
- [`.zkinit` vs `init.zrk` naming divergence] → the proposal explicitly
  leaves manifest naming to a separate change; this feature only adds
  editor recognition for the `.zkinit` name.
- [Two language ids for "Zirk" content could confuse features keyed on
  `languageId`] → mitigated by D3: the only gate is `=== 'zirk'`, and
  init files are intentionally excluded from compilation commands.

## Migration Plan

1. Copy `docs/assets/zirk-init.svg` → `editors/vscode/images/zirk-init-gear.svg`.
2. Add the `zirk-init` language + grammar (+ snippets) contributions in
   `editors/vscode/package.json`.
3. Copy updated `package.json` and icon to the installed extension dir.
4. Reload VS Code and verify: a `.zkinit` file shows the gear icon and
   Zirk highlighting; `.zrk` files unchanged.

Rollback: revert the `package.json` entries and remove the bundled icon.
