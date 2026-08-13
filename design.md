# Zirk Documentation — Visual Design Language

This guide defines the visual and interaction language for the Zirk documentation website in `web/`. It is intentionally separate from OpenSpec design artifacts: this file is the durable source of truth for product-facing styles.

## 1. Direction

Zirk documentation should feel like a precise native tool rendered as a calm technical workspace: dark layered surfaces, crisp typography, rounded editor panels, restrained translucency, and small moments of spectral color. The requested reference is the current `jwt.io` experience—especially its near-black canvas, broad rounded navigation shell, quiet borders, pill controls, generous vertical space, and code-editor framing.

The result must remain recognizably Zirk. Do not copy the JWT mark, Auth0 branding, proprietary assets, wording, or exact composition. Zirk's identity is a focused “compiler glow”: violet moving toward cyan, used sparingly around syntax, focus, status, and the central mark.

Principles:

1. **Content is the interface.** Decoration creates hierarchy but never competes with language reference material.
2. **Dense information, calm surfaces.** Reference pages may be deep; their chrome must remain quiet and predictable.
3. **Glass is an accent, not a substrate.** Blur belongs to navigation, floating search, and selected overlays—not every card.
4. **Native precision.** Fine borders, monospace metadata, aligned baselines, deterministic states, and immediate feedback evoke tooling.
5. **Accessible by construction.** Contrast, focus, motion preferences, semantic structure, and touch targets are part of the style.

## 2. Foundations

### Color tokens

Use semantic custom properties. Values below are the canonical dark theme.

```css
:root {
  color-scheme: dark;
  --canvas: #0b0b0e;
  --canvas-raised: #101014;
  --surface-1: rgba(28, 28, 34, 0.82);
  --surface-2: rgba(36, 36, 44, 0.72);
  --surface-solid: #1b1b21;
  --code-surface: #0e0f14;
  --text-strong: #f5f4f7;
  --text: #d1cfd6;
  --text-muted: #9996a1;
  --border: rgba(255, 255, 255, 0.09);
  --border-strong: rgba(255, 255, 255, 0.17);
  --accent: #a78bfa;
  --accent-strong: #8b5cf6;
  --accent-cyan: #5ee7f7;
  --success: #62d6a7;
  --warning: #f4c46b;
  --danger: #ff7d90;
  --info: #75b8ff;
  --focus: #b9a6ff;
}
```

The light theme uses an off-white violet-neutral canvas (`#f6f5f8`), white translucent surfaces, ink near `#17151c`, muted text near `#625f69`, and a darker violet accent near `#6d42dc`. Status colors must be tuned independently for AA contrast rather than mechanically inverted.

Accent gradients are reserved for the Zirk mark, hero glows, selected illustrations, and one-pixel emphasis:

```css
--zirk-gradient: linear-gradient(120deg, #8b5cf6 0%, #c084fc 46%, #5ee7f7 100%);
```

Never use the gradient behind body text or as a large full-page fill.

### Typography

Use a system-first sans stack to avoid network and privacy dependencies:

```css
--font-sans: Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont,
  "Segoe UI", sans-serif;
--font-mono: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
```

- Display: 48–68px, weight 580–650, line height 1.02–1.08, slightly negative tracking.
- Page title: 36–48px, weight 620, line height 1.1.
- Section title: 24–32px, weight 610.
- Body: 16–18px, line height 1.65–1.75; prose width 68–74 characters.
- UI: 13–15px, weight 520–620.
- Code: 13–15px, line height 1.65; never smaller than 12px.
- Eyebrow and metadata: 12px uppercase, 0.08em tracking; use sparingly.

### Spacing and sizing

Base spacing unit is 4px. Preferred steps: `4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 112`.

- Page max width: 1440px.
- Prose max width: 760px.
- Reference content max width: 920px.
- Sidebar: 248px; right outline: 216px.
- Header outer margin: 16px mobile, 24px desktop.
- Touch target: at least 44×44px when isolated; compact inline controls may be 36px with adequate separation.

### Radius, borders, and elevation

- Small controls: 10px.
- Inputs and code subpanels: 14px.
- Cards: 18px.
- Primary navigation shell and large feature panels: 24px.
- Pills: `999px`.
- Standard border: 1px `var(--border)`.
- Elevated border: 1px `var(--border-strong)` plus an inset white highlight at 4–6% opacity.

Use shadows conservatively: a broad low-opacity black shadow and an optional small accent halo. Elevation must still read when shadows are unavailable.

### Blur and translucency

Primary glass recipe:

```css
.glass {
  background: var(--surface-1);
  border: 1px solid var(--border);
}

@supports (backdrop-filter: blur(1px)) {
  .glass {
    background: rgba(28, 28, 34, 0.64);
    backdrop-filter: blur(18px) saturate(120%);
  }
}
```

Limit live blur to the fixed header, mobile navigation sheet, and search dialog. Ordinary content cards use solid or lightly translucent backgrounds without blur. This keeps scrolling stable on lower-powered devices.

## 3. Layout

### Global shell

Desktop documentation uses a three-column grid: left navigation, main content, and optional page outline. The header floats inside the viewport with rounded corners, echoing a compact tool window. Main content begins below it with generous breathing room.

