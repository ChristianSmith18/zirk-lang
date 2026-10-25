# VS Code `.zkinit` highlighting — design notes

**Status:** historical discussion checkpoint; the final contract is recorded below and in the linked OpenSpec/handbook sources.

**Date:** 2026-10-25

## Goal

Make `.zkinit` read clearly as Zirk project configuration while keeping it visually consistent with the Zirk language palette. Color should distinguish configuration roles (block, key, type, value, path), not assign a different color to every project section.

## Proposed palette

| Element | Suggested color | Existing Zirk palette token |
|---|---|---|
| Section/block names such as `project`, `dependencies`, `globals`, `requires` | Violet `#8E6CFF` | Declarative keywords |
| Field keys such as `name`, `entry`, `readonly`, `path`, `license` | Warm yellow `#F7C65C` | Mutability/property accent |
| Built-in types | Teal `#32D4C6` | Built-in types |
| User-defined types | Green `#34D399` | User types/classes |
| Enum-like option values such as `application` and `package` | Orange `#F4A261` | Modules/decorators |
| Strings, versions, descriptions | Pink `#FF4FA3` | String literals |
| Numbers | Blue `#60A5FA` | Numeric literals |
| Comments | Muted gray `#A3A3AF`, italic | Comments |
| Braces/separators | Violet or neutral | Zirk punctuation |

Example role mapping:

```text
project {                               // section name: violet
    name: "acme.api";                   // key: yellow; string: pink
    type: application;                  // key: yellow; option: orange
    entry: "src/main.zrk";              // key: yellow; path string: pink
}

globals {
    settings: {
        type: Settings;                 // custom type: green
        readonly: true;                 // key: yellow; boolean: highlighted value
    }
}
```

The palette is a role guide, not a requirement to hard-code these colors into every editor theme. Preserve the active theme where possible and verify readable contrast on both dark and light backgrounds.

## Grammar and token-scope direction

`.zkinit` already has a distinct VS Code language ID (`zirk-init`) and icon, but currently reuses the general Zirk TextMate grammar. The grammar should gain configuration-aware scopes so the editor can distinguish:

- section names (`project`, `globals`, `requires`, dependency blocks);
- object keys (`name`, `entry`, `type`, `default`, `readonly`, `path`, `git`, `rev`, `registry`);
- built-in and custom type names;
- enum-like values (`application`, `package`);
- ordinary strings versus path/source strings where the grammar can identify them reliably;
- booleans, numeric literals, punctuation, and comments.

Use scopes compatible with the existing Zirk brand palette (current rules target scopes ending in `.zirk`) or deliberately extend the palette application to `.zkinit`. Keep syntax coloring theme-aware; do not use semantic symbol tokens to color configuration fields unless `.zkinit` gains real parsed/semantic support.

## Current extension state

The extension recognizes `.zkinit` as `zirk-init` and applies the general Zirk grammar, icon, and language configuration. The existing brand palette defines the suggested Zirk colors in `editors/vscode/extension.js`. The semantic-token scope mapping in `package.json` currently targets `zirk`, not configuration fields in `zirk-init`. There is no config-aware grammar or manifest semantic validation yet.

## Future implementation considerations

- Add scoped TextMate patterns for the agreed `.zkinit` grammar without changing `.zrk` highlighting.
- Keep the `zirk-init` language ID so source-file commands do not run on manifests.
- Add semantic tokens only after the parser/LSP can provide reliable field and type classifications.
- Check contrast in representative dark and light themes and with brand colors enabled/disabled.
- Add examples covering project metadata, dependencies, globals, and the unified `requires` block once their syntax is finalized.

## Finalized model

This design checkpoint is superseded by `openspec/changes/complete-zkinit-project-runtime/specs/zkinit-editor-recognition/spec.md`. `.zkinit` keeps the `zirk-init` language id and a separate TextMate grammar. The role palette is: section names violet `#8E6CFF`, keys warm yellow `#F7C65C`, built-in types teal `#32D4C6`, custom types green `#34D399`, option values orange `#F4A261`, strings pink `#FF4FA3`, numbers/targets blue `#60A5FA`, comments muted gray `#A3A3AF`, and punctuation violet. The brand-color toggle affects only Zirk and `.zkinit` scopes; active themes remain the base and the rules must remain readable in light and dark themes. LSP manifest diagnostics and version-aware actions use compiler-backed overlays; extension commands remain source-only and do not run on `.zkinit`.
