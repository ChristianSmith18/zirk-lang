//! Per-`unsafe`-block undo log (roadmap Phase 4e, `fase-4e-unsafe-journal`,
//! design D1): the transactional journal/rollback contract
//! `fase-4e-unsafe-pointer-extern` deferred as its own D5/D6.
//!
//! A [`Journal`] is an ordinary Rust heap allocation, never collector-tracked
//! (the same category as `Weak<T>`'s WeakCell context or `fase-4e-clone`'s
//! memoization table, `crates/zirk-codegen-llvm/src/emit.rs`'s own doc
//! comment on `InstKind::JournalBegin`): codegen never spills its handle to a
//! GC root, and nothing here ever touches `crate::collector`.
//!
//! `zirk_rt_journal_record` snapshots `len` bytes at `address` into the log
//! *before* the write it guards executes. `zirk_rt_journal_commit` discards
//! the log without restoring — the durable case, used both at an `unsafe {}`
//! block's own normal fall-through exit and at `commit {}`'s own entry.
//! `zirk_rt_journal_rollback` restores every recorded snapshot in reverse
//! order, then discards the log — the failure case, taken instead when an
//! exception is pending at the block's own exit check point. Reverse-order
//! restoration is what makes several records against the *same* address
//! resolve correctly to the original, pre-block value rather than an
//! intermediate one (roadmap tasks.md 1.2).

use std::ffi::c_void;

/// One block's own undo log: every snapshot recorded so far, in the order
/// `zirk_rt_journal_record` was called.
struct Journal {
    records: Vec<Record>,
}

/// One recorded write: the address it guards, and the bytes that lived there
/// immediately before the write it guards executed.
struct Record {
    address: *mut u8,
    before: Vec<u8>,
}

/// Begins a new undo log (roadmap Phase 4e, `fase-4e-unsafe-journal`, design
/// D1) — `unsafe { ... }`'s own entry.
///
/// # Safety
///
/// The returned handle must be passed to exactly one of
/// [`zirk_rt_journal_commit`]/[`zirk_rt_journal_rollback`], and to no
/// [`zirk_rt_journal_record`] call after that.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_journal_begin() -> *mut c_void {
    Box::into_raw(Box::new(Journal {
        records: Vec::new(),
    })) as *mut c_void
}

/// Snapshots `len` bytes at `address` into `journal`'s undo log, immediately
/// before the write it guards executes (design D1/D3).
///
/// # Safety
///
/// `journal` must be a live handle from [`zirk_rt_journal_begin`], not yet
/// passed to [`zirk_rt_journal_commit`]/[`zirk_rt_journal_rollback`].
/// `address` must be valid for `len` bytes of reads (for this call) and, if
/// this journal is later rolled back, of writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_journal_record(
    journal: *mut c_void,
    address: *mut u8,
    len: usize,
) {
    let journal = unsafe { &mut *(journal as *mut Journal) };
    let mut before = vec![0u8; len];
    if len > 0 {
        unsafe { std::ptr::copy_nonoverlapping(address, before.as_mut_ptr(), len) };
    }
    journal.records.push(Record { address, before });
}

/// Snapshots a stable String handle's backing-reference field before indexed
/// mutation. Restoring this one pointer makes every alias observe the text
/// that existed at the enclosing `unsafe` block's entry.
///
/// # Safety
///
/// `journal` must be live and `string` must be a live runtime String handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_journal_record_string_backing(
    journal: *mut c_void,
    string: *mut c_void,
) {
    let address = unsafe {
        crate::string::payload_ptr(string)
            .cast::<u8>()
            .add(crate::string::ZirkString::BACKING_OFFSET)
    };
    unsafe { zirk_rt_journal_record(journal, address, std::mem::size_of::<*mut c_void>()) };
}

