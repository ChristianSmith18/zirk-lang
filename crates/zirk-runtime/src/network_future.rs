//! Bridge protocol futures to Zirk's owning stackful executor.
//!
//! Lightweight transport futures are polled on their owning Zirk task stack;
//! stack-heavy protocol state machines can instead run on the native reactor
//! through [`spawn_on_reactor`]. Tokio wakers publish a task id when resuming
//! Zirk work; reactor workers never enter generated code or inspect task-local
//! GC roots.

use std::future::Future;
use std::pin::{Pin, pin};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use tokio::task::JoinHandle;

use crate::executor::{
    current_executor_id, current_task, current_task_cancel_requested, network_event_epoch,
    park_current_for_network_event_if_unchanged,
};
use crate::task::TaskId;

static NETWORK_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    NETWORK_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_io()
            .enable_time()
            .thread_name("zirk-network-reactor")
            .build()
            .expect("failed to initialize Zirk's network reactor")
    })
}

/// A native protocol operation polled by the bounded Tokio reactor instead
/// of on a 128 KiB Zirk task stack. Dropping the owner future aborts the Tokio
/// operation, so cancellation cannot leave a detached handshake running.
pub(crate) struct ReactorTask<T>(JoinHandle<T>);

impl<T> Future for ReactorTask<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(context)
    }
}

impl<T> Drop for ReactorTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Schedules stack-heavy native protocol work on Tokio's OS worker. The
/// returned future is awaited through [`block_on_current`], which resumes the
/// originating Zirk task when the worker publishes its result.
pub(crate) fn spawn_on_reactor<F>(future: F) -> ReactorTask<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    ReactorTask(runtime().spawn(future))
}

/// Starts a reactor-owned protocol task whose lifetime is controlled by an
/// explicit abort handle held by the owning Zirk resource.
pub(crate) fn spawn_detached_on_reactor<F>(future: F) -> tokio::task::AbortHandle
where
    F: Future<Output = ()> + Send + 'static,
{
    runtime().spawn(future).abort_handle()
}

struct TaskWake {
    executor_id: u64,
    task: TaskId,
}

impl Wake for TaskWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        crate::executor::notify_network_task(self.executor_id, self.task);
    }
}

