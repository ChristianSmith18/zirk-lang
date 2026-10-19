//! Host-to-runtime shutdown notifications and generated-CLI signal handling.
//!
//! The OS handler only publishes a lock-free atomic notification. Executor
//! cancellation, cleanup, logging, and process termination run on ordinary
//! threads. OS handlers are installed only by `zirk_rt_run_main_cli`, and only
//! when the inherited disposition is the default action.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const DEFAULT_GRACE_NANOS: u64 = 5_000_000_000;
const WATCHDOG_POLL: Duration = Duration::from_millis(1);

struct SignalState {
    pending: AtomicUsize,
    // Retains the first signal even after the cooperative executor consumes
    // `pending`. The CLI watchdog needs to keep enforcing the same shutdown
    // grace while the executor is blocked in native code.
    watchdog_signal: AtomicUsize,
}

impl SignalState {
    const fn new() -> Self {
        Self {
            pending: AtomicUsize::new(0),
            watchdog_signal: AtomicUsize::new(0),
        }
    }

    fn request(&self, signal: i32) -> bool {
        if !(1..=127).contains(&signal) {
            return false;
        }
        if self
            .pending
            .compare_exchange(0, signal as usize, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let _ = self.watchdog_signal.compare_exchange(
            0,
            signal as usize,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        true
    }

    fn take(&self) -> Option<i32> {
        let signal = self.pending.swap(0, Ordering::AcqRel);
        (signal != 0).then_some(signal as i32)
    }

    fn reset(&self) {
        self.pending.store(0, Ordering::Release);
        self.watchdog_signal.store(0, Ordering::Release);
    }
}

static SIGNAL_STATE: SignalState = SignalState::new();
static SHUTDOWN_GRACE_NANOS: AtomicU64 = AtomicU64::new(DEFAULT_GRACE_NANOS);

pub fn request(signal: i32) -> bool {
    SIGNAL_STATE.request(signal)
}

pub fn take() -> Option<i32> {
    SIGNAL_STATE.take()
}

pub fn reset() {
    SIGNAL_STATE.reset();
    SHUTDOWN_GRACE_NANOS.store(DEFAULT_GRACE_NANOS, Ordering::Release);
}

pub(crate) fn set_shutdown_grace_nanos(nanos: u64) {
    SHUTDOWN_GRACE_NANOS.store(nanos, Ordering::Release);
}

/// Installs handlers for the generated CLI only. On Unix, inherited ignored
/// and custom dispositions are left in place, and the guard restores a
/// disposition only if the signal still points at our handler. On Windows,
/// the guard adds and removes a console-control callback for Ctrl-C/Break.
#[cfg(unix)]
pub struct CliSignalHandlers {
    previous: Vec<(libc::c_int, libc::sigaction, usize)>,
}

#[cfg(windows)]
pub struct CliSignalHandlers {
    installed: bool,
}

#[cfg(not(any(unix, windows)))]
pub struct CliSignalHandlers;

#[cfg(unix)]
unsafe extern "C" fn os_signal_handler(signal: libc::c_int) {
    // SAFETY: `request` performs only range checks and lock-free atomic
    // operations. It does not allocate, lock, log, or call executor code.
    let _ = request(signal);
}

#[cfg(unix)]
pub fn install_cli_signal_handlers() -> CliSignalHandlers {
    let mut previous = Vec::new();
    for signal in [libc::SIGINT, libc::SIGTERM] {
        // SAFETY: zero is a valid initial representation for sigaction, then
        // sigemptyset initializes its mask before the structure is installed.
        let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
        // SAFETY: querying the process disposition for a valid signal.
        if unsafe { libc::sigaction(signal, std::ptr::null(), &mut old) } != 0 {
            continue;
        }
        // Do not replace an inherited ignore or a handler installed by the
        // embedding host. The generated CLI normally starts with SIG_DFL.
        if old.sa_sigaction != libc::SIG_DFL {
            continue;
        }

        // SAFETY: initialize every field read by sigaction below.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = os_signal_handler as *const () as usize;
        action.sa_flags = 0;
        // SAFETY: action is a valid writable sigaction structure.
        unsafe { libc::sigemptyset(&mut action.sa_mask) };
        // SAFETY: action points to a valid C signal handler and initialized mask.
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } == 0 {
            previous.push((signal, old, os_signal_handler as *const () as usize));
        }
    }

    // Generated Zirk executables do not run Rust's process-startup signal
    // setup. Ignore SIGPIPE while the CLI runtime is active so a disconnected
    // HTTP peer (or closed output pipe) becomes an ordinary EPIPE result.
    let signal = libc::SIGPIPE;
    // SAFETY: querying the process disposition for a valid signal.
    let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
    if unsafe { libc::sigaction(signal, std::ptr::null(), &mut old) } == 0
        && old.sa_sigaction == libc::SIG_DFL
    {
        // SAFETY: zero is valid initial storage and sigemptyset initializes
        // the mask before installing the process-wide ignore disposition.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = libc::SIG_IGN;
        action.sa_flags = 0;
        // SAFETY: action is initialized for the SIGPIPE disposition.
        unsafe { libc::sigemptyset(&mut action.sa_mask) };
        // SAFETY: action contains a valid SIG_IGN disposition.
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } == 0 {
            previous.push((signal, old, libc::SIG_IGN));
        }
    }
    CliSignalHandlers { previous }
}

