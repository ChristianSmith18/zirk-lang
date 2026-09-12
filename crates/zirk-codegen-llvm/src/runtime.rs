//! Declaration of the runtime symbols the generated code calls.
//!
//! These are the C ABI boundary of `docs/decisions/ADR-002-runtime-staticlib.md`.
//! Codegen declares them and calls them; the implementation lives in
//! `zirk-runtime` and is resolved at link time.
//!
//! The names are a compatibility surface: changing one breaks already compiled
//! binaries.

use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::FunctionValue;

/// Names of the runtime symbols, in one place so codegen and tests agree.
pub mod symbols {
    /// Initializes the runtime before `main`.
    pub const INIT: &str = "zirk_rt_init";
    /// Runs the Zirk entrypoint as the cooperative executor's root task.
    pub const RUN_MAIN: &str = "zirk_rt_run_main";
    /// Starts a child scheduler task from a body thunk and boxed-callable
    /// capture block. This provisional symbol is reused and renamed by
    /// `concurrent-blocks-and-timers`.
    pub const SPAWN: &str = "zirk_rt_spawn";
    /// Suspends the current scheduler task until a child result is available.
    /// This provisional symbol is reused and renamed by
    /// `concurrent-blocks-and-timers`.
    pub const JOB_WAIT: &str = "zirk_rt_job_wait";
    /// Reports whether a job has reached a terminal state.
    pub const JOB_DONE: &str = "zirk_rt_job_done";
    /// `zirk_rt_parallel_for(kind, a, b, count, thunk, capture)` — the
    /// `parallel { for x in coll { ... } }` work-splitting dispatch
    /// (`parallel-cpu-regions` task 6.1).
    pub const PARALLEL_FOR: &str = "zirk_rt_parallel_for";
    /// Opens a structured-concurrency scope.
    pub const SCOPE_ENTER: &str = "zirk_rt_scope_enter";
    /// Advances the close protocol for a structured-concurrency scope.
    pub const SCOPE_EXIT: &str = "zirk_rt_scope_exit";
    /// Registers a joinable branch with a scope.
    pub const BRANCH_REGISTER: &str = "zirk_rt_branch_register";
    /// Requests cooperative cancellation of a job.
    pub const CANCEL: &str = "zirk_rt_cancel";
    /// Creates an ambient one-shot timer job.
    pub const TIMER_AFTER: &str = "zirk_rt_timer_after";
    /// Creates an ambient fixed-delay timer job.
    pub const TIMER_EVERY: &str = "zirk_rt_timer_every";
    /// Suspends the current job until a monotonic timer deadline.
    pub const SLEEP: &str = "zirk_rt_sleep";
    /// Carries a branch's unhandled Zirk exception into a scheduler-visible
    /// branch failure so the owning scope cancels its siblings.
    pub const BRANCH_FAIL: &str = "zirk_rt_branch_fail";
    /// Shuts the runtime down after `main`.
    pub const SHUTDOWN: &str = "zirk_rt_shutdown";
    /// Builds a `String` from UTF-8 bytes and a length.
    pub const STR_FROM_UTF8: &str = "zirk_str_from_utf8";
    /// Converts an `Int8` into a `String`.
    pub const STR_FROM_I8: &str = "zirk_str_from_i8";
    /// Converts an `Int16` into a `String`.
    pub const STR_FROM_I16: &str = "zirk_str_from_i16";
    /// Converts an `Int32` into a `String`.
    pub const STR_FROM_I32: &str = "zirk_str_from_i32";
    /// Converts an `Int64` into a `String`.
    pub const STR_FROM_I64: &str = "zirk_str_from_i64";
    /// Converts an `Int128` into a `String`, taken by pointer (no stable
    /// cross-target ABI for a by-value 128-bit integer).
    pub const STR_FROM_I128: &str = "zirk_str_from_i128";
    /// Converts a `UInt8` into a `String`.
    pub const STR_FROM_U8: &str = "zirk_str_from_u8";
    /// Converts a `UInt16` into a `String`.
    pub const STR_FROM_U16: &str = "zirk_str_from_u16";
    /// Converts a `UInt32` into a `String`.
    pub const STR_FROM_U32: &str = "zirk_str_from_u32";
    /// Converts a `UInt64` into a `String`.
    pub const STR_FROM_U64: &str = "zirk_str_from_u64";
    /// Converts a `UInt128` into a `String`, taken by pointer — see
    /// `STR_FROM_I128`.
    pub const STR_FROM_U128: &str = "zirk_str_from_u128";
    /// Converts a `Float32` into a `String`.
    pub const STR_FROM_F32: &str = "zirk_str_from_f32";
    /// Converts a `Float64` into a `String`.
    pub const STR_FROM_F64: &str = "zirk_str_from_f64";
    /// Formats a `Float64` according to a `spec` string.
    pub const FLOAT_FORMAT: &str = "zirk_float_format";
    /// Converts a `Boolean` into a `String`.
    pub const STR_FROM_BOOL: &str = "zirk_str_from_bool";
    /// Byte offset of the `index`-th grapheme, or `-1` past the end
    /// (roadmap Phase 4e, `String[index]` read-only access).
    pub const STR_GRAPHEME_OFFSET: &str = "zirk_str_grapheme_offset";
    /// Byte length of the grapheme at a given offset, or `-1` past the end
    /// (roadmap Phase 3b, task 6.3: `for ... in` over `String`).
    pub const STR_GRAPHEME_LEN_AT: &str = "zirk_str_grapheme_len_at";
    /// Builds a `Char` from a byte range already known to be one grapheme.
    pub const STR_GRAPHEME_SLICE: &str = "zirk_str_grapheme_slice";
    /// Structural equality of two strings.
    pub const STR_EQ: &str = "zirk_str_eq";
    /// Writes a `String` to standard output with a line break.
    pub const IO_PRINTLN: &str = "zirk_io_println";
    /// Reports a division by zero and terminates.
    pub const DIVISION_BY_ZERO: &str = "zirk_rt_division_by_zero";
    /// Obtains storage for an object. The strategy behind it is the runtime's
    /// (ADR-003), which is why the IR only ever says `alloc <type>`.
    pub const ALLOC: &str = "zirk_rt_alloc";
    /// Reports that an object could not be allocated and terminates.
    pub const ALLOCATION_FAILED: &str = "zirk_rt_allocation_failed";
    /// Concatenates two strings.
    pub const STR_CONCAT: &str = "zirk_str_concat";
    /// Repeats a string a non-negative number of times.
    pub const STR_REPEAT: &str = "zirk_str_repeat";
    /// Reports an invalid repetition count and terminates.
    pub const INVALID_REPEAT: &str = "zirk_rt_invalid_repeat";
    /// Reports a shift by a negative amount or by too much and terminates.
    pub const INVALID_SHIFT: &str = "zirk_rt_invalid_shift";
    /// Finds the dispatch table a descriptor holds for a contract.
    pub const CONTRACT_TABLE: &str = "zirk_rt_contract_table";
    /// Reports a descriptor missing a contract it was said to satisfy.
    pub const MISSING_CONTRACT: &str = "zirk_rt_missing_contract";
    /// Confirms a checked cast against a descriptor's ancestor list,
    /// terminating if it is not one of them.
    pub const CHECK_CAST: &str = "zirk_rt_check_cast";
    /// Reports a checked cast whose runtime type does not match and
    /// terminates.
    pub const INVALID_CAST: &str = "zirk_rt_invalid_cast";
    /// Reports a `Float` operation that would produce `NaN` and terminates
    /// (roadmap Phase 3b, design decision D2: `Float` prohibits `NaN` in the
    /// type, not after the fact — an infinite result is a valid value,
    /// unlike `NaN`).
    pub const FLOAT_NAN: &str = "zirk_rt_float_nan";
    /// Reports a program-supplied `fatalError(message)` and terminates
    /// (roadmap Phase 4a).
    pub const FATAL_ERROR: &str = "zirk_rt_fatal_error";
    /// Records the pending exception (roadmap Phase 4b) — `throw`'s own
    /// lowering.
    pub const THROW: &str = "zirk_rt_throw";
    /// Whether an exception is pending (roadmap Phase 4b) — checked after
    /// every call to a function that can throw.
    pub const HAS_PENDING_EXCEPTION: &str = "zirk_rt_has_pending_exception";
    /// Takes the pending exception, clearing the slot (roadmap Phase 4b) —
    /// a matching `catch`'s own lowering.
    pub const TAKE_PENDING_EXCEPTION: &str = "zirk_rt_take_pending_exception";
    /// Tests a descriptor's ancestor list against a target class, without
    /// terminating when it does not match (roadmap Phase 4b) — a `catch`
    /// clause's own coverage test.
    pub const IS_INSTANCE: &str = "zirk_rt_is_instance";
    /// Returns the lazily-built `String` stack trace of an exception
    /// (roadmap Phase 4b).
    pub const STACK_TRACE: &str = "zirk_rt_stack_trace";
    /// Returns the exception suppressed by another exception (roadmap Phase 4b).
    pub const SUPPRESSED: &str = "zirk_rt_suppressed";
    /// Attaches a suppressed exception before `throw` (roadmap Phase 4b).
    pub const SET_SUPPRESSED: &str = "zirk_rt_set_suppressed";
    /// Reports an exception that escaped `main` uncaught and terminates
    /// (roadmap Phase 4b).
    pub const UNCAUGHT_EXCEPTION: &str = "zirk_rt_uncaught_exception";
    /// Pushes this function activation's shadow-stack frame
    /// (`fase-4e-colector-mark-sweep`, design D2) — called once at function
    /// entry, after every reference-typed slot has been zero-initialized
    /// (design D5).
    pub const PUSH_FRAME: &str = "zirk_rt_push_frame";
    /// Pops the shadow-stack frame [`PUSH_FRAME`] pushed — called immediately
    /// before every `Terminator::Return` lowers to `ret` (design D2).
    pub const POP_FRAME: &str = "zirk_rt_pop_frame";
    /// The WeakCell sentinel descriptor (`fase-4e-weak`, design D2): a fixed
    /// global symbol, not a per-class table like an ordinary object's own
    /// descriptor — `WeakFrom` stores its address into a fresh WeakCell's
    /// header word, and the collector's mark pass compares an object's own
    /// descriptor against this same address to recognize one.
    pub const WEAK_CELL_DESCRIPTOR: &str = "zirk_rt_weak_cell_descriptor";
    /// Set to a nonzero byte the first time a `Weak<T>` is ever allocated
    /// (`fase-4e-weak`, design's own risk mitigation) — the collector's
    /// weak-clearing pass reads this before walking the allocation list, so
    /// a program that never uses `Weak<T>` pays only the one check.
    pub const WEAK_CELL_EVER_ALLOCATED: &str = "zirk_rt_weak_cell_ever_allocated";
    /// The deep-clone-graph traversal (roadmap Phase 4e, `fase-4e-clone`,
    /// design D2): one generic, descriptor-driven entry point for the whole
    /// recursive clone — see `InstKind::Clone`'s own doc comment for why
    /// the traversal lives here rather than being unrolled across several
    /// IR instructions.
    pub const CLONE: &str = "zirk_rt_clone";
    /// Allocates the capture block for a boxed callable (roadmap Phase 4d,
    /// `phase-4d-callables`, task 5). The current implementation is a stub.
    pub const ALLOC_CALLABLE: &str = "zirk_rt_alloc_callable";
    /// Clones the capture block of a boxed callable (roadmap Phase 4d,
    /// `phase-4d-callables`, task 6). The current implementation is a stub.
    pub const CLONE_CALLABLE: &str = "zirk_rt_clone_callable";
    /// Begins a new per-`unsafe`-block undo log (roadmap Phase 4e,
    /// `fase-4e-unsafe-journal`, design D1) — `InstKind::JournalBegin`'s own
    /// lowering.
    pub const JOURNAL_BEGIN: &str = "zirk_rt_journal_begin";
    /// Snapshots bytes at an address into a journal's undo log, before the
    /// write it guards executes (design D1/D3) — both
    /// `InstKind::JournalRecordSlot`/`JournalRecordField` lower to this same
    /// symbol, address and length already resolved by codegen.
    pub const JOURNAL_RECORD: &str = "zirk_rt_journal_record";
    /// Durably commits a journal: discards the undo log without restoring
    /// (design D1) — `InstKind::JournalCommit`'s own lowering.
    pub const JOURNAL_COMMIT: &str = "zirk_rt_journal_commit";
    /// Rolls a journal back: restores every recorded snapshot in reverse
    /// order, then discards the log (design D1/D2) —
    /// `InstKind::JournalRollback`'s own lowering.
    pub const JOURNAL_ROLLBACK: &str = "zirk_rt_journal_rollback";
    /// `pointer.as_slice(length)`/`.as_slice_mut(length)`'s own runtime
    /// validation (roadmap Phase 4e, `fase-4e-native-slice`, design D3/D5,
    /// `InstKind::NativeSliceValidate`'s own doc comment).
    pub const NATIVE_SLICE_VALIDATE: &str = "zirk_rt_native_slice_validate";
    /// Whether an active cancellation token is set (roadmap Phase 4c,
    /// `phase-4c-resources`, design D5) — consulted before resource close.
    pub const IS_CANCELLED: &str = "zirk_rt_is_cancelled";
    /// Closes a group of acquired resources right-to-left (roadmap Phase 4c,
    /// `phase-4c-resources`, design D1/D5) — `match with` grouped cleanup.
    pub const RESOURCE_CLOSE_GROUP: &str = "zirk_rt_resource_close_group";
    /// Transfers ownership of a resource, invalidating the source slot
    /// (roadmap Phase 4c, `phase-4c-resources`, design D3).
    pub const RESOURCE_TRANSFER: &str = "zirk_rt_resource_transfer";
    /// `zirk_rt_pin_object` (roadmap Phase 4e, `phase-4e-memory`, design D1):
    /// adds an object to the per-thread pin list.
    pub const PIN_OBJECT: &str = "zirk_rt_pin_object";
    /// `zirk_rt_unpin_object` (roadmap Phase 4e, `phase-4e-memory`, design
    /// D1): removes an object from the per-thread pin list.
    pub const UNPIN_OBJECT: &str = "zirk_rt_unpin_object";
    /// `zirk_rt_dependent_base` (roadmap Phase 4e, `phase-4e-memory`, design
    /// D1): reads the base pointer from a `Dependent<T>` value.
    pub const DEPENDENT_BASE: &str = "zirk_rt_dependent_base";
    /// Compiles a pattern into a `Regex` handle.
    pub const REGEX_FROM_PATTERN: &str = "zirk_regex_from_pattern";
    /// Tests whether a `Regex` matches a string.
    pub const REGEX_IS_MATCH: &str = "zirk_regex_is_match";
    /// Replaces matches of a `Regex` in a string.
    pub const REGEX_REPLACE: &str = "zirk_regex_replace";
    /// Renders a `Regex` as its pattern string.
    pub const REGEX_TO_STRING: &str = "zirk_regex_to_string";
    /// Finds the first match of a `Regex` in `text`.
    pub const REGEX_FIND: &str = "zirk_regex_find";
    /// Returns the nth positional capture group of a `Regex.Match`.
    pub const REGEX_MATCH_GROUP_POS: &str = "zirk_regex_match_group_pos";
    /// Returns the named capture group of a `Regex.Match`.
    pub const REGEX_MATCH_GROUP_NAME: &str = "zirk_regex_match_group_name";
    /// `zirk_str_trim(handle) -> *mut c_void`.
    pub const STR_TRIM: &str = "zirk_str_trim";
    /// `zirk_str_contains(handle, pat) -> bool`.
    pub const STR_CONTAINS: &str = "zirk_str_contains";
    /// `zirk_str_starts_with(handle, pat) -> bool`.
    pub const STR_STARTS_WITH: &str = "zirk_str_starts_with";
    /// `zirk_str_ends_with(handle, pat) -> bool`.
    pub const STR_ENDS_WITH: &str = "zirk_str_ends_with";
    /// `zirk_str_substring(handle, start, end) -> *mut c_void`.
    pub const STR_SUBSTRING: &str = "zirk_str_substring";
    /// `zirk_str_search(handle, pat) -> i64`.
    pub const STR_SEARCH: &str = "zirk_str_search";
    /// `zirk_char_is_uppercase(c) -> bool`.
    pub const CHAR_IS_UPPERCASE: &str = "zirk_char_is_uppercase";
    /// `zirk_char_is_lowercase(c) -> bool`.
    pub const CHAR_IS_LOWERCASE: &str = "zirk_char_is_lowercase";
    /// `zirk_char_is_digit(c) -> bool`.
    pub const CHAR_IS_DIGIT: &str = "zirk_char_is_digit";
    /// `zirk_char_is_letter(c) -> bool`.
    pub const CHAR_IS_LETTER: &str = "zirk_char_is_letter";
    /// `zirk_char_is_whitespace(c) -> bool`.
    pub const CHAR_IS_WHITESPACE: &str = "zirk_char_is_whitespace";
    /// `zirk_char_to_uppercase(c) -> *mut c_void`.
    pub const CHAR_TO_UPPERCASE: &str = "zirk_char_to_uppercase";
    /// `zirk_char_to_lowercase(c) -> *mut c_void`.
    pub const CHAR_TO_LOWERCASE: &str = "zirk_char_to_lowercase";

