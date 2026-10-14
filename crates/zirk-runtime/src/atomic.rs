//! Portable scalar atomic foundation.
//!
//! Only bool and signed/unsigned integers through 64 bits are accepted. The
//! value stored by an atomic is always inline scalar bits; pointers, including
//! managed heap pointers, have no representation in [`AtomicCell`].
//!
//! Loads, stores, exchanges, read-modify-write operations, and the default
//! compare-exchange use sequential consistency. `fetch_add` and `fetch_sub`
//! wrap modulo the selected width, matching Rust's atomic integer operations.
//! No lock-free guarantee is made. A target/width pair is supported only when
//! Rust exposes the corresponding `target_has_atomic` capability.

use std::ffi::c_void;
use std::sync::atomic::Ordering;

/// Scalar type stored in an [`AtomicCell`] and named at the C ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AtomicKind {
    Bool = 0,
    I8 = 1,
    U8 = 2,
    I16 = 3,
    U16 = 4,
    I32 = 5,
    U32 = 6,
    I64 = 7,
    U64 = 8,
}

/// Ordering accepted by the explicit compare-exchange API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AtomicOrder {
    Relaxed = 0,
    Acquire = 1,
    Release = 2,
    AcquireRelease = 3,
    SequentiallyConsistent = 4,
}

impl TryFrom<u32> for AtomicOrder {
    type Error = AtomicError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Relaxed),
            1 => Ok(Self::Acquire),
            2 => Ok(Self::Release),
            3 => Ok(Self::AcquireRelease),
            4 => Ok(Self::SequentiallyConsistent),
            _ => Err(AtomicError::InvalidOrdering),
        }
    }
}

impl From<AtomicOrder> for Ordering {
    fn from(value: AtomicOrder) -> Self {
        match value {
            AtomicOrder::Relaxed => Self::Relaxed,
            AtomicOrder::Acquire => Self::Acquire,
            AtomicOrder::Release => Self::Release,
            AtomicOrder::AcquireRelease => Self::AcqRel,
            AtomicOrder::SequentiallyConsistent => Self::SeqCst,
        }
    }
}

/// Whether an ordering is legal for an atomic load.
pub const fn valid_load_ordering(order: AtomicOrder) -> bool {
    matches!(
        order,
        AtomicOrder::Relaxed | AtomicOrder::Acquire | AtomicOrder::SequentiallyConsistent
    )
}

/// Whether an ordering is legal for an atomic store.
pub const fn valid_store_ordering(order: AtomicOrder) -> bool {
    matches!(
        order,
        AtomicOrder::Relaxed | AtomicOrder::Release | AtomicOrder::SequentiallyConsistent
    )
}

/// Failures returned by safe constructors and operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicError {
    UnsupportedWidthOrTarget,
    InvalidOrdering,
    TypeMismatch,
}

/// Returns whether this scalar width is implemented natively by this target.
/// The runtime does not emulate unsupported widths with locks.
pub const fn supports(kind: AtomicKind) -> bool {
    match kind {
        AtomicKind::Bool | AtomicKind::I8 | AtomicKind::U8 => cfg!(target_has_atomic = "8"),
        AtomicKind::I16 | AtomicKind::U16 => cfg!(target_has_atomic = "16"),
        AtomicKind::I32 | AtomicKind::U32 => cfg!(target_has_atomic = "32"),
        AtomicKind::I64 | AtomicKind::U64 => cfg!(target_has_atomic = "64"),
    }
}

/// Check the Rust compare-exchange success/failure ordering constraints.
/// Failure ordering may never be Release or AcqRel and cannot be stronger than
/// success ordering.
pub const fn valid_compare_exchange_orderings(success: AtomicOrder, failure: AtomicOrder) -> bool {
    match (success, failure) {
        (_, AtomicOrder::Release | AtomicOrder::AcquireRelease) => false,
        (AtomicOrder::Relaxed | AtomicOrder::Release, AtomicOrder::Relaxed) => true,
        (
            AtomicOrder::Acquire | AtomicOrder::AcquireRelease,
            AtomicOrder::Relaxed | AtomicOrder::Acquire,
        ) => true,
        (
            AtomicOrder::SequentiallyConsistent,
            AtomicOrder::Relaxed | AtomicOrder::Acquire | AtomicOrder::SequentiallyConsistent,
        ) => true,
        _ => false,
    }
}

/// A type-erased but width-checked scalar atomic. It is naturally aligned by
/// the contained standard-library atomic type. Construction is fallible for
/// unsupported target widths; callers never pass storage or alignment by hand.
pub enum AtomicCell {
    #[cfg(target_has_atomic = "8")]
    Bool(std::sync::atomic::AtomicBool),
    #[cfg(target_has_atomic = "8")]
    I8(std::sync::atomic::AtomicI8),
    #[cfg(target_has_atomic = "8")]
    U8(std::sync::atomic::AtomicU8),
    #[cfg(target_has_atomic = "16")]
    I16(std::sync::atomic::AtomicI16),
    #[cfg(target_has_atomic = "16")]
    U16(std::sync::atomic::AtomicU16),
    #[cfg(target_has_atomic = "32")]
    I32(std::sync::atomic::AtomicI32),
    #[cfg(target_has_atomic = "32")]
    U32(std::sync::atomic::AtomicU32),
    #[cfg(target_has_atomic = "64")]
    I64(std::sync::atomic::AtomicI64),
    #[cfg(target_has_atomic = "64")]
    U64(std::sync::atomic::AtomicU64),
}

