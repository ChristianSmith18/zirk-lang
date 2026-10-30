//! Manually-run collection microbenchmarks.
//!
//! Run with `cargo test -p zirk-runtime --lib collection_operation_costs -- --ignored --nocapture`.

use std::time::Instant;

#[test]
#[ignore = "manual focused collection benchmark"]
fn collection_operation_costs() {
    const WORD: usize = std::mem::size_of::<usize>();
    for count in [1_000_i64, 10_000, 100_000] {
        let started = Instant::now();
        let deque = unsafe {
            crate::deque::zirk_rt_deque_new_with_layout(
                std::mem::size_of::<i32>(),
                std::ptr::null(),
                0,
            )
        };
        for value in 0..count as i32 {
            assert_eq!(
                unsafe {
                    crate::deque::zirk_rt_deque_push_back(deque, (&value as *const i32).cast())
                },
                0
            );
        }
        let mut value = 0_i32;
        for _ in 0..count {
            assert_eq!(
                unsafe {
                    crate::deque::zirk_rt_deque_pop_front(deque, (&mut value as *mut i32).cast())
                },
                1
            );
        }
        let deque_elapsed = started.elapsed();

        let started = Instant::now();
        let stack = unsafe {
            crate::deque::zirk_rt_deque_new_with_layout(
                std::mem::size_of::<i32>(),
                std::ptr::null(),
                0,
            )
        };
        for value in 0..count as i32 {
            assert_eq!(
                unsafe {
                    crate::deque::zirk_rt_deque_push_front(stack, (&value as *const i32).cast())
                },
                0
            );
        }
        for _ in 0..count {
            assert_eq!(
                unsafe {
                    crate::deque::zirk_rt_deque_pop_front(stack, (&mut value as *mut i32).cast())
                },
                1
            );
        }
        let queue_stack_elapsed = started.elapsed();

        let started = Instant::now();
        let priority = unsafe {
            crate::priority_queue::zirk_rt_priority_queue_new_with_layout(
                std::mem::size_of::<i32>(),
                std::ptr::null(),
                0,
                crate::priority_queue::PriorityOrder::I32 as i32,
            )
        };
        for value in (0..count as i32).rev() {
            assert_eq!(
                unsafe {
                    crate::priority_queue::zirk_rt_priority_queue_enqueue(
                        priority,
                        (&value as *const i32).cast(),
                    )
                },
                0
            );
        }
        for _ in 0..count {
            assert_eq!(
                unsafe {
                    crate::priority_queue::zirk_rt_priority_queue_dequeue(
                        priority,
                        (&mut value as *mut i32).cast(),
                    )
                },
                1
            );
        }
        let priority_elapsed = started.elapsed();

        let started = Instant::now();
        let map = unsafe {
            crate::map::zirk_rt_map_new_with_layout(
                std::ptr::null(),
                0,
                WORD,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                false,
                WORD,
            )
        };
        let set =
            unsafe { crate::set::zirk_rt_set_new_with_layout(std::ptr::null(), 0, WORD, false) };
        for key in 0..count {
            assert_eq!(
                unsafe {
                    crate::map::zirk_rt_map_set_typed(
                        map,
                        (&key as *const i64).cast(),
                        WORD,
                        (&key as *const i64).cast(),
                        WORD,
                    )
                },
                0
            );
            assert_eq!(
                unsafe {
                    crate::set::zirk_rt_set_add_typed(set, (&key as *const i64).cast(), WORD)
                },
                1
            );
        }
        for key in 0..count {
            let mut output = 0_i64;
            assert_eq!(
                unsafe {
                    crate::map::zirk_rt_map_get_typed(
                        map,
                        key,
                        (&mut output as *mut i64).cast(),
                        WORD,
                    )
                },
                1
            );
            assert_eq!(
                unsafe {
                    crate::set::zirk_rt_set_contains_typed(set, (&key as *const i64).cast(), WORD)
                },
                1
            );
        }
        let map_set_elapsed = started.elapsed();
        println!(
            "n={count}: deque back/front={deque_elapsed:?}; queue/stack FIFO/LIFO={queue_stack_elapsed:?}; priority enqueue/dequeue={priority_elapsed:?}; map/set insert+lookup={map_set_elapsed:?}"
        );
    }
}
