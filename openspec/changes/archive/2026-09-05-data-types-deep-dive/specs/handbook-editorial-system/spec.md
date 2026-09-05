# Handbook Editorial System — Data Types Deep Dive

## ADDED Requirements

### Requirement: Coherent memory-behavior narrative for type chapters

The handbook SHALL provide a single conceptual through-line that places every important type on a memory/ownership spectrum: immediate values, native values, managed references, borrowed/dependent views, and unsafe pointers. Every expanded or new type chapter SHALL tie its examples back to one of those five positions and explicitly compare its sharing, cloning, and mutability behavior with the adjacent positions.

#### Scenario: Reader opens the new hub chapter

- **WHEN** a reader opens `docs/handbook/02-handbook/03-everyday-types/00-how-values-live-and-share.md`
- **THEN** the chapter maps the spectrum to concrete types and links to the per-type chapters that exemplify each position

#### Scenario: A per-type chapter follows the narrative

- **WHEN** a per-type chapter such as `List`, `Array`, `Pointer`, or `String` is expanded
- **THEN** it states the spectrum position, shows one code example that demonstrates it, and links to the hub

### Requirement: Per-type deep-dive example contract

Every expanded or new handbook type chapter SHALL include: a construction/literal example, a valid mutation or sharing example, a representative invalid example with the expected controlled error, and a "when to choose" callout. The chapter length SHALL follow semantic complexity and SHALL NOT be padded to a fixed size.

#### Scenario: Chapter depth matches complexity

- **WHEN** the `List` chapter is expanded
- **THEN** it contains construction, growth, sharing, an invalid mutation through `inmut::strict`, and a decision callout

#### Scenario: Simple types remain focused

- **WHEN** the `Boolean` chapter already satisfies the contract
- **THEN** the change leaves it unchanged and does not add filler examples
