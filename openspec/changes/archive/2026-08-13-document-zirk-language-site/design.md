## Context

Zirk's normative documentation already describes a mature language across language, compiler, runtime, standard-library, and project-system specifications. The repository lacks a reader-oriented entry point comparable to established language documentation. The new site must remain isolated in `web/`, require no build step, work from a static file server, and avoid presenting unimplemented compiler features as currently available.

The requested visual reference is `jwt.io`: dark neutral canvas, a large rounded navigation surface, subtle translucent layers, compact pill controls, generous whitespace, editor-like panels, and restrained accent color. The implementation will interpret these characteristics rather than copy branding, layout, or assets.

## Goals / Non-Goals

**Goals:**

- Turn all normative Zirk material into a coherent learning and reference journey.
- Make core information reachable through persistent navigation, in-page outlines, and client-side search.
- Deliver a polished responsive and accessible experience with semantic HTML, CSS, and vanilla JavaScript only.
- Establish reusable visual and content rules in the root `design.md`.
- Keep status truth visible while allowing prose to describe the intended mature language.

**Non-Goals:**

- Changing language semantics or replacing normative specifications.
- Implementing compiler/runtime features, an executable playground, server-side search, analytics, authentication, or a CMS.
- Reproducing `jwt.io` branding, assets, or page structure.
- Introducing npm, a framework, a bundler, or remote font dependencies.

## Decisions

### Static multi-page architecture

The site will use hand-authored HTML pages with shared `styles.css` and `app.js`. Real pages preserve meaningful URLs, browser history, deep links, and no-JavaScript navigation. A single-page application was rejected because it adds routing and state complexity without improving a documentation-first static artifact.

### Documentation taxonomy

Content will be grouped into Learn, Language, Standard Library, Concurrency & Runtime, Tooling, and Project & Packages. A concise landing page provides orientation, while dense topics receive dedicated reference pages. This mirrors how developers move from tutorial to lookup without forcing repository file boundaries onto readers.

### Normative source and implementation status

Existing `docs/ZIRK_*_SPEC.md` files remain the semantic source of truth. Site copy may reorder and explain them, but SHALL not invent incompatible syntax. Every page will show a status banner linking “specified” concepts to the repository's actual implementation phase. This resolves the tension between documenting Zirk “as if it existed” and avoiding misleading claims.

### Original visual interpretation

The interface will use near-black backgrounds, warm translucent panels, `backdrop-filter` as progressive enhancement, one violet/cyan Zirk accent gradient, 20–28px radii, fine borders, and monospace code surfaces. Fallback colors keep content legible without blur. The visual guide in `/design.md` owns exact tokens and states.

### Progressive enhancement and accessibility

Core content and links function without JavaScript. JavaScript adds theme persistence, mobile navigation, copy buttons, search filtering, active-section tracking, and optional keyboard shortcuts. Focus visibility, skip navigation, reduced motion, semantic landmarks, contrast, and touch targets are mandatory.

### Search as a local curated index

`app.js` will load a small local JSON index containing titles, descriptions, keywords, and URLs. Filtering remains deterministic and dependency-free. Full-text indexing was rejected because the corpus is modest and a generated search engine would violate the no-build constraint.

## Risks / Trade-offs

- [Hand-authored shared chrome can drift between pages] → Keep markup intentionally small, document the pattern, and validate all navigation links in a lightweight test script or browser pass.
- [Blur effects can reduce performance or contrast] → Limit blur to a few fixed surfaces, provide opaque fallbacks, and disable nonessential effects under reduced-transparency/reduced-motion preferences where supported.
- [Aspirational docs can be confused with shipped behavior] → Use a persistent status component, an implementation-status page, and explicit “language specification” labels.
- [Static search can become stale] → Treat the index as part of each documentation task and include link/index checks in completion criteria.
- [Large content scope can produce shallow pages] → Prioritize complete topic coverage and cross-links, then enrich examples in incremental passes tracked by tasks.

## Migration Plan

1. Add the visual guide and static site structure without changing existing documentation.
2. Populate navigation and reference pages from normative sources.
3. Add progressive interactions and the curated search index.
4. Validate links, keyboard navigation, responsive layouts, reduced-motion behavior, and representative content against the specs.
5. Publish only after the implementation-status language is visible on every page. Rollback consists of removing `web/` and the root visual guide; compiler artifacts remain unaffected.

## Open Questions

- The eventual hosting base URL is intentionally unspecified; all internal links will remain relative.
- A future version may generate API pages from compiler metadata, but this static first version does not assume that tooling exists.
