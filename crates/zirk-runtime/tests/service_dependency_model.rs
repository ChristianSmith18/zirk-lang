//! Deterministic dependency-graph traces against the public service API.

use std::panic::{AssertUnwindSafe, catch_unwind};

use zirk_runtime::{ServiceError, ServiceId, ServiceState, ServiceSupervisor};

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

#[test]
fn seeded_dependency_graphs_gate_startup_and_drain_in_actual_reverse_order() {
    const ROUNDS: usize = 64;
    const SERVICES: usize = 12;

    for round in 0..ROUNDS {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64 ^ round as u64;
        let supervisor = ServiceSupervisor::with_capacity(SERVICES + 1).unwrap();
        let mut dependencies: Vec<Vec<ServiceId>> = Vec::with_capacity(SERVICES);

        for index in 0..SERVICES {
            let deps = (0..index)
                .filter(|_| next(&mut seed).is_multiple_of(5))
                .map(|prior| ServiceId(prior as u64))
                .collect::<Vec<_>>();
            let id = supervisor
                .register_with_dependencies(format!("service-{index}"), false, &deps)
                .unwrap();
            assert_eq!(id, ServiceId(index as u64));
            dependencies.push(deps);
        }

        // Rejected edges must not consume an ID or mutate the registration set.
        let unknown = ServiceId(SERVICES as u64 + 10);
        assert_eq!(
            supervisor.register_with_dependencies("invalid", false, &[unknown]),
            Err(ServiceError::UnknownDependency(unknown))
        );
        assert_eq!(
            supervisor.register("after-invalid", false),
            Ok(ServiceId(SERVICES as u64))
        );
        assert_eq!(supervisor.services().len(), SERVICES + 1);

        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Ready));
        let mut start_order: Vec<usize> = (0..SERVICES).collect();
        // Deterministically vary the order while retaining a valid DAG schedule.
        for index in (1..SERVICES).rev() {
            let swap = next(&mut seed) as usize % (index + 1);
            start_order.swap(index, swap);
        }

        let mut started = Vec::new();
        let mut pending = start_order;
        while !pending.is_empty() {
            let before = pending.len();
            let mut next_pending = Vec::new();
            for index in pending {
                let id = ServiceId(index as u64);
                if dependencies[index].iter().all(|dependency| {
                    supervisor.service_state(*dependency) == Ok(ServiceState::Ready)
                }) {
                    supervisor.begin_service(id).unwrap();
                    supervisor.report_ready(id).unwrap();
                    started.push(id);
                } else {
                    assert_eq!(
                        supervisor.begin_service(id),
                        Err(ServiceError::DependencyNotReady(
                            dependencies[index]
                                .iter()
                                .copied()
                                .find(|dependency| {
                                    supervisor.service_state(*dependency) != Ok(ServiceState::Ready)
                                })
                                .unwrap()
                        )),
                        "round {round}, service {index} started before a prerequisite"
                    );
                    next_pending.push(index);
                }
            }
            assert!(next_pending.len() < before, "dependency schedule stalled");
            pending = next_pending;
        }

        assert_eq!(supervisor.lifecycle(), ServiceState::Ready);
        let expected_stop = started.iter().rev().copied().collect::<Vec<_>>();
        assert_eq!(supervisor.request_shutdown(), Ok(expected_stop.clone()));
        for id in expected_stop {
            supervisor.notify_stopped(id).unwrap();
        }
        assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
        assert!(
            supervisor
                .services()
                .iter()
                .all(|(_, _, _, state)| *state == ServiceState::Stopped)
        );
    }
}

