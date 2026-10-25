## ADDED Requirements

### Requirement: Directory authority is explicit, bounded, and operation-specific

The filesystem permission model SHALL support an explicitly approved directory scope for APIs whose requested resource is a directory, including enumeration and recursive traversal. The approval prompt and inspection output SHALL state whether the scope covers only the directory object or its descendants; a recursive descendant scope SHALL be granted only when the attempted operation requests it and the user approves that extent. The effective path SHALL remain within the authorized subtree after resolving links. Read, write, metadata, link, lock, and watch effects SHALL retain their distinct operation checks; consent for a directory listing SHALL NOT imply write, link creation, content access, or watcher authority unless that specific operation and extent are requested and approved.

#### Scenario: Approve one directory listing
- **WHEN** a program requests a listing of a concrete directory without saved read consent
- **THEN** the prompt identifies the canonical directory and listing operation
- **AND** approval permits the requested listing without granting descendant content reads or writes

#### Scenario: Approve a recursive subtree operation
- **WHEN** a program requests recursive traversal and the user explicitly approves descendant scope
- **THEN** descendants may be traversed only while their effective paths remain within the approved subtree and operation

#### Scenario: Directory read does not imply recursive write
- **WHEN** an application has directory read/traversal consent and attempts recursive deletion
- **THEN** deletion is denied unless matching write authority for the requested destructive subtree operation exists

#### Scenario: Link target leaves approved subtree
- **WHEN** a recursive operation encounters a link target outside the approved directory subtree
- **THEN** the target is not accessed and a typed permission denial is returned

### Requirement: Filesystem locks and watchers do not expand file authority

A lock SHALL authorize only the specified coordination operation on the validated file identity; a watcher SHALL authorize only observation of the approved directory scope and its documented event metadata. Neither a lock nor a watcher SHALL grant permission to read contents, mutate entries, follow links outside scope, or inspect unrelated paths. Asynchronous execution SHALL repeat authorization at the actual host operation after suspension, retry, or path re-resolution.

#### Scenario: Lock is not a content read grant
- **WHEN** a program successfully acquires a shared lock but lacks content-read consent
- **THEN** reading the locked file is checked separately and remains denied

#### Scenario: Watcher does not grant file mutation
- **WHEN** a watcher observes a path within its approved scope and the program attempts to delete it
- **THEN** the delete requires a distinct matching write approval

#### Scenario: Permission is rechecked after suspension
- **WHEN** an async filesystem operation resumes after suspension and its effective resource has changed
- **THEN** the runtime checks consent for that resource before its next side effect