    /// `zirk_rt_array_new(capacity, elem_size, elem_align, is_ref) -> *mut c_void`.
    pub const ARRAY_NEW: &str = "zirk_rt_array_new";
    /// `zirk_rt_list_new(elem_size, elem_align, is_ref) -> *mut c_void`.
    pub const LIST_NEW: &str = "zirk_rt_list_new";
    /// `zirk_rt_map_new() -> *mut c_void`.
    pub const MAP_NEW: &str = "zirk_rt_map_new";
    /// `zirk_rt_map_length(map) -> i64`.
    pub const MAP_LENGTH: &str = "zirk_rt_map_length";
    /// `zirk_rt_map_is_empty(map) -> i32`.
    pub const MAP_IS_EMPTY: &str = "zirk_rt_map_is_empty";
    /// `zirk_rt_map_set(map, key, value)`.
    pub const MAP_SET: &str = "zirk_rt_map_set";
    /// `zirk_rt_map_contains_key(map, key) -> i32`.
    pub const MAP_CONTAINS_KEY: &str = "zirk_rt_map_contains_key";
    /// `zirk_rt_map_get(map, key) -> i64`.
    pub const MAP_GET: &str = "zirk_rt_map_get";
    /// `zirk_rt_map_remove(map, key) -> i32`.
    pub const MAP_REMOVE: &str = "zirk_rt_map_remove";
    /// `zirk_rt_set_new() -> *mut c_void`.
    pub const SET_NEW: &str = "zirk_rt_set_new";
    /// `zirk_rt_set_length(set) -> i64`.
    pub const SET_LENGTH: &str = "zirk_rt_set_length";
    /// `zirk_rt_set_is_empty(set) -> i32`.
    pub const SET_IS_EMPTY: &str = "zirk_rt_set_is_empty";
    /// `zirk_rt_set_add(set, value)`.
    pub const SET_ADD: &str = "zirk_rt_set_add";
    /// `zirk_rt_set_contains(set, value) -> i32`.
    pub const SET_CONTAINS: &str = "zirk_rt_set_contains";
    /// `zirk_rt_set_remove(set, value) -> i32`.
    pub const SET_REMOVE: &str = "zirk_rt_set_remove";
    /// `zirk_rt_array_length(array) -> usize`.
    pub const ARRAY_LENGTH: &str = "zirk_rt_array_length";
    /// `zirk_rt_list_length(list) -> usize`.
    pub const LIST_LENGTH: &str = "zirk_rt_list_length";
    /// `zirk_rt_array_element(array, index, elem_size) -> *mut c_void`.
    pub const ARRAY_ELEMENT: &str = "zirk_rt_array_element";
    /// `zirk_rt_list_element(list, index, elem_size) -> *mut c_void`.
    pub const LIST_ELEMENT: &str = "zirk_rt_list_element";
    /// `zirk_rt_list_add(list, value_ptr, elem_size, elem_align, is_ref)`.
    pub const LIST_ADD: &str = "zirk_rt_list_add";
    /// `zirk_rt_list_insert(list, index, value_ptr, elem_size, elem_align, is_ref)`.
    pub const LIST_INSERT: &str = "zirk_rt_list_insert";
    /// `zirk_rt_list_remove(list, index, elem_size, elem_align, is_ref)`.
    pub const LIST_REMOVE: &str = "zirk_rt_list_remove";
    /// `zirk_rt_array_clone(array, elem_size, elem_align, is_ref) -> *mut c_void`.
    pub const ARRAY_CLONE: &str = "zirk_rt_array_clone";
    /// `zirk_rt_list_clone(list, elem_size, elem_align, is_ref) -> *mut c_void`.
    pub const LIST_CLONE: &str = "zirk_rt_list_clone";
    /// `zirk_rt_array_slice(array, start, end, step, elem_size, elem_align, is_ref) -> *mut c_void`.
    pub const ARRAY_SLICE: &str = "zirk_rt_array_slice";
    /// `zirk_rt_list_remove_value(list, value_ptr, elem_size, elem_align, is_ref) -> bool`.
    pub const LIST_REMOVE_VALUE: &str = "zirk_rt_list_remove_value";
    /// Reports an out-of-bounds index and terminates.
    pub const INDEX_OUT_OF_BOUNDS: &str = "zirk_rt_index_out_of_bounds";
}