// Constructors are written directly below so cfg-gated variants remain
// exhaustive on targets that lack one or more widths.
impl AtomicCell {
    pub fn new(kind: AtomicKind, value: u64) -> Result<Self, AtomicError> {
        if !supports(kind) {
            return Err(AtomicError::UnsupportedWidthOrTarget);
        }
        #[allow(unreachable_patterns)]
        match kind {
            #[cfg(target_has_atomic = "8")]
            AtomicKind::Bool => Ok(Self::Bool(std::sync::atomic::AtomicBool::new(value != 0))),
            #[cfg(target_has_atomic = "8")]
            AtomicKind::I8 => Ok(Self::I8(std::sync::atomic::AtomicI8::new(value as i8))),
            #[cfg(target_has_atomic = "8")]
            AtomicKind::U8 => Ok(Self::U8(std::sync::atomic::AtomicU8::new(value as u8))),
            #[cfg(target_has_atomic = "16")]
            AtomicKind::I16 => Ok(Self::I16(std::sync::atomic::AtomicI16::new(value as i16))),
            #[cfg(target_has_atomic = "16")]
            AtomicKind::U16 => Ok(Self::U16(std::sync::atomic::AtomicU16::new(value as u16))),
            #[cfg(target_has_atomic = "32")]
            AtomicKind::I32 => Ok(Self::I32(std::sync::atomic::AtomicI32::new(value as i32))),
            #[cfg(target_has_atomic = "32")]
            AtomicKind::U32 => Ok(Self::U32(std::sync::atomic::AtomicU32::new(value as u32))),
            #[cfg(target_has_atomic = "64")]
            AtomicKind::I64 => Ok(Self::I64(std::sync::atomic::AtomicI64::new(value as i64))),
            #[cfg(target_has_atomic = "64")]
            AtomicKind::U64 => Ok(Self::U64(std::sync::atomic::AtomicU64::new(value))),
            _ => Err(AtomicError::UnsupportedWidthOrTarget),
        }
    }

    pub fn kind(&self) -> AtomicKind {
        match self {
            #[cfg(target_has_atomic = "8")]
            Self::Bool(_) => AtomicKind::Bool,
            #[cfg(target_has_atomic = "8")]
            Self::I8(_) => AtomicKind::I8,
            #[cfg(target_has_atomic = "8")]
            Self::U8(_) => AtomicKind::U8,
            #[cfg(target_has_atomic = "16")]
            Self::I16(_) => AtomicKind::I16,
            #[cfg(target_has_atomic = "16")]
            Self::U16(_) => AtomicKind::U16,
            #[cfg(target_has_atomic = "32")]
            Self::I32(_) => AtomicKind::I32,
            #[cfg(target_has_atomic = "32")]
            Self::U32(_) => AtomicKind::U32,
            #[cfg(target_has_atomic = "64")]
            Self::I64(_) => AtomicKind::I64,
            #[cfg(target_has_atomic = "64")]
            Self::U64(_) => AtomicKind::U64,
        }
    }

    pub fn load(&self) -> u64 {
        self.load_with_order(AtomicOrder::SequentiallyConsistent)
            .expect("SeqCst is valid for atomic load")
    }

    /// Load using an explicit ordering, rejecting orders LLVM/Rust forbid for
    /// loads before touching the cell.
    pub fn load_with_order(&self, order: AtomicOrder) -> Result<u64, AtomicError> {
        if !valid_load_ordering(order) {
            return Err(AtomicError::InvalidOrdering);
        }
        let ordering = Ordering::from(order);
        Ok(match self {
            #[cfg(target_has_atomic = "8")]
            Self::Bool(v) => u64::from(v.load(ordering)),
            #[cfg(target_has_atomic = "8")]
            Self::I8(v) => v.load(ordering) as u8 as u64,
            #[cfg(target_has_atomic = "8")]
            Self::U8(v) => v.load(ordering) as u64,
            #[cfg(target_has_atomic = "16")]
            Self::I16(v) => v.load(ordering) as u16 as u64,
            #[cfg(target_has_atomic = "16")]
            Self::U16(v) => v.load(ordering) as u64,
            #[cfg(target_has_atomic = "32")]
            Self::I32(v) => v.load(ordering) as u32 as u64,
            #[cfg(target_has_atomic = "32")]
            Self::U32(v) => v.load(ordering) as u64,
            #[cfg(target_has_atomic = "64")]
            Self::I64(v) => v.load(ordering) as u64,
            #[cfg(target_has_atomic = "64")]
            Self::U64(v) => v.load(ordering),
        })
    }

    pub fn store(&self, value: u64) {
        self.store_with_order(value, AtomicOrder::SequentiallyConsistent)
            .expect("SeqCst is valid for atomic store");
    }

    /// Store using an explicit ordering, rejecting orders Rust forbids for
    /// stores before touching the cell.
    pub fn store_with_order(&self, value: u64, order: AtomicOrder) -> Result<(), AtomicError> {
        if !valid_store_ordering(order) {
            return Err(AtomicError::InvalidOrdering);
        }
        let ordering = Ordering::from(order);
        match self {
            #[cfg(target_has_atomic = "8")]
            Self::Bool(v) => v.store(value != 0, ordering),
            #[cfg(target_has_atomic = "8")]
            Self::I8(v) => v.store(value as i8, ordering),
            #[cfg(target_has_atomic = "8")]
            Self::U8(v) => v.store(value as u8, ordering),
            #[cfg(target_has_atomic = "16")]
            Self::I16(v) => v.store(value as i16, ordering),
            #[cfg(target_has_atomic = "16")]
            Self::U16(v) => v.store(value as u16, ordering),
            #[cfg(target_has_atomic = "32")]
            Self::I32(v) => v.store(value as i32, ordering),
            #[cfg(target_has_atomic = "32")]
            Self::U32(v) => v.store(value as u32, ordering),
            #[cfg(target_has_atomic = "64")]
            Self::I64(v) => v.store(value as i64, ordering),
            #[cfg(target_has_atomic = "64")]
            Self::U64(v) => v.store(value, ordering),
        }
        Ok(())
    }

    pub fn swap(&self, value: u64) -> u64 {
        self.swap_with_order(value, AtomicOrder::SequentiallyConsistent)
    }

