## Purpose

Defines the safe, bounded, portable file and path APIs exposed by Zirk's bundled standard library, including operation-local filesystem authorization and deterministic ownership of open handles.

## Requirements

### Requirement: Path is an immutable value with distinct lexical and filesystem operations

`std.path` SHALL expose an immutable `Path` value constructed from a string and supporting string conversion, joining, basic component access, parent/name access, lexical normalization, and explicit lexical equality. These lexical operations SHALL NOT access the host filesystem or require filesystem consent. Lexical normalization SHALL NOT be treated as proof of existence, canonical identity, containment, or authorization. Filesystem-aware `absolute` and `canonicalize` operations SHALL return typed results and SHALL use the filesystem permission boundary. `absolute` SHALL require `filesystem.read` for the resulting path; `canonicalize` SHALL require `filesystem.metadata`. Physical identity queries and extended name/extension manipulation are outside the first delivery. Equality SHALL mean lexical equality under documented target path rules, not physical file identity.

#### Scenario: Join path segments without touching the filesystem
- **WHEN** a program constructs `Path("data").join("settings.json")`
- **THEN** it receives an immutable path value without reading the current directory or checking filesystem permission

#### Scenario: Normalize traversal lexically
- **WHEN** a program normalizes `Path("data/../config.json")`
- **THEN** the lexical result is `config.json`
- **AND** the operation makes no claim that the path exists or is authorized

#### Scenario: Canonicalize a path
- **WHEN** a program canonicalizes a path that resolves within its approved metadata scope or full read scope
- **THEN** it receives the canonical path in `Ok`
- **AND** a missing path, denied access, invalid path, or host failure is returned as a typed `Error`

#### Scenario: Lexical equality does not claim physical identity
- **WHEN** two lexical paths refer to the same filesystem object through links or alternate spellings
- **THEN** lexical equality follows only the documented target path rules
- **AND** this first delivery does not claim to determine physical file identity

### Requirement: Whole-file reads are bounded and preserve typed failures

`std.fs` SHALL expose `File.read_bytes(path, limit)` and `File.read_text(path, limit, encoding)` convenience operations that manage their host handle internally and return `Result` values. A read SHALL reject a file larger than the caller's limit before unbounded allocation and SHALL also enforce a 64 MiB whole-file implementation ceiling while reading. Text decoding SHALL be strict UTF-8 by default; malformed text SHALL produce a decoding error unless an explicit supported encoding is selected. Missing files, permission denial, size limits, decoding failures, and host I/O failures SHALL remain distinguishable typed errors.

#### Scenario: Read bounded UTF-8 text
- **WHEN** `File.read_text(path, limit: 1MiB)` reads a valid UTF-8 file no larger than the limit
- **THEN** it returns the decoded text in `Ok`
- **AND** the convenience operation releases its internal file handle before returning

#### Scenario: Reject an oversized file
- **WHEN** a file is larger than the requested or implementation maximum
- **THEN** the operation returns a typed limit error without allocating the full file contents

#### Scenario: Reject invalid UTF-8 by default
- **WHEN** a caller reads bytes that are not valid UTF-8 using the default text encoding
- **THEN** the operation returns a decoding error rather than silently replacing bytes

#### Scenario: Preserve byte contents
- **WHEN** a caller uses `File.read_bytes`
- **THEN** the returned byte sequence preserves every input byte without text decoding

### Requirement: Whole-file writes have explicit overwrite semantics and bounded behavior

`std.fs` SHALL expose `File.write_bytes(path, bytes)` and `File.write_text(path, text, encoding)` convenience operations in the first delivery. The documented default SHALL create a missing destination or replace the complete contents of an existing regular file; it SHALL NOT follow an unapproved target outside the effective path scope. Writes SHALL return `Result<Void, FileError>` and SHALL preserve permission, invalid path, wrong type, and host errors. The API SHALL document that direct writes may leave partial output after interruption; crash-safe replacement SHALL use the separate next-stage atomic-write operation.

#### Scenario: Write a new text file
- **WHEN** a program writes text to an absent path with matching write consent
- **THEN** the file is created with the exact encoded contents and the operation returns `Ok`

#### Scenario: Replace an existing file
- **WHEN** a program writes text to an existing regular file with matching write consent
- **THEN** its contents are replaced according to the documented direct-write semantics

#### Scenario: Deny write without consent
- **WHEN** a program attempts to write a path without matching write consent
- **THEN** the operation returns a typed permission error before changing the path's contents

### Requirement: File opening uses typed modes and explicit resource ownership

`File.open(path, options)` SHALL return `Result<File, FileError>` in the first delivery. `OpenOptions` SHALL use typed constructors/builders rather than magic mode strings, SHALL default to no authority, and SHALL reject a configuration that requests no operation. First-delivery modes include read, write, create, create-new, and truncate; append mode is added in the next stage with the append operations. Read and write authority SHALL be checked independently for the effective open mode; create and truncate SHALL require write authority. `create_new` SHALL fail atomically if the destination already exists. The runtime SHALL allow at most 256 simultaneously open managed `File` resources per process; exceeding this limit SHALL return a typed limit error before opening another host handle. `File` SHALL implement the language's `Resource<FileError>` contract and SHALL be usable with `match with` so every successful acquisition is closed on normal exit, early return, exception, and structured cancellation. Closing SHALL be idempotent; close failures SHALL be preserved by the resource cleanup contract.

