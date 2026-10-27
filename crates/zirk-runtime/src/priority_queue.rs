//! Runtime storage for stable, typed `PriorityQueue<T>` values.

use std::cmp::Ordering;
use std::ffi::c_void;

const WORD: usize = std::mem::size_of::<usize>();
const BODY_OFFSET: usize = crate::collector::HEADER_BYTES;

/// Stable ABI codes selected by IR lowering for naturally ordered elements.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityOrder {
    Boolean = 1,
    Char = 2,
    I8 = 3,
    I16 = 4,
    I32 = 5,
    I64 = 6,
    I128 = 7,
    U8 = 8,
    U16 = 9,
    U32 = 10,
    U64 = 11,
    U128 = 12,
    F16 = 13,
    F32 = 14,
    F64 = 15,
    F128 = 16,
    Decimal = 17,
    String = 18,
}

struct PriorityQueueBody {
    elements: Vec<PriorityEntry>,
    sorted_snapshot: Option<Vec<Vec<u8>>>,
    next_sequence: u64,
    reference_offsets: Box<[usize]>,
    element_size: usize,
    order: PriorityOrder,
    structural_version: u64,
}

#[derive(Clone)]
struct PriorityEntry {
    bytes: Vec<u8>,
    sequence: u64,
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn fail_next_reservation_for_tests() {
    FAIL_NEXT_RESERVATION.with(|fail| fail.set(true));
}

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static zirk_rt_priority_queue_descriptor: [usize; 4] = [0, 0, 0, 0];

pub(crate) fn priority_queue_descriptor() -> *mut c_void {
    (&raw const zirk_rt_priority_queue_descriptor) as *mut c_void
}

unsafe fn body(queue: *mut c_void) -> *mut PriorityQueueBody {
    unsafe { (queue as *mut u8).add(BODY_OFFSET).cast() }
}

unsafe fn copy_value(body: &PriorityQueueBody, value: *const c_void) -> Result<Vec<u8>, ()> {
    if body.element_size != 0 && value.is_null() {
        return Err(());
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(body.element_size).map_err(|_| ())?;
    if body.element_size != 0 {
        bytes.extend_from_slice(unsafe {
            std::slice::from_raw_parts(value.cast::<u8>(), body.element_size)
        });
    }
    Ok(bytes)
}

unsafe fn compare(order: PriorityOrder, left: &[u8], right: &[u8]) -> Ordering {
    macro_rules! signed {
        ($ty:ty) => {{
            let left = unsafe { left.as_ptr().cast::<$ty>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<$ty>().read_unaligned() };
            left.cmp(&right)
        }};
    }
    macro_rules! unsigned {
        ($ty:ty) => {{
            let left = unsafe { left.as_ptr().cast::<$ty>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<$ty>().read_unaligned() };
            left.cmp(&right)
        }};
    }
    match order {
        PriorityOrder::Boolean => unsigned!(u8),
        PriorityOrder::Char => unsigned!(u32),
        PriorityOrder::I8 => signed!(i8),
        PriorityOrder::I16 => signed!(i16),
        PriorityOrder::I32 => signed!(i32),
        PriorityOrder::I64 => signed!(i64),
        PriorityOrder::I128 => signed!(i128),
        PriorityOrder::U8 => unsigned!(u8),
        PriorityOrder::U16 => unsigned!(u16),
        PriorityOrder::U32 => unsigned!(u32),
        PriorityOrder::U64 => unsigned!(u64),
        PriorityOrder::U128 => unsigned!(u128),
        PriorityOrder::F16 => {
            let left = unsafe { left.as_ptr().cast::<u16>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<u16>().read_unaligned() };
            float_key(left as u128, 16).cmp(&float_key(right as u128, 16))
        }
        PriorityOrder::F32 => {
            let left = unsafe { left.as_ptr().cast::<f32>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<f32>().read_unaligned() };
            left.total_cmp(&right)
        }
        PriorityOrder::F64 => {
            let left = unsafe { left.as_ptr().cast::<f64>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<f64>().read_unaligned() };
            left.total_cmp(&right)
        }
        PriorityOrder::F128 => {
            let left = unsafe { left.as_ptr().cast::<u128>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<u128>().read_unaligned() };
            float_key(left, 128).cmp(&float_key(right, 128))
        }
        PriorityOrder::Decimal => {
            let left = unsafe {
                left.as_ptr()
                    .cast::<zirk_runtime_core::decimal::Decimal>()
                    .read_unaligned()
            };
            let right = unsafe {
                right
                    .as_ptr()
                    .cast::<zirk_runtime_core::decimal::Decimal>()
                    .read_unaligned()
            };
            zirk_runtime_core::decimal::compare(left, right)
                .unwrap_or(0)
                .cmp(&0)
        }
        PriorityOrder::String => {
            let left = unsafe { left.as_ptr().cast::<*mut c_void>().read_unaligned() };
            let right = unsafe { right.as_ptr().cast::<*mut c_void>().read_unaligned() };
            let left = unsafe { crate::string::payload_ptr(left).as_ref() }
                .map_or("", |value| unsafe { value.as_str() });
            let right = unsafe { crate::string::payload_ptr(right).as_ref() }
                .map_or("", |value| unsafe { value.as_str() });
            left.cmp(right)
        }
    }
}

fn compare_entries(order: PriorityOrder, left: &PriorityEntry, right: &PriorityEntry) -> Ordering {
    unsafe { compare(order, &left.bytes, &right.bytes) }
        .then_with(|| left.sequence.cmp(&right.sequence))
}

fn float_key(bits: u128, width: u32) -> u128 {
    let sign = 1_u128 << (width - 1);
    let mask = (1_u128 << width) - 1;
    let bits = bits & mask;
    if bits & sign == 0 {
        bits | sign
    } else {
        (!bits) & mask
    }
}

/// Creates an empty collector-managed priority queue.
///
/// # Safety
/// `reference_offsets` must contain valid offsets within one element.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_priority_queue_new_with_layout(
    element_size: usize,
    reference_offsets: *const usize,
    reference_offset_count: usize,
    order: i32,
) -> *mut c_void {
    let Some(order) = priority_order(order) else {
        return std::ptr::null_mut();
    };
    let mut offsets = Vec::new();
    if offsets.try_reserve_exact(reference_offset_count).is_err() {
        return std::ptr::null_mut();
    }
    if reference_offset_count != 0 {
        if reference_offsets.is_null() {
            return std::ptr::null_mut();
        }
        let source =
            unsafe { std::slice::from_raw_parts(reference_offsets, reference_offset_count) };
        if source.iter().any(|offset| {
            offset
                .checked_add(WORD)
                .is_none_or(|end| end > element_size)
        }) {
            return std::ptr::null_mut();
        }
        offsets.extend_from_slice(source);
    }
    let object = unsafe {
        crate::memory::zirk_rt_alloc(
            BODY_OFFSET + std::mem::size_of::<PriorityQueueBody>(),
            std::mem::align_of::<PriorityQueueBody>(),
        )
    };
    unsafe {
        *(object as *mut *mut c_void) = priority_queue_descriptor();
        body(object).write(PriorityQueueBody {
            elements: Vec::new(),
            sorted_snapshot: None,
            next_sequence: 0,
            reference_offsets: offsets.into_boxed_slice(),
            element_size,
            order,
            structural_version: 0,
        });
    }
    object
}

fn priority_order(order: i32) -> Option<PriorityOrder> {
    Some(match order {
        1 => PriorityOrder::Boolean,
        2 => PriorityOrder::Char,
        3 => PriorityOrder::I8,
        4 => PriorityOrder::I16,
        5 => PriorityOrder::I32,
        6 => PriorityOrder::I64,
        7 => PriorityOrder::I128,
        8 => PriorityOrder::U8,
        9 => PriorityOrder::U16,
        10 => PriorityOrder::U32,
        11 => PriorityOrder::U64,
        12 => PriorityOrder::U128,
        13 => PriorityOrder::F16,
        14 => PriorityOrder::F32,
        15 => PriorityOrder::F64,
        16 => PriorityOrder::F128,
        17 => PriorityOrder::Decimal,
        18 => PriorityOrder::String,
        _ => return None,
    })
}

/// Inserts after existing equal-priority entries. Returns 0 on success and 2
/// when any required allocation fails; failure leaves the queue unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_priority_queue_enqueue(
    queue: *mut c_void,
    value: *const c_void,
) -> i32 {
    if queue.is_null() {
        return 3;
    }
    let body = unsafe { &mut *body(queue) };
    #[cfg(test)]
    if FAIL_NEXT_RESERVATION.with(std::cell::Cell::get) {
        FAIL_NEXT_RESERVATION.with(|fail| fail.set(false));
        return 2;
    }
    if body.elements.try_reserve(1).is_err() {
        return 2;
    }
    let Ok(element) = (unsafe { copy_value(body, value) }) else {
        return 2;
    };
    let Some(next_sequence) = body.next_sequence.checked_add(1) else {
        return 2;
    };
    let sequence = body.next_sequence;
    body.elements.push(PriorityEntry {
        bytes: element,
        sequence,
    });
    body.next_sequence = next_sequence;
    body.sorted_snapshot = None;
    let mut child = body.elements.len() - 1;
    while child > 0 {
        let parent = (child - 1) / 2;
        if compare_entries(body.order, &body.elements[child], &body.elements[parent])
            != Ordering::Less
        {
            break;
        }
        body.elements.swap(child, parent);
        child = parent;
    }
    body.structural_version = body.structural_version.wrapping_add(1);
    0
}