    /// Exchange using any supported read-modify-write ordering.
    pub fn swap_with_order(&self, value: u64, order: AtomicOrder) -> u64 {
        let ordering = Ordering::from(order);
        match self {
            #[cfg(target_has_atomic = "8")]
            Self::Bool(v) => u64::from(v.swap(value != 0, ordering)),
            #[cfg(target_has_atomic = "8")]
            Self::I8(v) => v.swap(value as i8, ordering) as u8 as u64,
            #[cfg(target_has_atomic = "8")]
            Self::U8(v) => v.swap(value as u8, ordering) as u64,
            #[cfg(target_has_atomic = "16")]
            Self::I16(v) => v.swap(value as i16, ordering) as u16 as u64,
            #[cfg(target_has_atomic = "16")]
            Self::U16(v) => v.swap(value as u16, ordering) as u64,
            #[cfg(target_has_atomic = "32")]
            Self::I32(v) => v.swap(value as i32, ordering) as u32 as u64,
            #[cfg(target_has_atomic = "32")]
            Self::U32(v) => v.swap(value as u32, ordering) as u64,
            #[cfg(target_has_atomic = "64")]
            Self::I64(v) => v.swap(value as i64, ordering) as u64,
            #[cfg(target_has_atomic = "64")]
            Self::U64(v) => v.swap(value, ordering),
        }
    }

    pub fn fetch_add(&self, value: u64) -> Result<u64, AtomicError> {
        self.fetch_update(value, Update::Add, AtomicOrder::SequentiallyConsistent)
    }

    pub fn fetch_sub(&self, value: u64) -> Result<u64, AtomicError> {
        self.fetch_update(value, Update::Sub, AtomicOrder::SequentiallyConsistent)
    }

    /// Atomic addition with an explicit read-modify-write ordering.
    pub fn fetch_add_with_order(&self, value: u64, order: AtomicOrder) -> Result<u64, AtomicError> {
        self.fetch_update(value, Update::Add, order)
    }

    /// Atomic subtraction with an explicit read-modify-write ordering.
    pub fn fetch_sub_with_order(&self, value: u64, order: AtomicOrder) -> Result<u64, AtomicError> {
        self.fetch_update(value, Update::Sub, order)
    }

    fn fetch_update(&self, value: u64, op: Update, order: AtomicOrder) -> Result<u64, AtomicError> {
        let ordering = Ordering::from(order);
        macro_rules! update {
            ($a:expr, $v:expr) => {{
                match op {
                    Update::Add => $a.fetch_add($v, ordering),
                    Update::Sub => $a.fetch_sub($v, ordering),
                }
            }};
        }
        let previous = match self {
            #[cfg(target_has_atomic = "8")]
            Self::Bool(_) => return Err(AtomicError::TypeMismatch),
            #[cfg(target_has_atomic = "8")]
            Self::I8(v) => update!(v, value as i8) as u8 as u64,
            #[cfg(target_has_atomic = "8")]
            Self::U8(v) => update!(v, value as u8) as u64,
            #[cfg(target_has_atomic = "16")]
            Self::I16(v) => update!(v, value as i16) as u16 as u64,
            #[cfg(target_has_atomic = "16")]
            Self::U16(v) => update!(v, value as u16) as u64,
            #[cfg(target_has_atomic = "32")]
            Self::I32(v) => update!(v, value as i32) as u32 as u64,
            #[cfg(target_has_atomic = "32")]
            Self::U32(v) => update!(v, value as u32) as u64,
            #[cfg(target_has_atomic = "64")]
            Self::I64(v) => update!(v, value as i64) as u64,
            #[cfg(target_has_atomic = "64")]
            Self::U64(v) => update!(v, value),
        };
        Ok(previous)
    }

    /// Compare-exchange returns `(observed_value, exchanged)`.
    pub fn compare_exchange(
        &self,
        current: u64,
        new: u64,
        success: AtomicOrder,
        failure: AtomicOrder,
    ) -> Result<(u64, bool), AtomicError> {
        if !valid_compare_exchange_orderings(success, failure) {
            return Err(AtomicError::InvalidOrdering);
        }
        let success_order: Ordering = success.into();
        let failure_order: Ordering = failure.into();
        macro_rules! cas {
            ($a:expr, $c:expr, $n:expr, $mask:ty) => {{
                match $a.compare_exchange($c, $n, success_order, failure_order) {
                    Ok(old) => (old as $mask as u64, true),
                    Err(old) => (old as $mask as u64, false),
                }
            }};
        }
        Ok(match self {
            #[cfg(target_has_atomic = "8")]
            Self::Bool(v) => {
                match v.compare_exchange(current != 0, new != 0, success_order, failure_order) {
                    Ok(old) => (u64::from(old), true),
                    Err(old) => (u64::from(old), false),
                }
            }
            #[cfg(target_has_atomic = "8")]
            Self::I8(v) => cas!(v, current as i8, new as i8, u8),
            #[cfg(target_has_atomic = "8")]
            Self::U8(v) => cas!(v, current as u8, new as u8, u8),
            #[cfg(target_has_atomic = "16")]
            Self::I16(v) => cas!(v, current as i16, new as i16, u16),
            #[cfg(target_has_atomic = "16")]
            Self::U16(v) => cas!(v, current as u16, new as u16, u16),
            #[cfg(target_has_atomic = "32")]
            Self::I32(v) => cas!(v, current as i32, new as i32, u32),
            #[cfg(target_has_atomic = "32")]
            Self::U32(v) => cas!(v, current as u32, new as u32, u32),
            #[cfg(target_has_atomic = "64")]
            Self::I64(v) => cas!(v, current as i64, new as i64, u64),
            #[cfg(target_has_atomic = "64")]
            Self::U64(v) => cas!(v, current, new, u64),
        })
    }
}

enum Update {
    Add,
    Sub,
}

// Stable C ABI status values: 0 success, 1 invalid argument, 2 unsupported
// type/target, 3 invalid ordering, 4 operation does not apply to this type.
const OK: i32 = 0;
const INVALID_ARGUMENT: i32 = 1;
const UNSUPPORTED: i32 = 2;
const INVALID_ORDERING: i32 = 3;
const TYPE_MISMATCH: i32 = 4;

fn status(error: AtomicError) -> i32 {
    match error {
        AtomicError::UnsupportedWidthOrTarget => UNSUPPORTED,
        AtomicError::InvalidOrdering => INVALID_ORDERING,
        AtomicError::TypeMismatch => TYPE_MISMATCH,
    }
}

