## ADDED Requirements

### Requirement: Weak referents are cleared before their storage is reclaimed

The collector SHALL clear every live `Weak<T>` handle's reference to a referent that collection determines is unreachable, strictly before that referent's storage is reclaimed — no observation of a weak handle SHALL ever expose reclaimed memory.

#### Scenario: Weak handle survives its referent's collection
- **WHEN** an object with a live `Weak<T>` handle pointing to it becomes unreachable and a collection runs
- **THEN** the handle itself remains valid, its referent reference is cleared, and the referent's storage is reclaimed