unsafe fn read(queue: *mut c_void, output: *mut c_void, remove: bool) -> i32 {
    if queue.is_null() {
        return 0;
    }
    let body = unsafe { &mut *body(queue) };
    let Some(element) = body.elements.first() else {
        return 0;
    };
    if body.element_size != 0 && output.is_null() {
        return 2;
    }
    if body.element_size != 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(element.bytes.as_ptr(), output.cast(), body.element_size)
        };
    }
    if remove {
        let last = body.elements.pop().expect("non-empty priority queue");
        if !body.elements.is_empty() {
            body.elements[0] = last;
            let mut parent = 0;
            loop {
                let left = parent * 2 + 1;
                if left >= body.elements.len() {
                    break;
                }
                let right = left + 1;
                let child = if right < body.elements.len()
                    && compare_entries(body.order, &body.elements[right], &body.elements[left])
                        == Ordering::Less
                {
                    right
                } else {
                    left
                };
                if compare_entries(body.order, &body.elements[child], &body.elements[parent])
                    != Ordering::Less
                {
                    break;
                }
                body.elements.swap(parent, child);
                parent = child;
            }
        }
        body.sorted_snapshot = None;
        body.structural_version = body.structural_version.wrapping_add(1);
    }
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_priority_queue_peek(
    queue: *mut c_void,
    output: *mut c_void,
) -> i32 {
    unsafe { read(queue, output, false) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_priority_queue_dequeue(
    queue: *mut c_void,
    output: *mut c_void,
) -> i32 {
    unsafe { read(queue, output, true) }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_priority_queue_length(queue: *mut c_void) -> u64 {
    if queue.is_null() {
        0
    } else {
        unsafe { (&*body(queue)).elements.len() as u64 }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_priority_queue_is_empty(queue: *mut c_void) -> i32 {
    (zirk_rt_priority_queue_length(queue) == 0) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_priority_queue_version(queue: *mut c_void) -> u64 {
    if queue.is_null() {
        0
    } else {
        unsafe { (&*body(queue)).structural_version }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn zirk_rt_priority_queue_clear(queue: *mut c_void) {
    if queue.is_null() {
        return;
    }
    let body = unsafe { &mut *body(queue) };
    if !body.elements.is_empty() {
        body.elements.clear();
        body.sorted_snapshot = None;
        body.structural_version = body.structural_version.wrapping_add(1);
    }
}

/// Returns the address of an item in dequeue order for generic iteration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zirk_rt_priority_queue_element(
    queue: *mut c_void,
    index: usize,
) -> *mut c_void {
    if queue.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let body = &mut *body(queue);
        if body.sorted_snapshot.is_none() {
            let mut ordered: Vec<PriorityEntry> = Vec::new();
            if ordered.try_reserve_exact(body.elements.len()).is_err() {
                return std::ptr::null_mut();
            }
            ordered.extend(body.elements.iter().cloned());
            ordered.sort_by(|left, right| compare_entries(body.order, left, right));
            let mut snapshot = Vec::new();
            if snapshot.try_reserve_exact(ordered.len()).is_err() {
                return std::ptr::null_mut();
            }
            for entry in ordered {
                let mut bytes = Vec::new();
                if bytes.try_reserve_exact(entry.bytes.len()).is_err() {
                    return std::ptr::null_mut();
                }
                bytes.extend_from_slice(&entry.bytes);
                snapshot.push(bytes);
            }
            body.sorted_snapshot = Some(snapshot);
        }
        body.sorted_snapshot
            .as_ref()
            .and_then(|items| items.get(index))
            .map_or(std::ptr::null_mut(), |item| item.as_ptr() as *mut c_void)
    }
}

pub(crate) unsafe fn mark_entries(
    queue: *mut c_void,
    allocations: &crate::collector::AllocationSet,
    mark: &mut dyn FnMut(*mut c_void),
) {
    let body = unsafe { &*body(queue) };
    for element in &body.elements {
        for &offset in &body.reference_offsets {
            let candidate = unsafe {
                element
                    .bytes
                    .as_ptr()
                    .add(offset)
                    .cast::<*mut c_void>()
                    .read_unaligned()
            };
            if allocations.contains(&(candidate as usize)) {
                mark(candidate);
            }
        }
    }
}

pub(crate) unsafe fn drop_body(queue: *mut c_void) {
    unsafe { std::ptr::drop_in_place(body(queue)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn new_i32_queue() -> *mut c_void {
        unsafe {
            zirk_rt_priority_queue_new_with_layout(
                std::mem::size_of::<i32>(),
                std::ptr::null(),
                0,
                PriorityOrder::I32 as i32,
            )
        }
    }

    #[test]
    fn integer_priority_queue_is_minimum_first_and_stable() {
        unsafe {
            let queue = new_i32_queue();
            for value in [8_i32, 1, 4, 1] {
                assert_eq!(
                    zirk_rt_priority_queue_enqueue(queue, (&value as *const i32).cast()),
                    0
                );
            }
            let mut output = 0_i32;
            for expected in [1_i32, 1, 4, 8] {
                assert_eq!(
                    zirk_rt_priority_queue_dequeue(queue, (&mut output as *mut i32).cast()),
                    1
                );
                assert_eq!(output, expected);
            }
            assert_eq!(zirk_rt_priority_queue_length(queue), 0);
        }
    }

    #[test]
    fn priority_queue_equal_text_priorities_keep_handle_insertion_order() {
        unsafe {
            let offsets = [0_usize];
            let queue = zirk_rt_priority_queue_new_with_layout(
                WORD,
                offsets.as_ptr(),
                offsets.len(),
                PriorityOrder::String as i32,
            );
            let first = crate::string::alloc_owned("same-priority");
            let second = crate::string::alloc_owned("same-priority");
            assert_ne!(first, second);
            assert_eq!(
                zirk_rt_priority_queue_enqueue(queue, (&first as *const *mut c_void).cast()),
                0
            );
            assert_eq!(
                zirk_rt_priority_queue_enqueue(queue, (&second as *const *mut c_void).cast()),
                0
            );
            let mut output = std::ptr::null_mut();
            assert_eq!(
                zirk_rt_priority_queue_dequeue(queue, (&mut output as *mut *mut c_void).cast()),
                1
            );
            assert_eq!(output, first);
            assert_eq!(
                zirk_rt_priority_queue_dequeue(queue, (&mut output as *mut *mut c_void).cast()),
                1
            );
            assert_eq!(output, second);
        }
    }

    #[test]
    fn priority_queue_failed_enqueue_reservation_is_atomic() {
        unsafe {
            let queue = new_i32_queue();
            let value = 3_i32;
            assert_eq!(
                zirk_rt_priority_queue_enqueue(queue, (&value as *const i32).cast()),
                0
            );
            let version = zirk_rt_priority_queue_version(queue);
            fail_next_reservation_for_tests();
            let next = 1_i32;
            assert_eq!(
                zirk_rt_priority_queue_enqueue(queue, (&next as *const i32).cast()),
                2
            );
            assert_eq!(zirk_rt_priority_queue_length(queue), 1);
            assert_eq!(zirk_rt_priority_queue_version(queue), version);
            let mut output = 0_i32;
            assert_eq!(
                zirk_rt_priority_queue_peek(queue, (&mut output as *mut i32).cast()),
                1
            );
            assert_eq!(output, 3);
        }
    }
}
