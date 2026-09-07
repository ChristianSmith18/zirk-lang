# Proposal: `.zkinit` file icon

## Why

Zirk project-initialization files are moving to a dedicated `.zkinit` name,
but the editor treats them as generic text: they get no Zirk identity in the
file explorer and no syntax highlighting. A dedicated icon
(`docs/assets/zirk-init.svg`, already designed with the configuration-gear
mark) makes init files visually distinguishable from regular `.zrk` sources,
the same way the `zirk` language icon distinguishes Zirk files today.

## What Changes

- The VS Code extension recognizes files named `.zkinit` and assigns them
  the `zirk-init.svg` icon (light and dark variants point at the same SVG,
  like the current `logo.svg` registration).
- `.zkinit` files are a distinct editor language surface from `.zrk`: they
  keep Zirk syntax highlighting and language configuration, but carry their
  own icon and language identity so the two file kinds never share an icon.
- No compiler, grammar, or semantic changes: `.zkinit` is an
  editor-recognition feature. Whether the manifest format itself is renamed
  from `init.zrk` is out of scope.

## Capabilities

### New Capabilities

- `zkinit-editor-recognition`: editor association of `.zkinit` files with a
  dedicated icon and Zirk-based highlighting/configuration, distinct from
  the `.zrk` file icon.

### Modified Capabilities

- `editor-tooling`: the source-file-recognition requirement gains `.zkinit`
  association and a dedicated-icon rule alongside `.zrk`/`init.zrk`.

## Impact

- `editors/vscode/package.json`: new language contribution (or equivalent
  contribution point) for `.zkinit` files with `icon` pointing to the new
  SVG; grammar/snippets reuse where applicable.
- `editors/vscode/images/zirk-init.svg`: new asset copied from
  `docs/assets/zirk-init.svg`.
- `editors/vscode/extension.js`: decide whether `zirk-init` documents get
  CLI-bridge diagnostics (init files are declarative, not compiled — likely
  out of the check/build/run scope).
- Installed extension copy at `~/.devin/extensions/` updated as usual.
- Documentation: `docs/SYNTAX_HIGHLIGHTING.md` / editor README mention the
  `.zkinit` icon if relevant.
