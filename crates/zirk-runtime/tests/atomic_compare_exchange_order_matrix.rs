use zirk_runtime::{AtomicCell, AtomicError, AtomicKind, AtomicOrder, atomic_supports};

const ORDERS: [AtomicOrder; 5] = [
    AtomicOrder::Relaxed,
    AtomicOrder::Acquire,
    AtomicOrder::Release,
    AtomicOrder::AcquireRelease,
    AtomicOrder::SequentiallyConsistent,
];

fn make_cell() -> Option<AtomicCell> {
    if atomic_supports(AtomicKind::U32) {
        Some(AtomicCell::new(AtomicKind::U32, 17).expect("supported U32 atomic"))
    } else {
        None
    }
}

fn expected_valid(success: AtomicOrder, failure: AtomicOrder) -> bool {
    use AtomicOrder::*;
    match (success, failure) {
        (_, Release | AcquireRelease) => false,
        (Relaxed | Release, Relaxed) => true,
        (Acquire | AcquireRelease, Relaxed | Acquire) => true,
        (SequentiallyConsistent, Relaxed | Acquire | SequentiallyConsistent) => true,
        _ => false,
    }
}

#[test]
fn compare_exchange_order_matrix_accepts_legal_pairs_and_rejects_invalid_pairs_without_mutation() {
    if make_cell().is_none() {
        eprintln!("skipping: this target does not support 32-bit atomics");
        return;
    }

    for success in ORDERS {
        for failure in ORDERS {
            let legal = expected_valid(success, failure);

            // A matching comparison exercises the success-ordering path.
            let cell = make_cell().unwrap();
            let result = cell.compare_exchange(17, 29, success, failure);
            if legal {
                assert_eq!(result, Ok((17, true)), "{success:?}/{failure:?}");
                assert_eq!(cell.load(), 29, "{success:?}/{failure:?}");
            } else {
                assert_eq!(
                    result,
                    Err(AtomicError::InvalidOrdering),
                    "{success:?}/{failure:?}"
                );
                assert_eq!(
                    cell.load(),
                    17,
                    "invalid pair mutated the cell: {success:?}/{failure:?}"
                );
            }

            // A mismatch exercises failure ordering for every legal pair.
            let cell = make_cell().unwrap();
            let result = cell.compare_exchange(99, 29, success, failure);
            if legal {
                assert_eq!(result, Ok((17, false)), "{success:?}/{failure:?}");
            } else {
                assert_eq!(
                    result,
                    Err(AtomicError::InvalidOrdering),
                    "{success:?}/{failure:?}"
                );
            }
            assert_eq!(
                cell.load(),
                17,
                "mismatch/invalid pair mutated the cell: {success:?}/{failure:?}"
            );
        }
    }
}