/// Create a scalar atomic. `kind` is an [`AtomicKind`] discriminant. `out`
/// receives an aligned opaque handle on success. Integer initial values use
/// their low bits; bool treats zero as false and nonzero as true.
///
/// # Safety
/// `out` must be writable and aligned for one pointer. The returned handle is
/// owned by the caller and must be freed exactly once with
/// [`zirk_rt_atomic_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_new(
    kind: u32,
    value: u64,
    out: *mut *mut AtomicCell,
) -> i32 {
    if out.is_null() {
        return INVALID_ARGUMENT;
    }
    let kind = match kind {
        0 => AtomicKind::Bool,
        1 => AtomicKind::I8,
        2 => AtomicKind::U8,
        3 => AtomicKind::I16,
        4 => AtomicKind::U16,
        5 => AtomicKind::I32,
        6 => AtomicKind::U32,
        7 => AtomicKind::I64,
        8 => AtomicKind::U64,
        _ => return UNSUPPORTED,
    };
    match AtomicCell::new(kind, value) {
        Ok(cell) => {
            unsafe {
                out.write(Box::into_raw(Box::new(cell)));
            }
            OK
        }
        Err(error) => status(error),
    }
}

/// Destroy a handle returned by [`zirk_rt_atomic_new`].
///
/// # Safety
/// `cell` is null or a live handle returned by `zirk_rt_atomic_new`, not
/// previously freed. No thread may access the handle concurrently with free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_free(cell: *mut AtomicCell) {
    if !cell.is_null() {
        unsafe {
            drop(Box::from_raw(cell));
        }
    }
}

/// Load a value with `SeqCst` ordering.
///
/// # Safety
/// `cell` is a live handle; `out` is writable and aligned for `u64`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_load(cell: *const AtomicCell, out: *mut u64) -> i32 {
    if cell.is_null() || out.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        out.write((*cell).load());
    }
    OK
}

/// Store a value with `SeqCst` ordering.
///
/// # Safety
/// `cell` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_store(cell: *const AtomicCell, value: u64) -> i32 {
    if cell.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        (*cell).store(value);
    }
    OK
}

/// Exchange a value with `SeqCst` ordering, returning the previous bits.
///
/// # Safety
/// `cell` is live; `out` is writable and aligned for `u64`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_exchange(
    cell: *const AtomicCell,
    value: u64,
    out: *mut u64,
) -> i32 {
    if cell.is_null() || out.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        out.write((*cell).swap(value));
    }
    OK
}

/// Compare-exchange with explicit orderings. `success` and `failure` use the
/// [`AtomicOrder`] discriminants. Failure ordering cannot be Release/AcqRel
/// and cannot be stronger than success; invalid pairs return status 3 before
/// executing a hardware operation.
///
/// # Safety
/// `cell` is live, `observed` and `exchanged` are writable and correctly
/// aligned. They must not alias each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_compare_exchange(
    cell: *const AtomicCell,
    current: u64,
    new: u64,
    success: u32,
    failure: u32,
    observed: *mut u64,
    exchanged: *mut bool,
) -> i32 {
    if cell.is_null()
        || observed.is_null()
        || exchanged.is_null()
        || observed.cast::<u8>() == exchanged.cast::<u8>()
    {
        return INVALID_ARGUMENT;
    }
    let (Ok(success), Ok(failure)) = (
        AtomicOrder::try_from(success),
        AtomicOrder::try_from(failure),
    ) else {
        return INVALID_ORDERING;
    };
    match unsafe { (*cell).compare_exchange(current, new, success, failure) } {
        Ok((old, did_exchange)) => {
            unsafe {
                observed.write(old);
                exchanged.write(did_exchange);
            }
            OK
        }
        Err(error) => status(error),
    }
}

/// Atomic integer addition with `SeqCst` ordering and modulo-width wraparound.
///
/// # Safety
/// `cell` is a live integer handle; `previous` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_fetch_add(
    cell: *const AtomicCell,
    value: u64,
    previous: *mut u64,
) -> i32 {
    if cell.is_null() || previous.is_null() {
        return INVALID_ARGUMENT;
    }
    match unsafe { (*cell).fetch_add(value) } {
        Ok(old) => {
            unsafe {
                previous.write(old);
            }
            OK
        }
        Err(error) => status(error),
    }
}

/// Atomic integer subtraction with `SeqCst` ordering and modulo-width wraparound.
///
/// # Safety
/// `cell` is a live integer handle; `previous` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_fetch_sub(
    cell: *const AtomicCell,
    value: u64,
    previous: *mut u64,
) -> i32 {
    if cell.is_null() || previous.is_null() {
        return INVALID_ARGUMENT;
    }
    match unsafe { (*cell).fetch_sub(value) } {
        Ok(old) => {
            unsafe {
                previous.write(old);
            }
            OK
        }
        Err(error) => status(error),
    }
}

// Managed Atomic objects use the collector's ordinary three-word header and
// a no-reference descriptor. `AtomicCell` contains no GC pointers, so the
// collector must never inspect its scalar payload as roots.
static ZIRK_RT_ATOMIC_MANAGED_DESCRIPTOR: [usize; 4] = [0, 0, 0, 0];
const MANAGED_CELL_ALIGN: usize = align_of_atomic_cell();
const MANAGED_CELL_OFFSET: usize = align_up(crate::collector::HEADER_BYTES, MANAGED_CELL_ALIGN);

const fn align_of_atomic_cell() -> usize {
    std::mem::align_of::<AtomicCell>()
}

const fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

fn valid_out<T>(pointer: *mut T) -> bool {
    !pointer.is_null() && (pointer as usize).is_multiple_of(std::mem::align_of::<T>())
}

/// Resolves a live managed Atomic object's inline payload.
///
/// # Safety
/// `object` must point to a live allocation returned by
/// [`zirk_rt_atomic_managed_new`] and remain rooted for the entire operation.
unsafe fn managed_cell(object: *const c_void) -> Result<*const AtomicCell, i32> {
    if object.is_null() || !(object as usize).is_multiple_of(crate::collector::allocation_align(1))
    {
        return Err(INVALID_ARGUMENT);
    }
    let descriptor = unsafe { object.cast::<*mut c_void>().read() };
    if descriptor != ZIRK_RT_ATOMIC_MANAGED_DESCRIPTOR.as_ptr().cast_mut().cast() {
        return Err(INVALID_ARGUMENT);
    }
    Ok(unsafe { object.cast::<u8>().add(MANAGED_CELL_OFFSET).cast() })
}

