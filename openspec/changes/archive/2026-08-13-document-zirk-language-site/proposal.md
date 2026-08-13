## Why

Zirk has detailed normative specifications, but they are distributed across repository documents and read like internal engineering references rather than a cohesive language manual. A first-class documentation site is needed so developers can discover, learn, and reference Zirk as a complete language with the clarity and depth expected from Python or TypeScript documentation.

## What Changes

- Add a static, dependency-free documentation site under `web/`, implemented with semantic HTML, modern CSS, and vanilla JavaScript.
- Organize the complete Zirk language surface into guided learning, language reference, standard library, tooling, concurrency, safety, and project-system documentation.
- Provide reference-quality examples, navigation, search/filter affordances, responsive behavior, theme support, and accessible interactions.
- Establish a visual system inspired by the layered dark surfaces, rounded panels, restrained blur, compact navigation, and technical-editor character of `jwt.io`, while retaining an original Zirk identity.
- Add a root-level `design.md` that defines the site's visual language, tokens, components, content patterns, responsive rules, accessibility requirements, and interaction behavior; this file is separate from the OpenSpec change design.
- Present the language documentation aspirationally as a coherent product while displaying a persistent, unambiguous implementation-status notice so readers do not mistake normative syntax for currently shipped compiler support.

## Capabilities

### New Capabilities

- `language-documentation-information-architecture`: Defines complete, navigable coverage of Zirk's language, standard library, concurrency/runtime, tooling, packages, and project configuration.
- `documentation-site-experience`: Defines the static documentation site's navigation, search, theme, responsive, accessibility, and progressive-enhancement behavior.
- `documentation-visual-system`: Defines the original Zirk visual language derived from the requested dark translucent-panel reference and codified in the root-level style guide.

### Modified Capabilities

None. This change documents existing normative behavior and introduces a documentation experience without changing language semantics.

## Impact

- Adds a new root-level `web/` directory containing only static web assets.
- Adds a root-level `design.md` for the documentation visual system.
- Adds OpenSpec planning artifacts under `openspec/changes/document-zirk-language-site/`.
- Uses no runtime dependencies, framework, package manager, build system, or external service.
- Does not modify compiler crates, runtime behavior, the normative language specifications, or the other agent's `feature/project-workflow` branch.