/// Polls a lightweight Rust I/O future on the current Zirk task stack.
///
/// A pending future parks the current task. Tokio's reactor runs independently
/// and wakes this task through the executor's targeted wake queue. Cancellation
/// drops the future on the task's stack before returning to the caller.
pub(crate) fn block_on_current<F: Future>(future: F) -> Result<F::Output, NetworkFutureError> {
    let task = current_task();
    let waker = Waker::from(Arc::new(TaskWake {
        executor_id: current_executor_id(),
        task,
    }));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        if current_task_cancel_requested() {
            return Err(NetworkFutureError::Cancelled);
        }

        // Network resources such as TcpStream bind to the Tokio runtime when
        // first polled. Enter only around the poll; no protocol code runs on a
        // Tokio worker thread.
        let epoch = network_event_epoch();
        let result = {
            let _entered = runtime().handle().enter();
            future.as_mut().poll(&mut context)
        };
        match result {
            Poll::Ready(output) => return Ok(output),
            Poll::Pending => match park_current_for_network_event_if_unchanged(epoch) {
                crate::executor::NetworkPark::EventOccurred => continue,
                crate::executor::NetworkPark::Parked => {}
                crate::executor::NetworkPark::CapacityExceeded => {
                    return Err(NetworkFutureError::CapacityExceeded);
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetworkFutureError {
    Cancelled,
    CapacityExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{Executor, await_task, cancel_current, spawn, yield_now};
    use bytes::Bytes;
    use http_body_util::{BodyExt, Full};
    use hyper::{Method, Request, Response, StatusCode};
    use hyper_util::rt::TokioIo;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct PendingDropProbe(Arc<std::sync::atomic::AtomicBool>);

    impl Future for PendingDropProbe {
        type Output = ();

        fn poll(self: std::pin::Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    impl Drop for PendingDropProbe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[test]
    fn loopback_read_parks_only_its_task_and_resumes_on_the_owner() {
        let _gc_guard = crate::collector::test_support::test_guard();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let progress = Arc::new(AtomicUsize::new(0));
        let rooted_handle = Arc::new(AtomicUsize::new(0));

        let progress_task = Arc::clone(&progress);
        let progress_rooted_handle = Arc::clone(&rooted_handle);
        let outcome = Executor::new().run_with_root(move || {
            let server_task = spawn(move || {
                let (stream, _) = block_on_current(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    listener.accept().await.unwrap()
                })
                .unwrap();
                let server_thread = std::thread::current().id();
                let handler =
                    hyper::service::service_fn(move |request: Request<hyper::body::Incoming>| {
                        assert_eq!(
                            std::thread::current().id(),
                            server_thread,
                            "HTTP handler must execute on its owning Zirk task"
                        );
                        let _task = crate::executor::current_task();
                        async move {
                            assert_eq!(*request.method(), Method::from_bytes(b"QUERY").unwrap());
                            assert_eq!(request.uri().path(), "/probe");
                            assert_eq!(
                                request.headers().get("content-type").unwrap(),
                                "application/query+json"
                            );
                            assert_eq!(
                                request.into_body().collect().await.unwrap().to_bytes(),
                                Bytes::from_static(b"{\"q\":\"zirk\"}")
                            );
                            Ok::<_, std::convert::Infallible>(
                                Response::builder()
                                    .status(StatusCode::OK)
                                    .body(Full::new(Bytes::from_static(b"ok")))
                                    .unwrap(),
                            )
                        }
                    });
                let connection = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), handler);
                block_on_current(connection).unwrap().unwrap();
                19
            });
            let client_task = spawn(move || {
                let object = unsafe { crate::zirk_rt_alloc(64, 8) };
                unsafe { *((object as *mut usize).add(3)) = 0x0ddc_0ffe };
                rooted_handle.store(object as usize, Ordering::Release);
                let mut rooted = object;
                let mut roots = [(&mut rooted as *mut *mut std::ffi::c_void).cast()];
                unsafe { crate::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
                let stream = block_on_current(async move {
                    tokio::net::TcpStream::connect(address).await.unwrap()
                })
                .unwrap();
                let (mut sender, connection) =
                    block_on_current(hyper::client::conn::http1::handshake(TokioIo::new(stream)))
                        .unwrap()
                        .unwrap();
                {
                    let _entered = runtime().handle().enter();
                    tokio::spawn(async move {
                        let _ = connection.await;
                    });
                }
                let request = Request::builder()
                    .method(Method::from_bytes(b"QUERY").unwrap())
                    .uri("/probe")
                    .header("host", address.to_string())
                    .header("content-type", "application/query+json")
                    .header("connection", "close")
                    .body(Full::new(Bytes::from_static(b"{\"q\":\"zirk\"}")))
                    .unwrap();
                let response = block_on_current(sender.send_request(request))
                    .unwrap()
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let body = block_on_current(response.into_body().collect())
                    .unwrap()
                    .unwrap()
                    .to_bytes();
                assert_eq!(&body[..], b"ok");
                assert_eq!(unsafe { *((rooted as *mut usize).add(3)) }, 0x0ddc_0ffe);
                crate::zirk_rt_pop_frame();
                17
            });
            let unrelated = spawn(move || {
                // Collect while the socket task is parked. Its generated-style
                // shadow frame must remain discoverable from the suspended TCB.
                crate::collector::collect_for_tests();
                assert!(crate::collector::is_live_for_tests(
                    progress_rooted_handle.load(Ordering::Acquire) as *mut std::ffi::c_void
                ));
                for _ in 0..8 {
                    progress_task.fetch_add(1, Ordering::AcqRel);
                    yield_now();
                }
                0
            });
            assert_eq!(await_task(client_task), 17);
            assert_eq!(await_task(server_task), 19);
            assert_eq!(await_task(unrelated), 0);
            assert!(progress.load(Ordering::Acquire) >= 3);
            0
        });

        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn cancellation_drops_a_pending_future_on_its_task_stack() {
        let observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task_observed = Arc::clone(&observed);
        let future_dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task_future_dropped = Arc::clone(&future_dropped);
        let outcome = Executor::new().run_with_root(move || {
            let waiting = spawn(move || {
                assert_eq!(
                    block_on_current(PendingDropProbe(task_future_dropped.clone())).unwrap_err(),
                    NetworkFutureError::Cancelled
                );
                assert!(task_future_dropped.load(Ordering::Acquire));
                task_observed.store(true, Ordering::Release);
                0
            });
            yield_now();
            cancel_current(waiting);
            yield_now();
            assert!(observed.load(Ordering::Acquire));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        assert!(future_dropped.load(Ordering::Acquire));
    }

    #[test]
    fn reactor_task_runs_protocol_work_on_its_native_worker() {
        let outcome = Executor::new().run_with_root(|| {
            let owner_thread = std::thread::current().id();
            let worker_thread = block_on_current(spawn_on_reactor(async move {
                assert_ne!(std::thread::current().id(), owner_thread);
                std::thread::current()
                    .name()
                    .expect("Tokio worker has a diagnostic name")
                    .to_owned()
            }))
            .expect("reactor completion wakes the Zirk task")
            .expect("reactor task did not panic");
            assert_eq!(worker_thread, "zirk-network-reactor");
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn cancelling_a_zirk_wait_aborts_its_reactor_operation() {
        struct DropFlag(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task_started = Arc::clone(&started);
        let task_dropped = Arc::clone(&dropped);
        let outcome = Executor::new().run_with_root(move || {
            let waiting = spawn(move || {
                let reactor_started = Arc::clone(&task_started);
                let reactor_dropped = Arc::clone(&task_dropped);
                let operation = spawn_on_reactor(async move {
                    let _drop_flag = DropFlag(reactor_dropped);
                    reactor_started.store(true, Ordering::Release);
                    std::future::pending::<()>().await;
                });
                assert_eq!(
                    block_on_current(operation).unwrap_err(),
                    NetworkFutureError::Cancelled
                );
                0
            });

            while !started.load(Ordering::Acquire) {
                yield_now();
            }
            cancel_current(waiting);
            yield_now();
            for _ in 0..10_000 {
                if dropped.load(Ordering::Acquire) {
                    break;
                }
                std::thread::yield_now();
            }
            assert!(dropped.load(Ordering::Acquire));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
    }

    #[test]
    fn tokio_timeout_wakes_the_owning_zirk_task() {
        use std::time::Duration;

        let outcome = Executor::new().run_with_root(|| {
            let timeout = {
                let _entered = runtime().handle().enter();
                tokio::time::timeout(Duration::from_millis(5), std::future::pending::<()>())
            };
            let result = block_on_current(timeout).expect("timer is not cancellation");
            assert!(result.is_err(), "pending operation should time out");
            23
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(23)));
    }
}