At widths below 1100px, hide the right outline. Below 820px, move left navigation into a modal-like sheet activated by a clearly labeled menu button. Main content becomes one column. Code surfaces scroll horizontally within themselves; the page must never develop horizontal overflow.

### Landing page

The hero is centered but not oversized. It contains:

- a small status/edition eyebrow;
- a direct title and one-paragraph value proposition;
- primary “Start learning” and secondary “Language reference” actions;
- a code-workbench panel showing a concise Zirk program;
- a small row of factual traits: native, static types, structured concurrency, standalone binaries.

Below the hero, use an asymmetric grid for learning tracks and major reference areas. Avoid generic marketing-card repetition; alternate compact link groups, a timeline, and code-led explanation panels.

### Reference pages

Reference pages use:

- breadcrumb or section label;
- page title, concise summary, and maturity badge;
- “On this page” outline when space allows;
- sections with stable anchor links;
- syntax panels followed by semantics and examples;
- bottom previous/next navigation.

Sticky navigation must never cover an anchor target; use `scroll-margin-top`.

## 4. Components

### Header and primary navigation

The header is a wide rounded glass shell. Left: Zirk mark and “Zirk Docs”. Center: primary sections on large screens. Right: search trigger, repository link, and theme control. Selected navigation uses a quiet filled pill, not a bright underline.

### Zirk mark

Use an original geometric mark built with CSS or an owned SVG: intersecting arcs or facets suggesting compilation stages converging to a native artifact. Apply the Zirk gradient only to the mark. It must remain legible in monochrome and at 20px.

### Status notice

A persistent compact notice distinguishes “Language specification” from “Compiler implementation”. Use an info icon, clear sentence, and link to status. It must appear near the top of every page and cannot be dismissible in the first version.

### Buttons and links

- Primary button: light foreground on deep violet, subtle inner highlight.
- Secondary button: translucent neutral fill and standard border.
- Ghost button: transparent until hover/focus.
- Text link: inherit text with a visible underline offset; accent on hover.

Every interactive state needs default, hover, active, focus-visible, and disabled styling. Never remove outlines without an equally strong replacement.

### Code workbench

Code lives in an editor-like panel with a header row, language/file label, and copy control. The body uses the deepest surface, stable padding, line-height, and horizontal scrolling. Syntax colors must satisfy contrast:

- keywords: violet;
- types: cyan;
- strings: soft green;
- numbers: warm amber;
- comments: muted gray;
- punctuation: foreground.

Line numbers are optional and must be `aria-hidden`. Copy feedback changes label to “Copied” and announces success through a polite live region.

### Callouts

Four variants: note, tip, warning, danger. Each combines icon, label, border treatment, and text; color alone never encodes meaning. Callouts are content devices, not decoration, and should be uncommon.

### API/reference entry

An entry contains a linked name, compact signature, stability/status label, one-sentence description, parameter/return/error definitions, and example. Long member catalogs may use tables on desktop but must transform into labeled stacked rows on narrow screens.

### Search

Desktop search opens a centered glass dialog; mobile search occupies a near-full-height sheet. The input is immediately focused, results show section, title, summary, and optional shortcut hint. Arrow keys navigate, Enter opens, Escape closes. Empty and no-result states provide useful next actions.

### Theme control

Offer System, Dark, and Light in a compact segmented control or accessible menu. The active state has text and programmatic selection, not only an icon or color.

## 5. Motion

- Standard transition: 140ms ease-out for color, border, and opacity.
- Panels: at most 180ms with 4–8px translation.
- No parallax, continuous floating, cursor-following glow, or animated backdrop blur.
- Anchor scrolling may be smooth only when reduced motion is not requested.

Under `prefers-reduced-motion: reduce`, disable nonessential transitions and use immediate state changes.

## 6. Content design

Write documentation as a mature reference while labeling implementation reality. Prefer direct declarative prose and small complete examples. Each topic answers, in order:

1. What problem does this feature solve?
2. What is the canonical syntax?
3. What exactly does it mean?
4. What constraints or errors matter?
5. How does it interact with adjacent features?

Use “Zirk” rather than “we”. Use `code` for identifiers and syntax. Avoid emoji in navigation and reference headings. Tables are for comparison; procedures use ordered lists; warnings are reserved for consequences that can surprise or harm users.

## 7. Accessibility acceptance rules

- Target WCAG 2.2 AA.
- One `h1` per page; headings descend without skipped structural levels.
- All pages include a first-focus skip link.
- Landmarks use `header`, `nav`, `main`, `aside`, and `footer` appropriately.
- Text contrast is at least 4.5:1; large text and meaningful UI graphics at least 3:1.
- Focus indicators use at least a two-pixel visible treatment with contrast against adjacent colors.
- Controls have accessible names; icon-only buttons include tooltips as enhancement, not as their only name.
- Navigation sheets and dialogs manage focus and restore it to their trigger.
- Content works at 200% zoom and at 320 CSS pixels without loss or page-level horizontal scrolling.
- Theme and status are not communicated by color alone.

## 8. Definition of visual completion

A page is visually complete when it works in dark and light themes, with and without blur support, at mobile and desktop widths, by keyboard, with reduced motion, and with long code/content samples. It must look related to the requested contemporary translucent technical aesthetic while remaining unmistakably Zirk and comfortably readable for sustained documentation work.
