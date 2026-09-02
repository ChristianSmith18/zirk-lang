## Context

The handbook tree contains all intended areas, but structural presence is not equivalent to useful documentation. Many chapters are short summaries that cannot yet support implementation, daily use, or a public website. The semantic core and canonical checkpoints must remain authoritative while derivative material is expanded.

## Goals / Non-Goals

**Goals:**

- Produce TypeScript-Handbook-depth explanations where complexity requires them.
- Work in three reviewable blocks: standard library/toolchain, testing/low-level, then tutorials/reference.
- Give every operational chapter real signatures, examples, errors, permissions, performance and platform behavior where applicable.
- Remove `Purpose: TBD` from active main specs and finish a website-readiness audit.

**Non-Goals:**

- Change accepted Zirk semantics or implement compiler features.
- Force every page to the same length.
- Expand index pages with filler.
- Build the HTML/CSS/JS website in this change.

## Decisions

### Work block by block

Each block is completed and audited before the next begins. This makes review manageable and prevents later tutorials from depending on shallow reference material.

### Depth follows the reader's task

Module and tool pages describe purpose, imports or commands, principal API/signatures, semantics, errors, permissions, blocking/cancellation, complexity/performance, platform notes, valid examples, common failures, status and related sources when those dimensions apply. Orientation/index pages may remain short.

### Examples form coherent programs

Examples reuse consistent domains and progress from focused snippets to complete projects. Invalid examples include expected diagnostics and corrections. Aspirational syntax is labeled against current implementation status.

### Canonical documents remain owners

Handbook prose links to canonical semantic documents and specialized specs. It does not restate a conflicting rule. Repository-wide searches and source-map updates are part of every block.

### The final reference is generated-ready

SUMMARY order, previous/next links, anchors, terminology, API tables, diagnostic catalogs and feature status must be machine-checkable enough to feed the future website navigation and search index.

## Risks / Trade-offs

- **[Large editorial surface]** -> Complete and validate one block at a time with explicit checkpoints.
- **[Invented APIs while deepening prose]** -> Derive signatures from normative specs and record genuine gaps instead of guessing.
- **[Repetition creates drift]** -> Assign canonical owners and use links/tables for derivative views.
- **[Line-count incentives create filler]** -> Judge completeness by topic contract and reader task, never a fixed size.
- **[Current compiler cannot execute all examples]** -> Validate against specs and label implementation status honestly.

## Migration Plan

1. Complete and audit standard library plus toolchain.
2. Complete and audit testing plus native/low-level development.
3. Complete tutorials plus reference and appendices.
4. Replace main-spec TBD purposes, run global contradiction/navigation/example audits, and declare documentation website-ready.

## Open Questions

None. The block order and scope were selected by the language author.