#### Scenario: Open a file for reading
- **WHEN** a program opens an existing file with `OpenOptions.read_only()` and has matching read consent
- **THEN** it receives an owned `File` resource in `Ok`

#### Scenario: Reject an empty open mode
- **WHEN** a program attempts to open with no read, write, append, or creation capability
- **THEN** validation rejects the options before a host file is opened

#### Scenario: Create a new file without a check-then-create race
- **WHEN** a program opens with `create_new` and another file already occupies the destination
- **THEN** the operation returns `AlreadyExists` and does not replace the existing file

#### Scenario: Respect the managed handle limit
- **WHEN** a process already owns 256 managed `File` resources and opens another file
- **THEN** `File.open` returns a typed limit error without opening an additional host handle

#### Scenario: Close on every managed-scope exit
- **WHEN** an opened `File` is managed by `match with` and control exits by return, exception, cancellation, or normal completion
- **THEN** the handle is closed exactly once before control leaves the scope

#### Scenario: Close is repeated
- **WHEN** a caller closes a `File` more than once
- **THEN** later closes are harmless and no recycled host handle is touched

### Requirement: Progressive file reads have bounded outcomes

In the next stage, an opened `File` SHALL support bounded partial/exact byte reads, `read_line`, lazy line events, and lazy fixed-size chunk events without loading the complete file into memory. `read_line()` SHALL return `Result<FileLine, FileError>`, where `FileLine` distinguishes `Line(String)` from normal `End`. `lines()` SHALL lazily yield `FileLine.Line(text)` events; normal EOF SHALL end iteration without an extra value, and a read failure SHALL be yielded once as `FileLine.Error(error)` before iteration terminates. `chunks(size)` SHALL lazily yield `FileChunk.Data(bytes)` events and one `FileChunk.Error(error)` before stopping on failure. Exact reads SHALL return a typed premature-EOF error when fewer bytes than requested are available. Line iteration SHALL decode strict UTF-8 by default, remove the line terminator while preserving an empty line as a value, and handle CRLF as one terminator. The iterators SHALL borrow and SHALL NOT outlive their owning `File`; the managed file scope SHALL close the underlying handle on every exit path. The maximum line size SHALL be 1 MiB and chunk size SHALL be in the range 1 byte through 4 MiB.

#### Scenario: Read a complete line and then EOF
- **WHEN** a caller iterates a file containing two lines
- **THEN** it receives both line values in order without their newline terminators
- **AND** normal EOF ends iteration without manufacturing an empty extra line

#### Scenario: Preserve an empty line
- **WHEN** a file contains an empty line between two non-empty lines
- **THEN** the line iterator yields an empty string for that line

#### Scenario: Report a progressive read error
- **WHEN** a filesystem failure occurs while iterating lines or chunks
- **THEN** iteration yields one typed error outcome and terminates
- **AND** it does not present the error as EOF or silently continue with partial data

#### Scenario: Reject an exact read past EOF
- **WHEN** `read_exact(count)` reaches EOF before reading `count` bytes
- **THEN** it returns a typed end-of-stream error

#### Scenario: Reject an excessive chunk request
- **WHEN** a caller requests a chunk larger than the documented maximum
- **THEN** the operation returns a typed limit error before allocating that chunk

### Requirement: File append and atomic replacement have distinct semantics

In the next stage, `File.append_bytes` and `File.append_text` SHALL use operating-system append semantics and SHALL return typed results. An append call SHALL be documented as the largest atomicity unit the target can guarantee; multiple calls SHALL NOT be represented as a transaction. `File.write_bytes_atomic` and `File.write_text_atomic` SHALL create a unique temporary sibling exclusively, write and sync its contents, then atomically publish it on the same filesystem. Readers SHALL observe the old complete contents or the new complete contents, never a partial replacement. If publication fails, the old target SHALL remain intact and temporary cleanup SHALL be attempted and reported if it fails. The operation guarantees atomic publication, not persistence across sudden power loss unless a target-specific parent-directory durability guarantee is explicitly documented.

#### Scenario: Append without replacing existing contents
- **WHEN** a program appends text to an existing file with matching write consent
- **THEN** the new encoded bytes are added after the existing contents

#### Scenario: Atomically replace a file
- **WHEN** an atomic text write succeeds
- **THEN** readers observe either the old complete contents or the new complete contents, never a partially written replacement

#### Scenario: Atomic replacement fails before publication
- **WHEN** an atomic write fails before the replacement is committed
- **THEN** the original destination remains intact and temporary state is cleaned up or reported as a cleanup failure

### Requirement: File metadata is bounded and does not imply extra authority

