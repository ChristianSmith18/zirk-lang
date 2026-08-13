## ADDED Requirements

### Requirement: Static dependency-free delivery
The documentation site SHALL run as static HTML, CSS, JavaScript, and local data/assets without a framework, package installation, bundling step, or mandatory network dependency.

#### Scenario: Site is served locally
- **WHEN** the `web/` directory is served by a basic static HTTP server
- **THEN** all documentation pages, styles, navigation, and core content operate without a build command

### Requirement: Persistent navigational model
The site SHALL provide a skip link, primary documentation navigation, contextual sidebar or section navigation, breadcrumbs where useful, in-page headings, previous/next learning links, and a usable footer.

#### Scenario: Keyboard user navigates a topic
- **WHEN** a keyboard user enters a documentation page
- **THEN** they can skip repeated navigation, reach page content and section links, and continue to adjacent topics with visible focus

### Requirement: Local documentation search
The site SHALL provide a keyboard-accessible client-side search over a curated local index of page titles, summaries, headings, keywords, and URLs.

#### Scenario: Reader searches for concurrency
- **WHEN** a reader searches for `parallel` or `task`
- **THEN** matching documentation results appear with topic context and selectable relative links

#### Scenario: JavaScript is unavailable
- **WHEN** client-side scripting is disabled
- **THEN** all content remains readable and reachable through ordinary navigation even though search enhancement is unavailable

### Requirement: Responsive documentation layout
The site SHALL support narrow mobile, tablet, laptop, and wide desktop viewports without horizontal page overflow or loss of content.

#### Scenario: Reader opens a narrow viewport
- **WHEN** viewport width is approximately 320 CSS pixels
- **THEN** navigation can be opened and dismissed, prose remains legible, code can scroll within its own surface, and controls remain usable

### Requirement: Theme and preference behavior
The site SHALL offer system, dark, and light theme choices, persist an explicit choice locally, and honor reduced-motion preferences.

#### Scenario: Reader selects a theme
- **WHEN** a reader selects light, dark, or system appearance
- **THEN** the page applies the chosen appearance consistently and restores the explicit selection on later visits

### Requirement: Accessible interaction and content
The site MUST use semantic landmarks and headings, meet WCAG 2.2 AA contrast targets, expose names and states for controls, provide visible focus, support keyboard operation, and avoid conveying meaning by color alone.

#### Scenario: Reader uses assistive technology
- **WHEN** navigation, search, theme, copy, and disclosure controls are inspected or operated without a pointer
- **THEN** their purpose, state, and resulting content changes are programmatically understandable

### Requirement: Code-example utilities
Code examples SHALL identify their language or context, preserve readable overflow behavior, and offer copy controls as progressive enhancement with non-disruptive feedback.

#### Scenario: Reader copies an example
- **WHEN** a reader activates a code block's copy control
- **THEN** the exact displayed example is copied and a temporary accessible success state is announced