#[cfg(windows)]
pub fn install_cli_signal_handlers() -> CliSignalHandlers {
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

    // SAFETY: the handler only publishes a lock-free atomic notification and
    // remains installed only for the lifetime of the generated CLI run.
    let installed = unsafe { SetConsoleCtrlHandler(Some(windows_console_handler), 1) } != 0;
    CliSignalHandlers { installed }
}

#[cfg(windows)]
unsafe extern "system" fn windows_console_handler(ctrl_type: u32) -> i32 {
    let Some(signal) = windows_console_signal(ctrl_type) else {
        return 0;
    };
    // This callback runs on an OS-created console-control thread. `request`
    // uses only lock-free atomics; cancellation and cleanup happen later on
    // the executor thread. Claim known events so Windows does not terminate
    // the process before the generated CLI deadline policy can run.
    let _ = request(signal);
    1
}

#[cfg(windows)]
fn windows_console_signal(ctrl_type: u32) -> Option<i32> {
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};

    match ctrl_type {
        CTRL_C_EVENT | CTRL_BREAK_EVENT => Some(2),
        _ => None,
    }
}

#[cfg(not(any(unix, windows)))]
pub fn install_cli_signal_handlers() -> CliSignalHandlers {
    CliSignalHandlers
}

#[cfg(unix)]
impl Drop for CliSignalHandlers {
    fn drop(&mut self) {
        for (signal, old, installed) in self.previous.drain(..) {
            // SAFETY: querying the current action for a valid signal.
            let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigaction(signal, std::ptr::null(), &mut current) } != 0 {
                continue;
            }
            if current.sa_sigaction != installed {
                continue;
            }
            // SAFETY: `old` was returned by sigaction and remains a valid
            // disposition. Restore only while our handler is still installed.
            unsafe { libc::sigaction(signal, &old, std::ptr::null_mut()) };
        }
    }
}

#[cfg(windows)]
impl Drop for CliSignalHandlers {
    fn drop(&mut self) {
        if !self.installed {
            return;
        }
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

        // SAFETY: remove only the callback this guard installed.
        unsafe { SetConsoleCtrlHandler(Some(windows_console_handler), 0) };
    }
}

#[cfg(not(any(unix, windows)))]
impl Drop for CliSignalHandlers {
    fn drop(&mut self) {}
}

/// Starts a CLI-only watchdog. A signal handler cannot run cleanup or enforce
/// a deadline while the executor is stuck in native code, so this ordinary
/// thread performs a hard process exit after the configured grace.
pub struct CliShutdownWatchdog {
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CliShutdownWatchdog {
    pub fn start() -> Self {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("zirk-cli-shutdown-watchdog".into())
            .spawn(move || {
                let mut deadline = None;
                loop {
                    if thread_stop.load(Ordering::Acquire) {
                        return;
                    }
                    if SIGNAL_STATE.watchdog_signal.load(Ordering::Acquire) != 0 {
                        let started = deadline.get_or_insert_with(Instant::now);
                        let grace =
                            Duration::from_nanos(SHUTDOWN_GRACE_NANOS.load(Ordering::Acquire));
                        if started.elapsed() >= grace {
                            hard_exit_124();
                        }
                    }
                    std::thread::sleep(WATCHDOG_POLL);
                }
            })
            .expect("failed to start CLI shutdown watchdog");
        Self {
            stop,
            thread: Some(thread),
        }
    }

    pub fn disarm(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for CliShutdownWatchdog {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(unix)]
fn hard_exit_124() -> ! {
    // `_exit` bypasses destructors and atexit handlers: the CLI deadline must
    // remain enforceable even when arbitrary native code is stuck.
    unsafe { libc::_exit(124) }
}

#[cfg(windows)]
fn hard_exit_124() -> ! {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ExitProcess(exit_code: u32) -> !;
    }
    // SAFETY: this is the process-level hard-deadline policy for generated CLI.
    unsafe { ExitProcess(124) }
}

#[cfg(not(any(unix, windows)))]
fn hard_exit_124() -> ! {
    std::process::exit(124)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn request_is_portable_atomic_and_first_signal_wins() {
        let state = SignalState::new();
        assert!(!state.request(0));
        assert!(!state.request(128));
        assert!(state.request(2));
        assert!(!state.request(15));
        assert_eq!(state.take(), Some(2));
        assert_eq!(state.take(), None);
        assert_eq!(state.watchdog_signal.load(Ordering::Acquire), 2);
    }

    #[cfg(windows)]
    #[test]
    fn windows_console_ctrl_c_and_break_map_to_sigint() {
        use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};

        assert_eq!(windows_console_signal(CTRL_C_EVENT), Some(2));
        assert_eq!(windows_console_signal(CTRL_BREAK_EVENT), Some(2));
        assert_eq!(windows_console_signal(u32::MAX), None);
    }

    #[test]
    fn concurrent_signal_requests_publish_exactly_one_winner_under_stress() {
        for _ in 0..64 {
            let state = SignalState::new();
            let start = Barrier::new(9);
            thread::scope(|scope| {
                let workers: Vec<_> = (1..=8)
                    .map(|signal| {
                        let start = &start;
                        let state = &state;
                        scope.spawn(move || {
                            start.wait();
                            state.request(signal)
                        })
                    })
                    .collect();
                start.wait();
                let accepted = workers
                    .into_iter()
                    .map(|worker| worker.join().expect("signal requester joins"))
                    .filter(|accepted| *accepted)
                    .count();
                assert_eq!(accepted, 1);
            });
            let winner = state.take().expect("one signal was published");
            assert!((1..=8).contains(&winner));
            assert_eq!(state.pending.load(Ordering::Acquire), 0);
        }
    }
}