In the next stage, `File.metadata_for(path)` and `file.metadata()` SHALL return an immutable metadata value with the documented file type, size, and timestamps, or a typed error. Metadata inspection SHALL require `filesystem.metadata` authority for the path; matching `filesystem.read` authority SHALL also satisfy this narrower check. Metadata-only consent SHALL NOT imply permission to read file contents, modify the file, or traverse unrelated paths. A successful absence check SHALL be distinguishable from permission denial and other failures.

#### Scenario: Inspect an authorized file
- **WHEN** a program requests metadata for a path inside its metadata or full read scope
- **THEN** it receives a typed immutable metadata snapshot

#### Scenario: Metadata consent does not authorize content reads
- **WHEN** an application has `filesystem.metadata` consent for a path and attempts to read its contents
- **THEN** metadata inspection succeeds and the content read is independently denied before reading bytes

#### Scenario: Metadata does not grant content access
- **WHEN** a program has consent to inspect metadata but lacks permission to read contents
- **THEN** a subsequent content read is checked separately and is denied

#### Scenario: Distinguish absence from denial
- **WHEN** `File.exists(path)` cannot inspect a path due to missing consent
- **THEN** it returns a permission error rather than `Ok(false)`

### Requirement: Filesystem operations enforce path-scoped consent at the host boundary

Every filesystem operation that inspects or changes host state SHALL check the corresponding `filesystem.read`, `filesystem.metadata`, or `filesystem.write` authority immediately before the effect. The runtime SHALL authorize the effective path, not only its input spelling. A file grant SHALL apply only to that file; a directory subtree SHALL be authorized only by an explicitly approved directory scope and SHALL NOT be inferred from a lexical prefix, `..` normalization, a package requirement, or consent for a different operation. Metadata consent SHALL NOT authorize content reads or writes; full read consent MAY cover metadata inspection at the same path. Path resolution and the subsequent open or mutation SHALL resist symlink escape and check/use races using platform-appropriate handle-relative operations. Missing consent SHALL follow the existing first-use yes/no prompt, persist only the concrete approved scope, add a missing direct capability to the single `requires` declaration as defined by the permission contract, and return typed denial when rejected or non-interactive. Denial SHALL leave the file system unchanged and SHALL NOT prevent unrelated program execution.

#### Scenario: First file read prompts for the concrete path
- **WHEN** an interactive run reaches a read of a canonical file path with no matching saved consent
- **THEN** the prompt identifies `filesystem.read`, the concrete path, and the requester provenance
- **AND** approval persists that scope and permits the operation

#### Scenario: Read consent does not authorize writes
- **WHEN** a program has consent to read a file but attempts to replace it
- **THEN** the write is independently denied before modification

#### Scenario: A sibling path is outside exact file consent
- **WHEN** a program has consent for one file and attempts to read a neighboring file
- **THEN** the second operation requires its own matching consent

#### Scenario: Symlink escapes the approved scope
- **WHEN** an input path resolves through a symbolic link outside its authorized scope
- **THEN** the runtime denies the operation before reading or changing the target

#### Scenario: An intermediate component is replaced with a symlink after resolution
- **WHEN** an attacker replaces a canonical parent-path component with a symlink or reparse point between resolution and use
- **THEN** component-by-component no-follow directory opening rejects the path
- **AND** no operation reaches the symlink target

#### Scenario: The final file is replaced with a symlink after resolution
- **WHEN** an attacker replaces the resolved file entry with a symlink or reparse point before the runtime opens it
- **THEN** the runtime rejects the final target before reading or changing its contents
- **AND** the symlink target remains unchanged

#### Scenario: A validated parent is replaced after its handle is opened
- **WHEN** an attacker renames or replaces a parent-path component after the runtime has opened that directory handle
- **THEN** subsequent traversal and file operations remain relative to the validated handle or fail
- **AND** the replacement path cannot redirect the operation

#### Scenario: Non-interactive access lacks consent
- **WHEN** an unapproved filesystem operation runs without an interactive terminal
- **THEN** it returns typed permission denial and emits a useful warning without blocking
- **AND** ordinary program execution outside that operation can continue

### Requirement: Filesystem target support is explicit and fail-closed

The native filesystem package SHALL provide equivalent documented behavior on Zirk's declared supported native targets: macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64. Platform-specific implementations MAY differ while preserving the same public outcomes, path-scope checks, resource cleanup, and size bounds. Unsupported operations SHALL return explicit typed capability errors. Browser execution SHALL reject a reachable filesystem effect during capability preflight and SHALL NOT access a host or browser filesystem implicitly.

#### Scenario: Run the native filesystem conformance suite
- **WHEN** the filesystem conformance suite runs on a declared native target
- **THEN** it verifies read, write, resource closure, denial, symlink containment, bounded allocation, and typed errors on that target

#### Scenario: Compile filesystem access for a browser target
- **WHEN** browser-targeted code reaches a filesystem operation
- **THEN** preflight reports an unsupported host capability before execution
- **AND** no filesystem operation is attempted