/// The runtime functions available to generated code.
#[allow(dead_code)]
pub struct Runtime<'ctx> {
    pub init: FunctionValue<'ctx>,
    pub run_main: FunctionValue<'ctx>,
    pub spawn: FunctionValue<'ctx>,
    pub job_wait: FunctionValue<'ctx>,
    pub job_done: FunctionValue<'ctx>,
    pub parallel_for: FunctionValue<'ctx>,
    pub scope_enter: FunctionValue<'ctx>,
    pub scope_exit: FunctionValue<'ctx>,
    pub branch_register: FunctionValue<'ctx>,
    pub cancel: FunctionValue<'ctx>,
    pub timer_after: FunctionValue<'ctx>,
    pub timer_every: FunctionValue<'ctx>,
    pub sleep: FunctionValue<'ctx>,
    pub branch_fail: FunctionValue<'ctx>,
    pub shutdown: FunctionValue<'ctx>,
    pub str_from_utf8: FunctionValue<'ctx>,
    pub str_from_i8: FunctionValue<'ctx>,
    pub str_from_i16: FunctionValue<'ctx>,
    pub str_from_i32: FunctionValue<'ctx>,
    pub str_from_i64: FunctionValue<'ctx>,
    pub str_from_i128: FunctionValue<'ctx>,
    pub str_from_u8: FunctionValue<'ctx>,
    pub str_from_u16: FunctionValue<'ctx>,
    pub str_from_u32: FunctionValue<'ctx>,
    pub str_from_u64: FunctionValue<'ctx>,
    pub str_from_u128: FunctionValue<'ctx>,
    pub str_from_f32: FunctionValue<'ctx>,
    pub str_from_f64: FunctionValue<'ctx>,
    pub float_format: FunctionValue<'ctx>,
    pub str_from_bool: FunctionValue<'ctx>,
    pub str_grapheme_offset: FunctionValue<'ctx>,
    pub str_grapheme_len_at: FunctionValue<'ctx>,
    pub str_grapheme_slice: FunctionValue<'ctx>,
    pub str_eq: FunctionValue<'ctx>,
    pub io_println: FunctionValue<'ctx>,
    pub alloc: FunctionValue<'ctx>,
    pub contract_table: FunctionValue<'ctx>,
    pub str_concat: FunctionValue<'ctx>,
    pub str_repeat: FunctionValue<'ctx>,
    pub check_cast: FunctionValue<'ctx>,
    pub fatal_error: FunctionValue<'ctx>,
    pub throw: FunctionValue<'ctx>,
    pub stack_trace: FunctionValue<'ctx>,
    pub suppressed: FunctionValue<'ctx>,
    pub set_suppressed: FunctionValue<'ctx>,
    pub has_pending_exception: FunctionValue<'ctx>,
    pub take_pending_exception: FunctionValue<'ctx>,
    pub is_instance: FunctionValue<'ctx>,
    pub uncaught_exception: FunctionValue<'ctx>,
    pub push_frame: FunctionValue<'ctx>,
    pub pop_frame: FunctionValue<'ctx>,
    /// The WeakCell sentinel descriptor's own address (`fase-4e-weak`,
    /// design D2) — a global, not a function, unlike everything else here.
    pub weak_cell_descriptor: inkwell::values::PointerValue<'ctx>,
    /// The WeakCell "ever allocated" flag's own address (`fase-4e-weak`) —
    /// also a global: `WeakFrom` stores `1` into it directly, no call.
    pub weak_cell_ever_allocated: inkwell::values::PointerValue<'ctx>,
    /// `zirk_rt_clone` (roadmap Phase 4e, `fase-4e-clone`, design D2) — the
    /// whole deep-clone-graph traversal, one call per `.clone()` site.
    pub clone: FunctionValue<'ctx>,
    /// `zirk_rt_alloc_callable` (roadmap Phase 4d, `phase-4d-callables`).
    pub alloc_callable: FunctionValue<'ctx>,
    /// `zirk_rt_clone_callable` (roadmap Phase 4d, `phase-4d-callables`).
    pub clone_callable: FunctionValue<'ctx>,
    /// `zirk_rt_journal_begin` (roadmap Phase 4e, `fase-4e-unsafe-journal`,
    /// design D1).
    pub journal_begin: FunctionValue<'ctx>,
    /// `zirk_rt_journal_record` (design D1/D3).
    pub journal_record: FunctionValue<'ctx>,
    /// `zirk_rt_journal_commit` (design D1).
    pub journal_commit: FunctionValue<'ctx>,
    /// `zirk_rt_journal_rollback` (design D1/D2).
    pub journal_rollback: FunctionValue<'ctx>,
    /// `zirk_rt_native_slice_validate` (roadmap Phase 4e,
    /// `fase-4e-native-slice`, design D3/D5) — one call per
    /// `.as_slice(length)`/`.as_slice_mut(length)` construction.
    pub native_slice_validate: FunctionValue<'ctx>,
    /// `zirk_rt_is_cancelled` (roadmap Phase 4c, `phase-4c-resources`,
    /// design D5) — checked before resource close.
    pub is_cancelled: FunctionValue<'ctx>,
    /// `zirk_rt_resource_close_group` (roadmap Phase 4c,
    /// `phase-4c-resources`, design D1/D5) — grouped `match with` cleanup.
    pub resource_close_group: FunctionValue<'ctx>,
    /// `zirk_rt_resource_transfer` (roadmap Phase 4c,
    /// `phase-4c-resources`, design D3) — ownership transfer.
    pub resource_transfer: FunctionValue<'ctx>,
    /// `zirk_rt_pin_object` (roadmap Phase 4e, `phase-4e-memory`, design D1).
    pub pin_object: FunctionValue<'ctx>,
    /// `zirk_rt_unpin_object` (roadmap Phase 4e, `phase-4e-memory`, design D1).
    pub unpin_object: FunctionValue<'ctx>,
    /// `zirk_rt_dependent_base` (roadmap Phase 4e, `phase-4e-memory`, design
    /// D1): reads the base pointer from a `Dependent<T>` value.
    pub dependent_base: FunctionValue<'ctx>,
    /// `zirk_regex_from_pattern(pattern, len) -> *mut c_void`.
    pub regex_from_pattern: FunctionValue<'ctx>,
    /// `zirk_regex_is_match(handle, text, len) -> bool`.
    pub regex_is_match: FunctionValue<'ctx>,
    /// `zirk_regex_replace(handle, text, text_len, repl, repl_len, out_len) -> *mut u8`.
    pub regex_replace: FunctionValue<'ctx>,
    /// `zirk_regex_to_string(handle, out_len) -> *mut u8`.
    pub regex_to_string: FunctionValue<'ctx>,
    /// `zirk_regex_find(handle, text) -> {bool, *mut c_void}`.
    pub regex_find: FunctionValue<'ctx>,
    /// `zirk_regex_match_group_pos(match_object, n) -> *mut c_void`.
    pub regex_match_group_pos: FunctionValue<'ctx>,
    /// `zirk_regex_match_group_name(match_object, name) -> *mut c_void`.
    pub regex_match_group_name: FunctionValue<'ctx>,
    /// `zirk_str_trim(handle) -> *mut c_void`.
    pub str_trim: FunctionValue<'ctx>,
    /// `zirk_str_contains(handle, pat) -> bool`.
    pub str_contains: FunctionValue<'ctx>,
    /// `zirk_str_starts_with(handle, pat) -> bool`.
    pub str_starts_with: FunctionValue<'ctx>,
    /// `zirk_str_ends_with(handle, pat) -> bool`.
    pub str_ends_with: FunctionValue<'ctx>,
    /// `zirk_str_substring(handle, start, end) -> *mut c_void`.
    pub str_substring: FunctionValue<'ctx>,
    /// `zirk_str_search(handle, pat) -> i64`.
    pub str_search: FunctionValue<'ctx>,
    /// `zirk_char_is_uppercase(c) -> bool`.
    pub char_is_uppercase: FunctionValue<'ctx>,
    /// `zirk_char_is_lowercase(c) -> bool`.
    pub char_is_lowercase: FunctionValue<'ctx>,
    /// `zirk_char_is_digit(c) -> bool`.
    pub char_is_digit: FunctionValue<'ctx>,
    /// `zirk_char_is_letter(c) -> bool`.
    pub char_is_letter: FunctionValue<'ctx>,
    /// `zirk_char_is_whitespace(c) -> bool`.
    pub char_is_whitespace: FunctionValue<'ctx>,
    /// `zirk_char_to_uppercase(c) -> *mut c_void`.
    pub char_to_uppercase: FunctionValue<'ctx>,
    /// `zirk_char_to_lowercase(c) -> *mut c_void`.
    pub char_to_lowercase: FunctionValue<'ctx>,

    pub array_new: FunctionValue<'ctx>,
    pub list_new: FunctionValue<'ctx>,
    pub map_new: FunctionValue<'ctx>,
    pub map_length: FunctionValue<'ctx>,
    pub map_is_empty: FunctionValue<'ctx>,
    pub map_set: FunctionValue<'ctx>,
    pub map_contains_key: FunctionValue<'ctx>,
    pub map_get: FunctionValue<'ctx>,
    pub map_remove: FunctionValue<'ctx>,
    pub set_new: FunctionValue<'ctx>,
    pub set_length: FunctionValue<'ctx>,
    pub set_is_empty: FunctionValue<'ctx>,
    pub set_add: FunctionValue<'ctx>,
    pub set_contains: FunctionValue<'ctx>,
    pub set_remove: FunctionValue<'ctx>,
    pub array_length: FunctionValue<'ctx>,
    pub list_length: FunctionValue<'ctx>,
    pub array_element: FunctionValue<'ctx>,
    pub list_element: FunctionValue<'ctx>,
    pub list_add: FunctionValue<'ctx>,
    pub list_insert: FunctionValue<'ctx>,
    pub list_remove: FunctionValue<'ctx>,
    pub array_clone: FunctionValue<'ctx>,
    pub list_clone: FunctionValue<'ctx>,
    pub array_slice: FunctionValue<'ctx>,
    pub list_remove_value: FunctionValue<'ctx>,
    pub index_out_of_bounds: FunctionValue<'ctx>,
}

