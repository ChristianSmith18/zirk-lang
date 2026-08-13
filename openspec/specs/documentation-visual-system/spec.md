## Purpose

Define the durable visual language and presentation requirements for Zirk documentation without coupling them to a specific framework or page implementation.

## Requirements

### Requirement: Root visual design guide
The repository SHALL contain a root-level `design.md`, distinct from OpenSpec's design artifact, that defines visual principles, design tokens, typography, color, surfaces, blur usage, spacing, radii, elevation, layout, components, interaction states, responsive behavior, accessibility, and content presentation.

#### Scenario: Contributor designs a new page
- **WHEN** a contributor consults the root `design.md`
- **THEN** they can construct a page consistent with the existing documentation experience without reverse-engineering CSS

### Requirement: Original Zirk identity
The site SHALL interpret the requested `jwt.io` qualities through an original Zirk identity and MUST NOT copy its trademark, logo, text, proprietary assets, or exact composition.

#### Scenario: Visual comparison is performed
- **WHEN** the Zirk site is compared with the reference
- **THEN** shared high-level traits such as dark layered surfaces, subtle blur, rounded panels, compact pills, and editor-inspired code areas are recognizable while branding and detailed composition remain distinct

### Requirement: Resilient translucent surfaces
Translucent surfaces SHALL retain legibility and hierarchy when `backdrop-filter` is unsupported, reduced, or computationally constrained.

#### Scenario: Blur is unavailable
- **WHEN** the browser does not render backdrop blur
- **THEN** opaque fallback colors, borders, and elevation preserve separation and readable contrast

### Requirement: Token-driven styling
Color, typography, spacing, radii, borders, shadows, motion, and layout dimensions SHALL be expressed through documented CSS custom properties rather than repeated unexplained literals.

#### Scenario: Theme changes
- **WHEN** the active color scheme changes
- **THEN** semantic tokens update backgrounds, text, borders, accents, code surfaces, and interactive states consistently

### Requirement: Restrained motion and effects
Motion and decorative effects SHALL support orientation and feedback, remain subtle, avoid blocking interaction, and be disabled or simplified for reduced-motion users.

#### Scenario: Reduced motion is requested
- **WHEN** the operating system reports `prefers-reduced-motion: reduce`
- **THEN** nonessential transitions, animated gradients, and scrolling effects are removed or reduced to immediate state changes

### Requirement: Documentation readability
The visual system SHALL prioritize readable prose measure, strong heading hierarchy, scannable reference tables, distinguishable code, and stable anchors over decorative density.

#### Scenario: Reader consumes a long reference page
- **WHEN** a page contains long-form prose, code, notes, tables, and API entries
- **THEN** line length, spacing, contrast, sticky elements, and section hierarchy support sustained reading without obscuring content