/// Durably commits `journal`: discards the undo log without restoring, and
/// frees the journal itself (design D1) — the success path.
///
/// # Safety
///
/// `journal` must be a live handle from [`zirk_rt_journal_begin`], not
/// already committed or rolled back. It must not be used again after this
/// call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_journal_commit(journal: *mut c_void) {
    drop(unsafe { Box::from_raw(journal as *mut Journal) });
}

/// Rolls `journal` back: restores every recorded snapshot in reverse order,
/// then discards and frees the log (design D1/D2) — the failure path.
///
/// # Safety
///
/// Same obligations as [`zirk_rt_journal_commit`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_journal_rollback(journal: *mut c_void) {
    let journal = unsafe { Box::from_raw(journal as *mut Journal) };
    for record in journal.records.into_iter().rev() {
        if !record.before.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    record.before.as_ptr(),
                    record.address,
                    record.before.len(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_then_commit_leaves_memory_unchanged() {
        let mut value: u32 = 111;
        unsafe {
            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_record(journal, (&mut value as *mut u32).cast(), 4);
            value = 222;
            zirk_rt_journal_commit(journal);
        }
        assert_eq!(value, 222, "a commit must never restore anything");
    }

    #[test]
    fn record_then_rollback_restores_the_original_bytes() {
        let mut value: u32 = 111;
        unsafe {
            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_record(journal, (&mut value as *mut u32).cast(), 4);
            value = 222;
            zirk_rt_journal_rollback(journal);
        }
        assert_eq!(value, 111, "a rollback must restore the pre-record value");
    }

    #[test]
    fn multiple_records_to_different_addresses_roll_back_all_of_them() {
        let mut a: u32 = 1;
        let mut b: u32 = 2;
        unsafe {
            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_record(journal, (&mut a as *mut u32).cast(), 4);
            a = 100;
            zirk_rt_journal_record(journal, (&mut b as *mut u32).cast(), 4);
            b = 200;
            zirk_rt_journal_rollback(journal);
        }
        assert_eq!(a, 1);
        assert_eq!(b, 2);
    }

    /// Two records against the *same* address roll back correctly to the
    /// original pre-block value, not an intermediate one — the reason
    /// restoration must run in reverse order (roadmap tasks.md 1.2): the
    /// second record's own "before" snapshot is the intermediate value, so
    /// restoring it *first* (i.e. processing records in forward order)
    /// would leave the intermediate value in place once the first record's
    /// own (correct, original) snapshot was applied and then overwritten.
    #[test]
    fn multiple_records_to_the_same_address_roll_back_to_the_original_value() {
        let mut value: u32 = 1;
        unsafe {
            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_record(journal, (&mut value as *mut u32).cast(), 4);
            value = 2;
            zirk_rt_journal_record(journal, (&mut value as *mut u32).cast(), 4);
            value = 3;
            zirk_rt_journal_rollback(journal);
        }
        assert_eq!(
            value, 1,
            "rollback must reach the original pre-block value, not the \
             intermediate one"
        );
    }

    #[test]
    fn a_journal_with_no_records_commits_and_rolls_back_without_effect() {
        unsafe {
            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_commit(journal);

            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_rollback(journal);
        }
    }

    #[test]
    fn string_backing_record_restores_a_shared_string_on_rollback() {
        let text = crate::string::alloc_owned("abc");
        let replacement = crate::string::alloc_owned("X");
        unsafe {
            let journal = zirk_rt_journal_begin();
            zirk_rt_journal_record_string_backing(journal, text);
            let offset = crate::string::zirk_str_grapheme_offset(text, 1);
            let len = crate::string::zirk_str_grapheme_len_at(text, offset);
            crate::string::zirk_str_set(text, offset, len, replacement);
            assert_eq!(crate::string::borrow(text).map(|s| s.as_str()), Some("aXc"));
            zirk_rt_journal_rollback(journal);
        }
        assert_eq!(
            unsafe { crate::string::borrow(text) }.map(|s| unsafe { s.as_str() }),
            Some("abc")
        );
    }
}