/// Allocates an AtomicCell inline in one collector-managed object.
///
/// Unlike [`zirk_rt_atomic_new`], the returned object is reclaimed by the GC
/// and must be kept in a GC root while in use. Its descriptor contains no
/// reference fields. The payload offset is rounded up from the standard GC
/// header to `align_of::<AtomicCell>()`; the collector's fixed base alignment
/// is checked here to cover that layout.
///
/// # Safety
/// `out_object` must be writable and aligned for one pointer. To keep the new
/// allocation alive, it must point to a slot registered as a GC root before
/// any later operation that can collect.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_new(
    kind: u32,
    value: u64,
    out_object: *mut *mut c_void,
) -> i32 {
    if !valid_out(out_object)
        || MANAGED_CELL_ALIGN > crate::collector::allocation_align(1)
        || MANAGED_CELL_OFFSET < crate::collector::HEADER_BYTES
    {
        return INVALID_ARGUMENT;
    }
    let kind = match kind {
        0 => AtomicKind::Bool,
        1 => AtomicKind::I8,
        2 => AtomicKind::U8,
        3 => AtomicKind::I16,
        4 => AtomicKind::U16,
        5 => AtomicKind::I32,
        6 => AtomicKind::U32,
        7 => AtomicKind::I64,
        8 => AtomicKind::U64,
        _ => return UNSUPPORTED,
    };
    let Ok(cell) = AtomicCell::new(kind, value) else {
        return UNSUPPORTED;
    };
    let Some(size) = MANAGED_CELL_OFFSET.checked_add(std::mem::size_of::<AtomicCell>()) else {
        return INVALID_ARGUMENT;
    };
    let object = unsafe { crate::memory::zirk_rt_alloc(size, MANAGED_CELL_ALIGN) };
    unsafe {
        object
            .cast::<*mut c_void>()
            .write(ZIRK_RT_ATOMIC_MANAGED_DESCRIPTOR.as_ptr().cast_mut().cast());
        object
            .cast::<u8>()
            .add(MANAGED_CELL_OFFSET)
            .cast::<AtomicCell>()
            .write(cell);
        out_object.write(object);
    }
    OK
}

/// Managed Atomic load with `SeqCst` ordering.
///
/// # Safety
/// `object` is a rooted object returned by the managed constructor; `out` is
/// writable, aligned and does not overlap the object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_load(object: *const c_void, out: *mut u64) -> i32 {
    if !valid_out(out) {
        return INVALID_ARGUMENT;
    }
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    unsafe { out.write(cell.load()) };
    OK
}

/// Managed Atomic load with an explicit load ordering.
///
/// # Safety
/// `object` is a rooted object returned by the managed constructor; `out` is
/// writable, aligned and does not overlap the object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_load_ordered(
    object: *const c_void,
    order: u32,
    out: *mut u64,
) -> i32 {
    if !valid_out(out) {
        return INVALID_ARGUMENT;
    }
    let order = match AtomicOrder::try_from(order) {
        Ok(order) if valid_load_ordering(order) => order,
        _ => return INVALID_ORDERING,
    };
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    match cell.load_with_order(order) {
        Ok(value) => {
            unsafe { out.write(value) };
            OK
        }
        Err(error) => status(error),
    }
}

/// Managed Atomic store with `SeqCst` ordering.
///
/// # Safety
/// `object` is a rooted object returned by the managed constructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_store(object: *const c_void, value: u64) -> i32 {
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    cell.store(value);
    OK
}

/// Managed Atomic store with an explicit store ordering.
///
/// # Safety
/// `object` is a rooted object returned by the managed constructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_store_ordered(
    object: *const c_void,
    value: u64,
    order: u32,
) -> i32 {
    let order = match AtomicOrder::try_from(order) {
        Ok(order) if valid_store_ordering(order) => order,
        _ => return INVALID_ORDERING,
    };
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    match cell.store_with_order(value, order) {
        Ok(()) => OK,
        Err(error) => status(error),
    }
}

/// Managed Atomic exchange with `SeqCst` ordering.
///
/// # Safety
/// `object` is rooted; `out` is writable, aligned and does not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_exchange(
    object: *const c_void,
    value: u64,
    out: *mut u64,
) -> i32 {
    if !valid_out(out) {
        return INVALID_ARGUMENT;
    }
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    unsafe { out.write(cell.swap(value)) };
    OK
}

/// Managed Atomic exchange with an explicit read-modify-write ordering.
///
/// # Safety
/// `object` is rooted; `out` is writable, aligned and does not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_exchange_ordered(
    object: *const c_void,
    value: u64,
    order: u32,
    out: *mut u64,
) -> i32 {
    if !valid_out(out) {
        return INVALID_ARGUMENT;
    }
    let Ok(order) = AtomicOrder::try_from(order) else {
        return INVALID_ORDERING;
    };
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    unsafe { out.write(cell.swap_with_order(value, order)) };
    OK
}

/// Managed Atomic compare-exchange with explicit orderings.
///
/// # Safety
/// `object` is rooted; both outputs are writable, aligned, disjoint, and do
/// not overlap the object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_compare_exchange(
    object: *const c_void,
    current: u64,
    new_value: u64,
    success: u32,
    failure: u32,
    observed: *mut u64,
    exchanged: *mut bool,
) -> i32 {
    if !valid_out(observed)
        || !valid_out(exchanged)
        || ranges_overlap(
            observed.cast(),
            std::mem::size_of::<u64>(),
            exchanged.cast(),
            std::mem::size_of::<bool>(),
        )
    {
        return INVALID_ARGUMENT;
    }
    let (Ok(success), Ok(failure)) = (
        AtomicOrder::try_from(success),
        AtomicOrder::try_from(failure),
    ) else {
        return INVALID_ORDERING;
    };
    if !valid_compare_exchange_orderings(success, failure) {
        return INVALID_ORDERING;
    }
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    match cell.compare_exchange(current, new_value, success, failure) {
        Ok((old, did_exchange)) => {
            unsafe {
                observed.write(old);
                exchanged.write(did_exchange);
            }
            OK
        }
        Err(error) => status(error),
    }
}