/// Exercises the host-side contract around the bookkeeping supervisor: callback
/// panics become retained failures, and one cleanup panic must not prevent the
/// remaining services from being drained and acknowledged.
#[test]
fn startup_and_cleanup_panics_are_captured_and_reverse_drain_completes() {
    let supervisor = ServiceSupervisor::with_capacity(4).unwrap();
    let database = supervisor.register("database", true).unwrap();
    let cache = supervisor
        .register_with_dependencies("cache", true, &[database])
        .unwrap();
    let api = supervisor
        .register_with_dependencies("api", false, &[cache])
        .unwrap();
    let never_started = supervisor.register("metrics", false).unwrap();

    assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));
    let mut acquired = Vec::new();
    for (id, name, callback_panics) in [
        (database, "database", false),
        (cache, "cache", true),
        (api, "api", false),
    ] {
        // Dependencies are reported ready before their dependents start.
        supervisor.begin_service(id).unwrap();
        acquired.push(name);
        let startup = catch_unwind(AssertUnwindSafe(|| {
            if callback_panics {
                panic!("injected startup panic: {name}");
            }
        }));
        if let Err(payload) = startup {
            assert_eq!(
                panic_message(payload.as_ref()),
                "injected startup panic: cache"
            );
            let cleanup_order = supervisor
                .fail_startup(id, panic_message(payload.as_ref()))
                .unwrap();

            // The failed startup callback may already have acquired resources,
            // so its cleanup runs first. Its panic is captured, and does not
            // abort cleanup of earlier services.
            let mut cleanup_attempts = Vec::new();
            for (cleanup_id, cleanup_name) in [(cache, "cache"), (database, "database")] {
                assert_eq!(cleanup_order[cleanup_attempts.len()], cleanup_id);
                cleanup_attempts.push(cleanup_name);
                let cleanup = catch_unwind(AssertUnwindSafe(|| {
                    if cleanup_name == "cache" {
                        panic!("injected cleanup panic: cache");
                    }
                }));
                if let Err(cleanup_payload) = cleanup {
                    assert_eq!(
                        panic_message(cleanup_payload.as_ref()),
                        "injected cleanup panic: cache"
                    );
                }
                supervisor.notify_stopped(cleanup_id).unwrap();
            }

            assert_eq!(cleanup_attempts, ["cache", "database"]);
            assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
            assert_eq!(supervisor.service_state(api), Ok(ServiceState::Stopped));
            assert_eq!(
                supervisor.service_state(never_started),
                Ok(ServiceState::Stopped)
            );
            assert_eq!(
                supervisor
                    .failures()
                    .iter()
                    .map(|failure| (
                        failure.service,
                        failure.during_startup,
                        failure.message.as_str(),
                    ))
                    .collect::<Vec<_>>(),
                [(cache, true, "injected startup panic: cache")]
            );
            return;
        }
        supervisor.report_ready(id).unwrap();
    }

    panic!("injected startup panic was not observed; acquired={acquired:?}");
}

#[test]
fn runtime_service_panic_requests_complete_reverse_cleanup() {
    let supervisor = ServiceSupervisor::with_capacity(3).unwrap();
    let first = supervisor.register("first", true).unwrap();
    let second = supervisor
        .register_with_dependencies("second", true, &[first])
        .unwrap();
    let third = supervisor
        .register_with_dependencies("third", false, &[second])
        .unwrap();
    assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));

    for id in [first, second, third] {
        supervisor.begin_service(id).unwrap();
        supervisor.report_ready(id).unwrap();
    }

    let runtime = catch_unwind(AssertUnwindSafe(|| {
        panic!("injected runtime panic: third");
    }));
    let payload = runtime.expect_err("injected service panic must be caught by host boundary");
    let panic = panic_message(payload.as_ref());
    let stop_order = supervisor.fail_runtime(third, panic.clone()).unwrap();
    assert_eq!(stop_order, [third, second, first]);
    assert_eq!(supervisor.failures()[0].message, panic);

    let mut cleanup_order = Vec::new();
    for id in stop_order {
        cleanup_order.push(id);
        // A cleanup panic is contained per callback. Remaining callbacks must
        // still run, and each terminal acknowledgement advances the drain.
        let cleanup = catch_unwind(AssertUnwindSafe(|| {
            if id == second {
                panic!("injected cleanup panic: second");
            }
        }));
        if let Err(payload) = cleanup {
            assert_eq!(
                panic_message(payload.as_ref()),
                "injected cleanup panic: second"
            );
        }
        supervisor.notify_stopped(id).unwrap();
    }

    assert_eq!(cleanup_order, [third, second, first]);
    assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
    assert!(
        supervisor
            .services()
            .iter()
            .all(|(_, _, _, state)| *state == ServiceState::Stopped)
    );
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|message| (*message).to_owned())
        })
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}
