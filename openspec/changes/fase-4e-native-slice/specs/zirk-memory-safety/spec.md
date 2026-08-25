## MODIFIED Requirements

### Requirement: Checked dependent lifetimes
Native views, pinned borrows, borrowed iterators, resource-derived handles, and internal-storage views MUST NOT escape the lifetime of their owner, and the compiler SHALL enforce this without requiring public lifetime syntax.

#### Scenario: Native view escapes its borrow
- **WHEN** code attempts to return a `NativeSlice<T>` whose validated owner ends in the function
- **THEN** compilation fails with the owner and escape path identified

### Requirement: Pointer and native-view behavior
Pointer arithmetic SHALL be measured in elements, byte offsets SHALL be explicit, null pointers SHALL be permitted only as raw native values, and validated `NativeSlice<T>`/`NativeSliceMut<T>` views SHALL carry bounded extent and lifetime.

#### Scenario: Null raw pointer is inspected
- **WHEN** native code returns a null `Pointer<T>`
- **THEN** `is_null` can inspect it without creating a nullable safe reference

#### Scenario: View indexing stays bounds-checked
- **WHEN** safe code indexes a validated `NativeSlice<T>`/`NativeSliceMut<T>` with an out-of-range position
- **THEN** the operation fails with a controlled bounds error rather than reading or writing outside the validated extent

#### Scenario: Construction validates before a view exists
- **WHEN** `pointer.as_slice(length)` or `pointer.as_slice_mut(length)` is called with a null pointer, misaligned address, or unrepresentable extent
- **THEN** construction returns `Error` and no `NativeSlice<T>`/`NativeSliceMut<T>` value is produced