/// Managed Atomic integer addition with `SeqCst` ordering.
///
/// # Safety
/// `object` is rooted; `previous` is writable, aligned and does not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_fetch_add(
    object: *const c_void,
    value: u64,
    previous: *mut u64,
) -> i32 {
    unsafe {
        managed_fetch_update(
            object,
            value,
            AtomicOrder::SequentiallyConsistent,
            previous,
            true,
        )
    }
}

/// Managed Atomic integer addition with an explicit ordering.
///
/// # Safety
/// `object` is rooted; `previous` is writable, aligned and does not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_fetch_add_ordered(
    object: *const c_void,
    value: u64,
    order: u32,
    previous: *mut u64,
) -> i32 {
    let Ok(order) = AtomicOrder::try_from(order) else {
        return INVALID_ORDERING;
    };
    unsafe { managed_fetch_update(object, value, order, previous, true) }
}

/// Managed Atomic integer subtraction with `SeqCst` ordering.
///
/// # Safety
/// `object` is rooted; `previous` is writable, aligned and does not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_fetch_sub(
    object: *const c_void,
    value: u64,
    previous: *mut u64,
) -> i32 {
    unsafe {
        managed_fetch_update(
            object,
            value,
            AtomicOrder::SequentiallyConsistent,
            previous,
            false,
        )
    }
}

/// Managed Atomic integer subtraction with an explicit ordering.
///
/// # Safety
/// `object` is rooted; `previous` is writable, aligned and does not overlap it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_atomic_managed_fetch_sub_ordered(
    object: *const c_void,
    value: u64,
    order: u32,
    previous: *mut u64,
) -> i32 {
    let Ok(order) = AtomicOrder::try_from(order) else {
        return INVALID_ORDERING;
    };
    unsafe { managed_fetch_update(object, value, order, previous, false) }
}

unsafe fn managed_fetch_update(
    object: *const c_void,
    value: u64,
    order: AtomicOrder,
    previous: *mut u64,
    add: bool,
) -> i32 {
    if !valid_out(previous) {
        return INVALID_ARGUMENT;
    }
    let cell = match unsafe { managed_cell(object) } {
        Ok(cell) => unsafe { &*cell },
        Err(error) => return error,
    };
    let result = if add {
        cell.fetch_add_with_order(value, order)
    } else {
        cell.fetch_sub_with_order(value, order)
    };
    match result {
        Ok(old) => {
            unsafe { previous.write(old) };
            OK
        }
        Err(error) => status(error),
    }
}