/// Declares every runtime symbol in the module.
pub fn declare<'ctx>(context: &'ctx Context, module: &Module<'ctx>) -> Runtime<'ctx> {
    let void = context.void_type();
    let i64 = context.i64_type();
    let ptr = context.ptr_type(AddressSpace::default());

    let external = Some(Linkage::External);

    let init = module.add_function(symbols::INIT, void.fn_type(&[], false), external);
    let run_main = module.add_function(
        symbols::RUN_MAIN,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let spawn = module.add_function(
        symbols::SPAWN,
        i64.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let job_wait = module.add_function(
        symbols::JOB_WAIT,
        i64.fn_type(&[i64.into()], false),
        external,
    );
    let job_done = module.add_function(
        symbols::JOB_DONE,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let parallel_for = module.add_function(
        symbols::PARALLEL_FOR,
        void.fn_type(
            &[
                context.i8_type().into(),
                i64.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let scope_enter = module.add_function(symbols::SCOPE_ENTER, i64.fn_type(&[], false), external);
    let scope_exit = module.add_function(
        symbols::SCOPE_EXIT,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let branch_register = module.add_function(
        symbols::BRANCH_REGISTER,
        void.fn_type(&[i64.into(), i64.into()], false),
        external,
    );
    let cancel = module.add_function(
        symbols::CANCEL,
        void.fn_type(&[i64.into()], false),
        external,
    );
    let timer_after = module.add_function(
        symbols::TIMER_AFTER,
        i64.fn_type(&[i64.into(), i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let timer_every = module.add_function(
        symbols::TIMER_EVERY,
        i64.fn_type(&[i64.into(), i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let sleep = module.add_function(symbols::SLEEP, void.fn_type(&[i64.into()], false), external);
    let branch_fail = module.add_function(
        symbols::BRANCH_FAIL,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let shutdown = module.add_function(symbols::SHUTDOWN, void.fn_type(&[], false), external);

    let str_from_utf8 = module.add_function(
        symbols::STR_FROM_UTF8,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );

    let str_from_i8 = module.add_function(
        symbols::STR_FROM_I8,
        ptr.fn_type(&[context.i8_type().into()], false),
        external,
    );
    let str_from_i16 = module.add_function(
        symbols::STR_FROM_I16,
        ptr.fn_type(&[context.i16_type().into()], false),
        external,
    );
    let str_from_i32 = module.add_function(
        symbols::STR_FROM_I32,
        ptr.fn_type(&[context.i32_type().into()], false),
        external,
    );
    let str_from_i64 = module.add_function(
        symbols::STR_FROM_I64,
        ptr.fn_type(&[context.i64_type().into()], false),
        external,
    );
    // Taken by pointer: no stable cross-target ABI for a by-value 128-bit
    // integer (`zirk-runtime/src/string.rs`'s own doc comment on the
    // handler explains why).
    let str_from_i128 = module.add_function(
        symbols::STR_FROM_I128,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let str_from_u8 = module.add_function(
        symbols::STR_FROM_U8,
        ptr.fn_type(&[context.i8_type().into()], false),
        external,
    );
    let str_from_u16 = module.add_function(
        symbols::STR_FROM_U16,
        ptr.fn_type(&[context.i16_type().into()], false),
        external,
    );
    let str_from_u32 = module.add_function(
        symbols::STR_FROM_U32,
        ptr.fn_type(&[context.i32_type().into()], false),
        external,
    );
    let str_from_u64 = module.add_function(
        symbols::STR_FROM_U64,
        ptr.fn_type(&[context.i64_type().into()], false),
        external,
    );
    let str_from_u128 = module.add_function(
        symbols::STR_FROM_U128,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let str_from_f32 = module.add_function(
        symbols::STR_FROM_F32,
        ptr.fn_type(&[context.f32_type().into()], false),
        external,
    );
    let str_from_f64 = module.add_function(
        symbols::STR_FROM_F64,
        ptr.fn_type(&[context.f64_type().into()], false),
        external,
    );
    let float_format = module.add_function(
        symbols::FLOAT_FORMAT,
        ptr.fn_type(&[context.f64_type().into(), ptr.into()], false),
        external,
    );

    let str_from_bool = module.add_function(
        symbols::STR_FROM_BOOL,
        ptr.fn_type(&[context.bool_type().into()], false),
        external,
    );

    let str_grapheme_offset = module.add_function(
        symbols::STR_GRAPHEME_OFFSET,
        i64.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let str_grapheme_len_at = module.add_function(
        symbols::STR_GRAPHEME_LEN_AT,
        i64.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let str_grapheme_slice = module.add_function(
        symbols::STR_GRAPHEME_SLICE,
        ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
        external,
    );

    let str_eq = module.add_function(
        symbols::STR_EQ,
        context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );

    let io_println = module.add_function(
        symbols::IO_PRINTLN,
        void.fn_type(&[ptr.into()], false),
        external,
    );

    // The failure handlers never return: marking them `noreturn` lets LLVM
    // treat the code after them as unreachable and optimize accordingly.
    let division_by_zero = module.add_function(
        symbols::DIVISION_BY_ZERO,
        void.fn_type(&[], false),
        external,
    );

    let alloc = module.add_function(
        symbols::ALLOC,
        ptr.fn_type(&[i64.into(), i64.into()], false),
        external,
    );

    let clone = module.add_function(symbols::CLONE, ptr.fn_type(&[ptr.into()], false), external);

    // Phase 4d boxed callables: capture-block allocation and cloning are
    // declared with C ABI linkage; the generated code does not yet emit calls
    // to them (shortcut documented in `phase-4d-callables/tasks.md`).
    let alloc_callable = module.add_function(
        symbols::ALLOC_CALLABLE,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let clone_callable = module.add_function(
        symbols::CLONE_CALLABLE,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );

    let journal_begin =
        module.add_function(symbols::JOURNAL_BEGIN, ptr.fn_type(&[], false), external);
    let journal_record = module.add_function(
        symbols::JOURNAL_RECORD,
        void.fn_type(&[ptr.into(), ptr.into(), i64.into()], false),
        external,
    );
    let journal_commit = module.add_function(
        symbols::JOURNAL_COMMIT,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let journal_rollback = module.add_function(
        symbols::JOURNAL_ROLLBACK,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    // `zirk_rt_native_slice_validate(pointer, length, elem_size, elem_align,
    // known_length) -> bool` (roadmap Phase 4e, `fase-4e-native-slice`,
    // design D3/D5): `known_length` is `i64` with `-1` meaning "opaque
    // provenance, trusted" (`InstKind::NativeSliceValidate`'s own doc
    // comment) — a signed width is what makes that sentinel representable
    // without a separate "is this known" flag.
    let native_slice_validate = module.add_function(
        symbols::NATIVE_SLICE_VALIDATE,
        context.bool_type().fn_type(
            &[ptr.into(), i64.into(), i64.into(), i64.into(), i64.into()],
            false,
        ),
        external,
    );
    // `zirk_rt_is_cancelled() -> bool` (roadmap Phase 4c,
    // `phase-4c-resources`, design D5): cancellation-aware cleanup guardrail.
    let is_cancelled = module.add_function(
        symbols::IS_CANCELLED,
        context.bool_type().fn_type(&[], false),
        external,
    );
    // `zirk_rt_resource_close_group(resources, close) -> *mut c_void`
    // (roadmap Phase 4c, `phase-4c-resources`, design D1/D5): right-to-left
    // grouped cleanup; the stub returns `null` and does not block.
    let resource_close_group = module.add_function(
        symbols::RESOURCE_CLOSE_GROUP,
        ptr.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    // `zirk_rt_resource_transfer(resource) -> *mut c_void` (roadmap Phase 4c,
    // `phase-4c-resources`, design D3): ownership transfer stub.
    let resource_transfer = module.add_function(
        symbols::RESOURCE_TRANSFER,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    // `zirk_rt_pin_object(object)` / `zirk_rt_unpin_object(object)` (roadmap
    // Phase 4e, `phase-4e-memory`, design D1): per-thread pin list stubs.
    let pin_object = module.add_function(
        symbols::PIN_OBJECT,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let unpin_object = module.add_function(
        symbols::UNPIN_OBJECT,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    // `zirk_rt_dependent_base(dependent) -> *mut c_void` (roadmap Phase 4e,
    // `phase-4e-memory`, design D1): reads the base object from a
    // `Dependent<T>` value. Surface-only stub.
    let dependent_base = module.add_function(
        symbols::DEPENDENT_BASE,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    // `Regex` runtime helpers: opaque handles, matching, replacement and
    // pattern-to-string conversion.  See `crates/zirk-runtime/src/regex.rs`.
    let regex_from_pattern = module.add_function(
        symbols::REGEX_FROM_PATTERN,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let regex_is_match = module.add_function(
        symbols::REGEX_IS_MATCH,
        context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let regex_replace = module.add_function(
        symbols::REGEX_REPLACE,
        ptr.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let regex_to_string = module.add_function(
        symbols::REGEX_TO_STRING,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    // `Regex.find` returns a `Regex.Match?` object, represented as a
    // present flag followed by an opaque object pointer — the same shape as
    // every other nullable object/reference type.
    let nullable_object_ty = context.struct_type(&[context.bool_type().into(), ptr.into()], false);
    let regex_find = module.add_function(
        symbols::REGEX_FIND,
        nullable_object_ty.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let regex_match_group_pos = module.add_function(
        symbols::REGEX_MATCH_GROUP_POS,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let regex_match_group_name = module.add_function(
        symbols::REGEX_MATCH_GROUP_NAME,
        ptr.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    // `String` built-in methods (roadmap Phase 7, `String` ops).
    let str_trim = module.add_function(
        symbols::STR_TRIM,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let str_contains = module.add_function(
        symbols::STR_CONTAINS,
        context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let str_starts_with = module.add_function(
        symbols::STR_STARTS_WITH,
        context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let str_ends_with = module.add_function(
        symbols::STR_ENDS_WITH,
        context
            .bool_type()
            .fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let str_substring = module.add_function(
        symbols::STR_SUBSTRING,
        ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
        external,
    );
    let str_search = module.add_function(
        symbols::STR_SEARCH,
        i64.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    // `Char` built-in classification and normalization.
    let char_is_uppercase = module.add_function(
        symbols::CHAR_IS_UPPERCASE,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let char_is_lowercase = module.add_function(
        symbols::CHAR_IS_LOWERCASE,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let char_is_digit = module.add_function(
        symbols::CHAR_IS_DIGIT,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let char_is_letter = module.add_function(
        symbols::CHAR_IS_LETTER,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let char_is_whitespace = module.add_function(
        symbols::CHAR_IS_WHITESPACE,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let char_to_uppercase = module.add_function(
        symbols::CHAR_TO_UPPERCASE,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let char_to_lowercase = module.add_function(
        symbols::CHAR_TO_LOWERCASE,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );

    // `Array<T>` / `List<T>` runtime helpers.
    let array_new = module.add_function(
        symbols::ARRAY_NEW,
        ptr.fn_type(
            &[
                i64.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let list_new = module.add_function(
        symbols::LIST_NEW,
        ptr.fn_type(&[i64.into(), i64.into(), context.bool_type().into()], false),
        external,
    );
    let map_new = module.add_function(symbols::MAP_NEW, ptr.fn_type(&[], false), external);
    let map_length = module.add_function(
        symbols::MAP_LENGTH,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let map_is_empty = module.add_function(
        symbols::MAP_IS_EMPTY,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let map_set = module.add_function(
        symbols::MAP_SET,
        void.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
        external,
    );
    let map_contains_key = module.add_function(
        symbols::MAP_CONTAINS_KEY,
        context
            .bool_type()
            .fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let map_get = module.add_function(
        symbols::MAP_GET,
        i64.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let map_remove = module.add_function(
        symbols::MAP_REMOVE,
        context
            .bool_type()
            .fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let set_new = module.add_function(symbols::SET_NEW, ptr.fn_type(&[], false), external);
    let set_length = module.add_function(
        symbols::SET_LENGTH,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let set_is_empty = module.add_function(
        symbols::SET_IS_EMPTY,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let set_add = module.add_function(
        symbols::SET_ADD,
        void.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let set_contains = module.add_function(
        symbols::SET_CONTAINS,
        context
            .bool_type()
            .fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let set_remove = module.add_function(
        symbols::SET_REMOVE,
        context
            .bool_type()
            .fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let array_length = module.add_function(
        symbols::ARRAY_LENGTH,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let list_length = module.add_function(
        symbols::LIST_LENGTH,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let array_element = module.add_function(
        symbols::ARRAY_ELEMENT,
        ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
        external,
    );
    let list_element = module.add_function(
        symbols::LIST_ELEMENT,
        ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
        external,
    );
    let list_add = module.add_function(
        symbols::LIST_ADD,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let list_insert = module.add_function(
        symbols::LIST_INSERT,
        void.fn_type(
            &[
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let list_remove = module.add_function(
        symbols::LIST_REMOVE,
        void.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let array_clone = module.add_function(
        symbols::ARRAY_CLONE,
        ptr.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let list_clone = module.add_function(
        symbols::LIST_CLONE,
        ptr.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let array_slice = module.add_function(
        symbols::ARRAY_SLICE,
        ptr.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let list_remove_value = module.add_function(
        symbols::LIST_REMOVE_VALUE,
        context.bool_type().fn_type(
            &[
                ptr.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let index_out_of_bounds = module.add_function(
        symbols::INDEX_OUT_OF_BOUNDS,
        void.fn_type(&[], false),
        external,
    );

    // Declared so the allocator can reach it, and marked `noreturn` with the
    // rest: generated code never calls it directly, the runtime does.
    let allocation_failed = module.add_function(
        symbols::ALLOCATION_FAILED,
        void.fn_type(&[], false),
        external,
    );

    let str_concat = module.add_function(
        symbols::STR_CONCAT,
        ptr.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let str_repeat = module.add_function(
        symbols::STR_REPEAT,
        ptr.fn_type(&[ptr.into(), context.i32_type().into()], false),
        external,
    );
    let invalid_repeat =
        module.add_function(symbols::INVALID_REPEAT, void.fn_type(&[], false), external);
    let invalid_shift =
        module.add_function(symbols::INVALID_SHIFT, void.fn_type(&[], false), external);

    let contract_table = module.add_function(
        symbols::CONTRACT_TABLE,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let missing_contract = module.add_function(
        symbols::MISSING_CONTRACT,
        void.fn_type(&[], false),
        external,
    );

    let check_cast = module.add_function(
        symbols::CHECK_CAST,
        void.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let invalid_cast =
        module.add_function(symbols::INVALID_CAST, void.fn_type(&[], false), external);
    let float_nan = module.add_function(symbols::FLOAT_NAN, void.fn_type(&[], false), external);
    let fatal_error = module.add_function(
        symbols::FATAL_ERROR,
        void.fn_type(&[ptr.into()], false),
        external,
    );

    let throw = module.add_function(symbols::THROW, void.fn_type(&[ptr.into()], false), external);
    let stack_trace = module.add_function(
        symbols::STACK_TRACE,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let suppressed = module.add_function(
        symbols::SUPPRESSED,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let set_suppressed = module.add_function(
        symbols::SET_SUPPRESSED,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let bool_ty = context.bool_type();
    let has_pending_exception = module.add_function(
        symbols::HAS_PENDING_EXCEPTION,
        bool_ty.fn_type(&[], false),
        external,
    );
    let take_pending_exception = module.add_function(
        symbols::TAKE_PENDING_EXCEPTION,
        ptr.fn_type(&[], false),
        external,
    );
    let is_instance = module.add_function(
        symbols::IS_INSTANCE,
        bool_ty.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let uncaught_exception = module.add_function(
        symbols::UNCAUGHT_EXCEPTION,
        void.fn_type(&[], false),
        external,
    );

    // `roots`: the address of an array of root addresses (design D2) — each
    // element is itself the address of a slot (or a slot's inner
    // reference-typed field) holding a managed reference, not the reference
    // itself, so the collector dereferences once more to reach the candidate
    // object. `count`: how many elements that array has.
    let push_frame = module.add_function(
        symbols::PUSH_FRAME,
        void.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let pop_frame = module.add_function(symbols::POP_FRAME, void.fn_type(&[], false), external);

    // Both globals (`fase-4e-weak`, design D2 and its risk mitigation): an
    // `i8`, external linkage, no initializer here — `zirk-runtime` owns the
    // one real definition, resolved at link time like every other symbol in
    // this module.
    let weak_cell_descriptor =
        module.add_global(context.i8_type(), None, symbols::WEAK_CELL_DESCRIPTOR);
    weak_cell_descriptor.set_linkage(Linkage::External);
    let weak_cell_ever_allocated =
        module.add_global(context.i8_type(), None, symbols::WEAK_CELL_EVER_ALLOCATED);
    weak_cell_ever_allocated.set_linkage(Linkage::External);

    for handler in [
        division_by_zero,
        allocation_failed,
        missing_contract,
        invalid_repeat,
        invalid_cast,
        invalid_shift,
        float_nan,
        fatal_error,
        uncaught_exception,
        index_out_of_bounds,
    ] {
        let noreturn = context.create_enum_attribute(
            inkwell::attributes::Attribute::get_named_enum_kind_id("noreturn"),
            0,
        );
        handler.add_attribute(inkwell::attributes::AttributeLoc::Function, noreturn);
    }

    Runtime {
        init,
        run_main,
        spawn,
        job_wait,
        job_done,
        parallel_for,
        scope_enter,
        scope_exit,
        branch_register,
        cancel,
        timer_after,
        timer_every,
        sleep,
        branch_fail,
        shutdown,
        str_from_utf8,
        str_from_i8,
        str_from_i16,
        str_from_i32,
        str_from_i64,
        str_from_i128,
        str_from_u8,
        str_from_u16,
        str_from_u32,
        str_from_u64,
        str_from_u128,
        str_from_f32,
        str_from_f64,
        float_format,
        str_from_bool,
        str_grapheme_offset,
        str_grapheme_len_at,
        str_grapheme_slice,
        str_eq,
        io_println,
        alloc,
        contract_table,
        str_concat,
        str_repeat,
        check_cast,
        fatal_error,
        throw,
        stack_trace,
        suppressed,
        set_suppressed,
        has_pending_exception,
        take_pending_exception,
        is_instance,
        uncaught_exception,
        push_frame,
        pop_frame,
        weak_cell_descriptor: weak_cell_descriptor.as_pointer_value(),
        weak_cell_ever_allocated: weak_cell_ever_allocated.as_pointer_value(),
        clone,
        alloc_callable,
        clone_callable,
        journal_begin,
        journal_record,
        journal_commit,
        journal_rollback,
        native_slice_validate,
        is_cancelled,
        resource_close_group,
        resource_transfer,
        pin_object,
        unpin_object,
        dependent_base,
        regex_from_pattern,
        regex_is_match,
        regex_replace,
        regex_to_string,
        regex_find,
        regex_match_group_pos,
        regex_match_group_name,
        str_trim,
        str_contains,
        str_starts_with,
        str_ends_with,
        str_substring,
        str_search,
        char_is_uppercase,
        char_is_lowercase,
        char_is_digit,
        char_is_letter,
        char_is_whitespace,
        char_to_uppercase,
        char_to_lowercase,

        array_new,
        list_new,
        map_new,
        map_length,
        map_is_empty,
        map_set,
        map_contains_key,
        map_get,
        map_remove,
        set_new,
        set_length,
        set_is_empty,
        set_add,
        set_contains,
        set_remove,
        array_length,
        list_length,
        array_element,
        list_element,
        list_add,
        list_insert,
        list_remove,
        array_clone,
        list_clone,
        array_slice,
        list_remove_value,
        index_out_of_bounds,
    }
}