fn ranges_overlap(a: *const u8, a_len: usize, b: *const u8, b_len: usize) -> bool {
    let a = a as usize;
    let b = b as usize;
    a.checked_add(a_len).is_none_or(|a_end| b < a_end)
        && b.checked_add(b_len).is_none_or(|b_end| a < b_end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn scalar_operations_are_sequentially_consistent_and_wrap_at_width() {
        let cell = AtomicCell::new(AtomicKind::U8, 255).unwrap();
        assert_eq!(cell.load(), 255);
        assert_eq!(cell.fetch_add(1), Ok(255));
        assert_eq!(cell.load(), 0);
        assert_eq!(cell.fetch_sub(1), Ok(0));
        assert_eq!(cell.swap(0x123), 255);
        assert_eq!(cell.load(), 0x23);
        assert_eq!(
            cell.compare_exchange(
                0x23,
                7,
                AtomicOrder::SequentiallyConsistent,
                AtomicOrder::Acquire
            ),
            Ok((0x23, true))
        );
        assert_eq!(
            cell.compare_exchange(
                0x23,
                9,
                AtomicOrder::SequentiallyConsistent,
                AtomicOrder::Relaxed
            ),
            Ok((7, false))
        );
    }

    #[test]
    fn operation_orderings_follow_load_store_and_rmw_legality() {
        use AtomicOrder::*;

        let cell = AtomicCell::new(AtomicKind::U32, 7).unwrap();
        for order in [Relaxed, Acquire, SequentiallyConsistent] {
            assert_eq!(cell.load_with_order(order), Ok(7));
        }
        for order in [Release, AcquireRelease] {
            assert_eq!(
                cell.load_with_order(order),
                Err(AtomicError::InvalidOrdering)
            );
        }

        for order in [Relaxed, Release, SequentiallyConsistent] {
            assert_eq!(cell.store_with_order(7, order), Ok(()));
        }
        for order in [Acquire, AcquireRelease] {
            assert_eq!(
                cell.store_with_order(9, order),
                Err(AtomicError::InvalidOrdering)
            );
            assert_eq!(cell.load(), 7, "invalid store must not mutate the cell");
        }

        for order in [
            Relaxed,
            Acquire,
            Release,
            AcquireRelease,
            SequentiallyConsistent,
        ] {
            assert_eq!(cell.swap_with_order(8, order), 7);
            assert_eq!(cell.fetch_add_with_order(1, order), Ok(8));
            assert_eq!(cell.fetch_sub_with_order(1, order), Ok(9));
            assert_eq!(cell.swap_with_order(7, order), 8);
        }
    }

    #[test]
    fn bool_is_boolean_and_has_no_integer_updates() {
        let cell = AtomicCell::new(AtomicKind::Bool, 22).unwrap();
        assert_eq!(cell.load(), 1);
        assert_eq!(cell.fetch_add(1), Err(AtomicError::TypeMismatch));
        cell.store(0);
        assert_eq!(cell.swap(9), 0);
        assert_eq!(cell.load(), 1);
    }

    #[test]
    fn all_supported_widths_preserve_low_bits_and_are_properly_aligned() {
        let kinds = [
            AtomicKind::Bool,
            AtomicKind::I8,
            AtomicKind::U8,
            AtomicKind::I16,
            AtomicKind::U16,
            AtomicKind::I32,
            AtomicKind::U32,
            AtomicKind::I64,
            AtomicKind::U64,
        ];
        for kind in kinds {
            if supports(kind) {
                let cell = AtomicCell::new(kind, u64::MAX).unwrap();
                let address = (&cell as *const AtomicCell) as usize;
                assert_eq!(address % align_of::<AtomicCell>(), 0);
                assert_eq!(cell.kind(), kind);
                assert_eq!(
                    cell.load(),
                    match kind {
                        AtomicKind::Bool => 1,
                        AtomicKind::I8 | AtomicKind::U8 => 0xff,
                        AtomicKind::I16 | AtomicKind::U16 => 0xffff,
                        AtomicKind::I32 | AtomicKind::U32 => 0xffff_ffff,
                        AtomicKind::I64 | AtomicKind::U64 => u64::MAX,
                    }
                );
                assert!(size_of::<AtomicCell>() >= align_of::<AtomicCell>());
            } else {
                assert!(matches!(
                    AtomicCell::new(kind, 0),
                    Err(AtomicError::UnsupportedWidthOrTarget)
                ));
            }
        }
    }

    #[test]
    fn compare_exchange_ordering_matrix_rejects_every_invalid_pair() {
        use AtomicOrder::*;
        let orders = [
            Relaxed,
            Acquire,
            Release,
            AcquireRelease,
            SequentiallyConsistent,
        ];
        let cell = AtomicCell::new(AtomicKind::U32, 0).unwrap();
        for success in orders {
            for failure in orders {
                let expected = valid_compare_exchange_orderings(success, failure);
                assert_eq!(
                    cell.compare_exchange(0, 1, success, failure).is_ok(),
                    expected,
                    "{success:?}/{failure:?}"
                );
            }
        }
        assert!(!valid_compare_exchange_orderings(Relaxed, Acquire));
        assert!(!valid_compare_exchange_orderings(Release, Acquire));
        assert!(!valid_compare_exchange_orderings(
            Acquire,
            SequentiallyConsistent
        ));
    }

    #[test]
    fn unknown_abi_widths_and_bad_order_values_are_rejected() {
        for raw in [5, 99, u32::MAX] {
            assert_eq!(
                AtomicOrder::try_from(raw),
                Err(AtomicError::InvalidOrdering)
            );
        }
        let mut out = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_atomic_new(0, 0, std::ptr::null_mut()) },
            INVALID_ARGUMENT
        );
        assert_eq!(unsafe { zirk_rt_atomic_new(9, 0, &mut out) }, UNSUPPORTED);
        assert_eq!(unsafe { zirk_rt_atomic_new(128, 0, &mut out) }, UNSUPPORTED);
        assert!(out.is_null());
        assert_eq!(
            unsafe { zirk_rt_atomic_new(AtomicKind::U64 as u32, 0, &mut out) },
            OK
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_compare_exchange(out, 0, 1, 0, 1, &mut 0, &mut false) },
            INVALID_ORDERING
        );
        unsafe {
            zirk_rt_atomic_free(out);
        }
    }

    #[test]
    fn c_abi_constructs_every_kind_at_its_natural_alignment() {
        for kind in [
            AtomicKind::Bool,
            AtomicKind::I8,
            AtomicKind::U8,
            AtomicKind::I16,
            AtomicKind::U16,
            AtomicKind::I32,
            AtomicKind::U32,
            AtomicKind::I64,
            AtomicKind::U64,
        ] {
            let mut handle = std::ptr::null_mut();
            let status = unsafe { zirk_rt_atomic_new(kind as u32, 5, &mut handle) };
            if !supports(kind) {
                assert_eq!(status, UNSUPPORTED, "{kind:?}");
                assert!(handle.is_null());
                continue;
            }
            assert_eq!(status, OK, "{kind:?}");
            assert!(!handle.is_null());
            assert_eq!((handle as usize) % align_of::<AtomicCell>(), 0, "{kind:?}");
            assert_eq!(unsafe { (*handle).kind() }, kind);
            unsafe { zirk_rt_atomic_free(handle) };
        }
    }

    #[test]
    fn c_abi_null_inputs_outputs_and_compare_exchange_alias_are_rejected() {
        let mut handle = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_atomic_new(AtomicKind::U64 as u32, 5, &mut handle) },
            OK
        );
        let cell = handle.cast_const();
        let mut value = 0u64;
        let mut exchanged = false;

        assert_eq!(
            unsafe { zirk_rt_atomic_load(std::ptr::null(), &mut value) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_load(cell, std::ptr::null_mut()) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_store(std::ptr::null(), 1) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_exchange(std::ptr::null(), 1, &mut value) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_exchange(cell, 1, std::ptr::null_mut()) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_fetch_add(std::ptr::null(), 1, &mut value) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_fetch_add(cell, 1, std::ptr::null_mut()) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_fetch_sub(std::ptr::null(), 1, &mut value) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_fetch_sub(cell, 1, std::ptr::null_mut()) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_compare_exchange(
                    std::ptr::null(),
                    5,
                    8,
                    4,
                    0,
                    &mut value,
                    &mut exchanged,
                )
            },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_compare_exchange(
                    cell,
                    5,
                    8,
                    4,
                    0,
                    std::ptr::null_mut(),
                    &mut exchanged,
                )
            },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_compare_exchange(cell, 5, 8, 4, 0, &mut value, std::ptr::null_mut())
            },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_compare_exchange(
                    cell,
                    5,
                    8,
                    4,
                    0,
                    &mut value,
                    (&mut value as *mut u64).cast(),
                )
            },
            INVALID_ARGUMENT
        );

        unsafe { zirk_rt_atomic_free(handle) };
        unsafe { zirk_rt_atomic_free(std::ptr::null_mut()) };
    }

    #[test]
    fn managed_atomic_is_registered_aligned_and_survives_collection_while_rooted() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();

        let mut rooted_object = std::ptr::null_mut();
        let mut roots = [(&mut rooted_object as *mut *mut c_void).cast::<c_void>()];
        unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_new(AtomicKind::U64 as u32, 41, &mut rooted_object) },
            OK
        );
        assert!(!rooted_object.is_null());
        assert!(crate::collector::is_live_for_tests(rooted_object));
        assert_eq!(
            (rooted_object as usize + MANAGED_CELL_OFFSET) % align_of::<AtomicCell>(),
            0
        );
        assert_eq!(
            unsafe { rooted_object.cast::<*mut c_void>().read() },
            ZIRK_RT_ATOMIC_MANAGED_DESCRIPTOR.as_ptr().cast_mut().cast()
        );

        crate::collector::collect_for_tests();
        assert!(crate::collector::is_live_for_tests(rooted_object));
        let mut observed = 0;
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_load(rooted_object, &mut observed) },
            OK
        );
        assert_eq!(observed, 41);
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_fetch_add(rooted_object, 1, &mut observed) },
            OK
        );
        assert_eq!(observed, 41);
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_load(rooted_object, &mut observed) },
            OK
        );
        assert_eq!(observed, 42);
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_store_ordered(rooted_object, 50, AtomicOrder::Release as u32)
            },
            OK
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_exchange_ordered(
                    rooted_object,
                    51,
                    AtomicOrder::AcquireRelease as u32,
                    &mut observed,
                )
            },
            OK
        );
        assert_eq!(observed, 50);
        let mut exchanged = false;
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_compare_exchange(
                    rooted_object,
                    51,
                    60,
                    AtomicOrder::SequentiallyConsistent as u32,
                    AtomicOrder::Relaxed as u32,
                    &mut observed,
                    &mut exchanged,
                )
            },
            OK
        );
        assert_eq!((observed, exchanged), (51, true));
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_fetch_sub_ordered(
                    rooted_object,
                    2,
                    AtomicOrder::Acquire as u32,
                    &mut observed,
                )
            },
            OK
        );
        assert_eq!(observed, 60);
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_load_ordered(
                    rooted_object,
                    AtomicOrder::Relaxed as u32,
                    &mut observed,
                )
            },
            OK
        );
        assert_eq!(observed, 58);

        crate::collector::zirk_rt_pop_frame();
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn managed_atomic_rejects_bad_pointers_and_orderings_before_payload_access() {
        let _guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();

        let mut object = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_new(AtomicKind::U32 as u32, 7, &mut object) },
            OK
        );
        let mut out = 0xfeed;
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_load(std::ptr::null(), &mut out) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { zirk_rt_atomic_managed_load(object, std::ptr::null_mut()) },
            INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_load_ordered(
                    std::ptr::null(),
                    AtomicOrder::Release as u32,
                    &mut out,
                )
            },
            INVALID_ORDERING
        );
        assert_eq!(out, 0xfeed, "invalid order leaves output untouched");
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_store_ordered(
                    std::ptr::null(),
                    99,
                    AtomicOrder::Acquire as u32,
                )
            },
            INVALID_ORDERING
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_compare_exchange(
                    std::ptr::null(),
                    7,
                    99,
                    AtomicOrder::Relaxed as u32,
                    AtomicOrder::Acquire as u32,
                    &mut out,
                    &mut false,
                )
            },
            INVALID_ORDERING
        );
        assert_eq!(out, 0xfeed);
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_load_ordered(object, AtomicOrder::Release as u32, &mut out)
            },
            INVALID_ORDERING
        );
        assert_eq!(out, 0xfeed, "invalid load leaves output untouched");
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_store_ordered(object, 99, AtomicOrder::Acquire as u32)
            },
            INVALID_ORDERING
        );
        assert_eq!(
            unsafe {
                zirk_rt_atomic_managed_compare_exchange(
                    object,
                    7,
                    99,
                    AtomicOrder::Relaxed as u32,
                    AtomicOrder::Acquire as u32,
                    &mut out,
                    &mut false,
                )
            },
            INVALID_ORDERING
        );
        assert_eq!(unsafe { zirk_rt_atomic_managed_load(object, &mut out) }, OK);
        assert_eq!(out, 7, "rejected orderings do not mutate the object");
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn c_abi_compare_exchange_reports_success_failure_and_rejects_orderings() {
        let mut handle = std::ptr::null_mut();
        assert_eq!(
            unsafe { zirk_rt_atomic_new(AtomicKind::U32 as u32, 7, &mut handle) },
            OK
        );
        let mut observed = u64::MAX;
        let mut exchanged = false;
        for success in 0..=4 {
            for failure in 0..=4 {
                let valid = valid_compare_exchange_orderings(
                    AtomicOrder::try_from(success).unwrap(),
                    AtomicOrder::try_from(failure).unwrap(),
                );
                if valid {
                    let status = unsafe {
                        zirk_rt_atomic_compare_exchange(
                            handle,
                            7,
                            9,
                            success,
                            failure,
                            &mut observed,
                            &mut exchanged,
                        )
                    };
                    assert_eq!(status, OK, "success ordering pair {success}/{failure}");
                    assert_eq!((observed, exchanged), (7, true), "pair {success}/{failure}");
                    assert_eq!(unsafe { zirk_rt_atomic_store(handle, 7) }, OK);

                    let status = unsafe {
                        zirk_rt_atomic_compare_exchange(
                            handle,
                            6,
                            9,
                            success,
                            failure,
                            &mut observed,
                            &mut exchanged,
                        )
                    };
                    assert_eq!(status, OK, "failure ordering pair {success}/{failure}");
                    assert_eq!(
                        (observed, exchanged),
                        (7, false),
                        "pair {success}/{failure}"
                    );
                } else {
                    observed = 0xfeed;
                    exchanged = true;
                    assert_eq!(
                        unsafe {
                            zirk_rt_atomic_compare_exchange(
                                handle,
                                7,
                                9,
                                success,
                                failure,
                                &mut observed,
                                &mut exchanged,
                            )
                        },
                        INVALID_ORDERING,
                        "{success}/{failure}"
                    );
                    assert_eq!(
                        (observed, exchanged),
                        (0xfeed, true),
                        "outputs changed for {success}/{failure}"
                    );
                }
            }
        }

        for (success, failure) in [(5, 0), (0, 5), (u32::MAX, 4), (4, u32::MAX)] {
            observed = 0xfeed;
            exchanged = true;
            assert_eq!(
                unsafe {
                    zirk_rt_atomic_compare_exchange(
                        handle,
                        7,
                        9,
                        success,
                        failure,
                        &mut observed,
                        &mut exchanged,
                    )
                },
                INVALID_ORDERING,
                "unknown ordering {success}/{failure}"
            );
            assert_eq!((observed, exchanged), (0xfeed, true));
        }
        unsafe { zirk_rt_atomic_free(handle) };
    }
}
