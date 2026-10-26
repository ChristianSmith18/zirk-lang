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
    /// Allocates an inline GC-traced mutex payload: `(ptr, usize, usize, ptr,
    /// usize, ptr) -> ptr`, where the final pointer receives a status code.
    pub const GC_MUTEX_ALLOC: &str = "zirk_rt_gc_mutex_alloc";
    /// Acquires a mutex and writes the borrowed payload pointer/status out.
    pub const GC_MUTEX_ACQUIRE: &str = "zirk_rt_gc_mutex_acquire";
    /// Releases the current participant's acquisition and writes status out.
    pub const GC_MUTEX_RELEASE: &str = "zirk_rt_gc_mutex_release";
    /// Roots a long-lived managed object and returns its runtime root handle.
    pub const GLOBAL_ROOT_ADD: &str = "zirk_rt_global_root_add";
    /// Initializes revision/subscriber state for one project-global cell.
    pub const GLOBAL_WATCH_RESET: &str = "zirk_rt_global_watch_reset";
    /// Registers a typed event channel and returns token plus snapshot revision.
    pub const GLOBAL_WATCH_SUBSCRIBE: &str = "zirk_rt_global_watch_subscribe";
    /// Publishes a change to bounded subscribers while the global cell is locked.
    pub const GLOBAL_WATCH_PUBLISH: &str = "zirk_rt_global_watch_publish";
    /// Reads a queued change or terminal subscription status without blocking.
    pub const GLOBAL_WATCH_NEXT: &str = "zirk_rt_global_watch_next";
    /// Closes a subscription and releases its managed channel root.
    pub const GLOBAL_WATCH_CLOSE: &str = "zirk_rt_global_watch_close";
    /// Runs a synchronous borrowed-payload callback and releases via RAII.
    pub const GC_MUTEX_WITH: &str = "zirk_rt_gc_mutex_with";
    pub const ATOMIC_MANAGED_NEW: &str = "zirk_rt_atomic_managed_new";
    pub const ATOMIC_MANAGED_LOAD: &str = "zirk_rt_atomic_managed_load";
    pub const ATOMIC_MANAGED_LOAD_ORDERED: &str = "zirk_rt_atomic_managed_load_ordered";
    pub const ATOMIC_MANAGED_STORE: &str = "zirk_rt_atomic_managed_store";
    pub const ATOMIC_MANAGED_STORE_ORDERED: &str = "zirk_rt_atomic_managed_store_ordered";
    pub const ATOMIC_MANAGED_EXCHANGE: &str = "zirk_rt_atomic_managed_exchange";
    pub const ATOMIC_MANAGED_EXCHANGE_ORDERED: &str = "zirk_rt_atomic_managed_exchange_ordered";
    pub const ATOMIC_MANAGED_COMPARE_EXCHANGE: &str = "zirk_rt_atomic_managed_compare_exchange";
    pub const ATOMIC_MANAGED_FETCH_ADD: &str = "zirk_rt_atomic_managed_fetch_add";
    pub const ATOMIC_MANAGED_FETCH_ADD_ORDERED: &str = "zirk_rt_atomic_managed_fetch_add_ordered";
    pub const ATOMIC_MANAGED_FETCH_SUB: &str = "zirk_rt_atomic_managed_fetch_sub";
    pub const ATOMIC_MANAGED_FETCH_SUB_ORDERED: &str = "zirk_rt_atomic_managed_fetch_sub_ordered";
    /// Creates an opaque, runtime-aligned scalar AtomicCell handle.
    pub const ATOMIC_NEW: &str = "zirk_rt_atomic_new";
    /// Frees an AtomicCell handle after all concurrent users have stopped.
    pub const ATOMIC_FREE: &str = "zirk_rt_atomic_free";
    /// Sequentially consistent AtomicCell load.
    pub const ATOMIC_LOAD: &str = "zirk_rt_atomic_load";
    /// Sequentially consistent AtomicCell store.
    pub const ATOMIC_STORE: &str = "zirk_rt_atomic_store";
    /// Sequentially consistent AtomicCell exchange.
    pub const ATOMIC_EXCHANGE: &str = "zirk_rt_atomic_exchange";
    /// Atomic compare-exchange; runtime validates both memory orderings.
    pub const ATOMIC_COMPARE_EXCHANGE: &str = "zirk_rt_atomic_compare_exchange";
    /// Sequentially consistent wrapping integer addition.
    pub const ATOMIC_FETCH_ADD: &str = "zirk_rt_atomic_fetch_add";
    /// Sequentially consistent wrapping integer subtraction.
    pub const ATOMIC_FETCH_SUB: &str = "zirk_rt_atomic_fetch_sub";

    /// Initializes the runtime before `main`.
    pub const INIT: &str = "zirk_rt_init";
    /// Runs the Zirk entrypoint as the cooperative executor's root task.
    pub const RUN_MAIN: &str = "zirk_rt_run_main";
    /// CLI host entrypoint; exits with status 124 at background-drain expiry.
    pub const RUN_MAIN_CLI: &str = "zirk_rt_run_main_cli";
    pub const APP_SET_SHUTDOWN_GRACE: &str = "zirk_rt_app_set_shutdown_grace";
    pub const APP_SET_BACKGROUND_ERROR: &str = "zirk_rt_app_set_background_error";
    pub const BACKGROUND_HANDLER_FAILED: &str = "zirk_rt_background_handler_failed";
    pub const SERVICE_REGISTER: &str = "zirk_rt_service_register";
    pub const SERVICE_READY: &str = "zirk_rt_service_ready";
    pub const SERVICE_FAILED_CURRENT: &str = "zirk_rt_service_failed_current";
    /// Starts a child scheduler task from a body thunk and boxed-callable
    /// capture block. This provisional symbol is reused and renamed by
    /// `concurrent-blocks-and-timers`.
    pub const SPAWN: &str = "zirk_rt_spawn_checked";
    /// Starts a detached task owned by the bounded background supervisor.
    pub const BACKGROUND_SPAWN: &str = "zirk_rt_background_spawn";
    /// Suspends the current scheduler task until a child result is available.
    /// This provisional symbol is reused and renamed by
    /// `concurrent-blocks-and-timers`.
    pub const JOB_WAIT: &str = "zirk_rt_job_wait";
    /// Runs one verified zero-argument Zirk callback on a registered managed
    /// native worker and joins it before returning: `i32 (ptr, ptr, ptr, ptr, ptr)`.
    pub const THREAD_RUN: &str = "zirk_rt_thread_run";
    /// Waits for the earliest serialized completion among 2–8 job IDs and
    /// returns its input index as `u64`; `u64::MAX` denotes caller cancellation.
    pub const JOB_WAIT_ANY: &str = "zirk_rt_job_wait_any";
    pub const JOB_WAIT_ANY_WITH_TIMER: &str = "zirk_rt_job_wait_any_with_timer";
    /// Waits for a terminal job outcome and transfers managed payload pointers
    /// into caller-rooted slots: `i32 (i64, ptr, ptr)`.
    pub const JOB_OUTCOME_WAIT_TAKE: &str = "zirk_rt_job_outcome_wait_take";
    pub const JOB_OUTCOME_TAKE: &str = "zirk_rt_job_outcome_take";
    /// Reports whether a job has reached a terminal state.
    pub const JOB_DONE: &str = "zirk_rt_job_done";
    /// `zirk_rt_parallel_for(kind, a, b, count, thunk, capture)` — the
    /// `parallel { for x in coll { ... } }` work-splitting dispatch
    /// (`parallel-cpu-regions` task 6.1).
    pub const PARALLEL_FOR: &str = "zirk_rt_parallel_for";
    /// Polls the runtime safepoint from generated code (`void ()`).
    pub const SAFEPOINT_POLL: &str = "zirk_rt_safepoint_poll";
    /// Opens a structured-concurrency scope.
    pub const SCOPE_ENTER: &str = "zirk_rt_scope_enter";
    pub const SCOPE_ENTER_COLLECT_ALL: &str = "zirk_rt_scope_enter_collect_all";
    /// Advances the close protocol for a structured-concurrency scope.
    pub const SCOPE_EXIT: &str = "zirk_rt_scope_exit";
    pub const SCOPE_EXIT_COLLECT_ALL: &str = "zirk_rt_scope_exit_collect_all";
    /// Waits for and takes one scoped branch outcome into caller-rooted slots.
    pub const SCOPE_OUTCOME_WAIT_TAKE: &str = "zirk_rt_scope_outcome_wait_take";
    /// Requests cancellation of all active branches in a scope.
    pub const SCOPE_CANCEL: &str = "zirk_rt_scope_cancel";
    /// Registers a joinable branch with a scope.
    pub const BRANCH_REGISTER: &str = "zirk_rt_branch_register";
    /// Requests cooperative cancellation of a job.
    pub const CANCEL: &str = "zirk_rt_cancel";
    /// Cancels a `Concurrent.first()` loser and promotes cleanup failures.
    pub const CANCEL_WITH_CLEANUP_FAILURE: &str = "zirk_rt_cancel_with_cleanup_failure";
    /// Creates an ambient one-shot timer job.
    pub const TIMER_AFTER: &str = "zirk_rt_timer_after_checked";
    /// Creates an ambient fixed-delay timer job.
    pub const TIMER_EVERY: &str = "zirk_rt_timer_every_checked";
    /// Suspends the current job until a monotonic timer deadline.
    pub const SLEEP: &str = "zirk_rt_sleep_checked";
    /// Creates a typed channel. Elements cross this ABI through indirect slots.
    pub const CHAN_NEW: &str = "zirk_rt_chan_new";
    /// Attempts to send an element through a channel without suspension.
    pub const CHAN_TRY_SEND: &str = "zirk_rt_chan_try_send";
    /// Attempts to receive an element through a channel without suspension.
    pub const CHAN_TRY_RECV: &str = "zirk_rt_chan_try_recv";
    /// Sends an element, suspending cooperatively when the channel is full.
    pub const CHAN_SEND: &str = "zirk_rt_chan_send";
    /// Receives an element, suspending cooperatively while the channel is empty.
    pub const CHAN_RECV: &str = "zirk_rt_chan_recv";
    /// Closes a channel, waking blocked senders and receivers.
    pub const CHAN_CLOSE: &str = "zirk_rt_chan_close";
    /// Returns the queued element count.
    pub const CHAN_LEN: &str = "zirk_rt_chan_len";
    /// Reports whether a channel has been closed.
    pub const CHAN_IS_CLOSED: &str = "zirk_rt_chan_is_closed";
    /// Returns a channel's declared capacity.
    pub const CHAN_CAPACITY: &str = "zirk_rt_chan_capacity";
    /// Drains queued values into a managed list.
    pub const CHAN_DRAIN: &str = "zirk_rt_chan_drain";
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
    /// Converts a `Float128`'s raw bits (taken by pointer, like
    /// `STR_FROM_I128`) to the nearest `Float64`, for `ToString`.
    pub const FLOAT128_TO_F64: &str = "zirk_float128_to_f64";
    /// Widens a `Float64` to `Float128`'s raw bits, written through an
    /// out-pointer like every other 128-bit runtime result.
    pub const F64_TO_FLOAT128: &str = "zirk_f64_to_float128";
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
    /// Writes a `String` to standard output without a line break.
    pub const IO_PRINT: &str = "zirk_io_print";
    /// Writes a `String` to standard error with a line break.
    pub const IO_EPRINTLN: &str = "zirk_io_eprintln";
    /// Writes a `String` to standard error without a line break.
    pub const IO_EPRINT: &str = "zirk_io_eprint";
    /// Flushes standard output; `0` succeeded, `1` failed
    /// (`add-stdin-stdout-standard-library`).
    pub const IO_STDOUT_FLUSH: &str = "zirk_io_stdout_flush";
    /// Flushes standard error; see [`IO_STDOUT_FLUSH`].
    pub const IO_STDERR_FLUSH: &str = "zirk_io_stderr_flush";
    /// Reads one line from standard input into a `zirk_io::StdinReadOut` out
    /// parameter (`add-stdin-stdout-standard-library`).
    pub const IO_STDIN_READ_LINE: &str = "zirk_io_stdin_read_line";
    /// Reads one Unicode scalar value from standard input into a
    /// `zirk_io::StdinReadOut` out parameter.
    pub const IO_STDIN_READ_CHAR: &str = "zirk_io_stdin_read_char";
    /// Reads up to `max` bytes from standard input into a
    /// `zirk_io::StdinReadOut` out parameter.
    pub const IO_STDIN_READ_BYTES: &str = "zirk_io_stdin_read_bytes";
    /// Reads exactly `count` bytes from standard input into a
    /// `zirk_io::StdinReadOut` out parameter.
    pub const IO_STDIN_READ_EXACT: &str = "zirk_io_stdin_read_exact";
    /// Permission-gated standard environment string lookup.
    pub const ENVIRONMENT_GET: &str = "zirk_environment_get";
    /// `zirk_environment_get_secret(name, out)`.
    pub const ENVIRONMENT_GET_SECRET: &str = "zirk_environment_get_secret";
    /// Sets the package identity used to attribute the immediately following
    /// protected runtime operation.
    pub const PERMISSION_REQUESTER_SET: &str = "zirk_rt_permission_requester_set";
    /// Clears the package identity after the protected runtime operation.
    pub const PERMISSION_REQUESTER_CLEAR: &str = "zirk_rt_permission_requester_clear";
    /// `zirk_environment_contains(name, out)`.
    pub const ENVIRONMENT_CONTAINS: &str = "zirk_environment_contains";
    /// `zirk_environment_list_names(out)`.
    pub const ENVIRONMENT_LIST_NAMES: &str = "zirk_environment_list_names";
    /// Closed `std.fs`/`std.path` operation dispatcher. Its arguments are
    /// compiler-packed machine words and its output is the stable
    /// `(kind, error, scalar, payload)` record.
    pub const FILESYSTEM_CALL: &str = "zirk_fs_call";
    pub const NETWORK_TCP_CONNECT: &str = "zirk_network_tcp_connect";
    pub const NETWORK_UDP_CONNECT: &str = "zirk_network_udp_connect";
    pub const NETWORK_UDP_BIND: &str = "zirk_network_udp_bind";
    pub const NETWORK_UDP_SEND: &str = "zirk_network_udp_send";
    pub const NETWORK_UDP_SEND_TO: &str = "zirk_network_udp_send_to";
    pub const NETWORK_UDP_SEND_BROADCAST: &str = "zirk_network_udp_send_broadcast";
    pub const NETWORK_UDP_SEND_MULTICAST: &str = "zirk_network_udp_send_multicast";
    pub const NETWORK_UDP_JOIN_MULTICAST: &str = "zirk_network_udp_join_multicast";
    pub const NETWORK_UDP_LEAVE_MULTICAST: &str = "zirk_network_udp_leave_multicast";
    pub const NETWORK_UDP_RECEIVE: &str = "zirk_network_udp_receive";
    pub const NETWORK_UDP_LOCAL_ADDRESS: &str = "zirk_network_udp_local_address";
    pub const NETWORK_UDP_CLOSE: &str = "zirk_network_udp_close";
    pub const NETWORK_UDP_IS_CLOSED: &str = "zirk_network_udp_is_closed";
    pub const NETWORK_LOCAL_CONNECT: &str = "zirk_network_local_connect";
    pub const NETWORK_LOCAL_BIND: &str = "zirk_network_local_bind";
    pub const NETWORK_LOCAL_ACCEPT: &str = "zirk_network_local_accept";
    pub const NETWORK_LOCAL_READ: &str = "zirk_network_local_read";
    pub const NETWORK_LOCAL_WRITE: &str = "zirk_network_local_write";
    pub const NETWORK_LOCAL_WRITE_ALL: &str = "zirk_network_local_write_all";
    pub const NETWORK_LOCAL_SHUTDOWN: &str = "zirk_network_local_shutdown";
    pub const NETWORK_LOCAL_CLOSE_STREAM: &str = "zirk_network_local_close_stream";
    pub const NETWORK_LOCAL_CLOSE_LISTENER: &str = "zirk_network_local_close_listener";
    pub const NETWORK_LOCAL_STREAM_IS_CLOSED: &str = "zirk_network_local_stream_is_closed";
    pub const NETWORK_LOCAL_LISTENER_IS_CLOSED: &str = "zirk_network_local_listener_is_closed";
    pub const NETWORK_DNS_RESOLVE: &str = "zirk_network_dns_resolve";
    pub const NETWORK_INTERFACES_SNAPSHOT: &str = "zirk_network_interfaces_snapshot";
    pub const NETWORK_IP_ADDRESS_PARSE: &str = "zirk_network_ip_address_parse";
    pub const NETWORK_TCP_ORIGIN_NORMALIZE: &str = "zirk_network_tcp_origin_normalize";
    pub const NETWORK_TLS_CONNECT: &str = "zirk_network_tls_connect";
    pub const NETWORK_TLS_CONNECT_WITH_ROOTS: &str = "zirk_network_tls_connect_with_roots";
    pub const NETWORK_TLS_CONNECT_WITH_ALPN: &str = "zirk_network_tls_connect_with_alpn";
    pub const NETWORK_TLS_UPGRADE_TCP: &str = "zirk_network_tls_upgrade_tcp";
    pub const NETWORK_TLS_CONNECT_WITH_CLIENT_IDENTITY: &str =
        "zirk_network_tls_connect_with_client_identity";
    pub const NETWORK_TLS_LISTEN: &str = "zirk_network_tls_listen";
    pub const NETWORK_TLS_LISTEN_WITH_CLIENT_AUTH: &str =
        "zirk_network_tls_listen_with_client_auth";
    pub const NETWORK_TLS_LISTEN_HTTP: &str = "zirk_network_tls_listen_http";
    pub const NETWORK_TLS_ACCEPT: &str = "zirk_network_tls_accept";
    pub const NETWORK_TLS_ACCEPT_UNTIL_SHUTDOWN: &str = "zirk_network_tls_accept_until_shutdown";
    pub const NETWORK_TLS_CLOSE_LISTENER: &str = "zirk_network_tls_close_listener";
    pub const NETWORK_TLS_LISTENER_IS_CLOSED: &str = "zirk_network_tls_listener_is_closed";
    pub const NETWORK_TLS_PEER_ADDRESS: &str = "zirk_network_tls_peer_address";
    pub const NETWORK_TLS_NEGOTIATED_ALPN: &str = "zirk_network_tls_negotiated_alpn";
    pub const NETWORK_TLS_PEER_CERTIFICATE_SHA256: &str =
        "zirk_network_tls_peer_certificate_sha256";
    pub const NETWORK_TLS_READ: &str = "zirk_network_tls_read";
    pub const NETWORK_TLS_WRITE_ALL: &str = "zirk_network_tls_write_all";
    pub const NETWORK_TLS_SHUTDOWN: &str = "zirk_network_tls_shutdown";
    pub const NETWORK_TLS_CLOSE: &str = "zirk_network_tls_close";
    pub const NETWORK_TLS_IS_CLOSED: &str = "zirk_network_tls_is_closed";
    pub const NETWORK_TCP_READ: &str = "zirk_network_tcp_read";
    pub const NETWORK_TCP_WRITE: &str = "zirk_network_tcp_write";
    pub const NETWORK_TCP_WRITE_ALL: &str = "zirk_network_tcp_write_all";
    pub const NETWORK_TCP_SHUTDOWN: &str = "zirk_network_tcp_shutdown";
    pub const NETWORK_TCP_CLOSE: &str = "zirk_network_tcp_close";
    pub const NETWORK_TCP_IS_CLOSED: &str = "zirk_network_tcp_is_closed";
    pub const NETWORK_TCP_PEER_ADDRESS: &str = "zirk_network_tcp_peer_address";
    pub const NETWORK_TCP_LISTEN: &str = "zirk_network_tcp_listen";
    pub const NETWORK_TCP_ACCEPT: &str = "zirk_network_tcp_accept";
    pub const NETWORK_TCP_ACCEPT_UNTIL_SHUTDOWN: &str = "zirk_network_tcp_accept_until_shutdown";
    pub const NETWORK_TCP_CLOSE_LISTENER: &str = "zirk_network_tcp_close_listener";
    pub const NETWORK_TCP_LISTENER_IS_CLOSED: &str = "zirk_network_tcp_listener_is_closed";
    pub const HTTP_GET: &str = "zirk_http_get";
    pub const HTTP_GET_CACHED: &str = "zirk_http_get_cached";
    pub const HTTP_REQUEST: &str = "zirk_http_request";
    pub const HTTP_CLIENT_CREATE: &str = "zirk_http_client_create";
    pub const HTTP_CLIENT_CREATE_WITH_OPTIONS: &str = "zirk_http_client_create_with_options";
    pub const HTTP_CLIENT_CREATE_WITH_PROTOCOL_OPTIONS: &str =
        "zirk_http_client_create_with_protocol_options";
    pub const HTTP_CLIENT_CREATE_WITH_TRUST_ROOTS: &str =
        "zirk_http_client_create_with_trust_roots";
    pub const HTTP_CLIENT_CREATE_WITH_PROXY: &str = "zirk_http_client_create_with_proxy";
    pub const HTTP_CLIENT_CREATE_WITH_PROXY_AND_TRUST_ROOTS: &str =
        "zirk_http_client_create_with_proxy_and_trust_roots";
    pub const HTTP_CLIENT_ENABLE_COOKIES: &str = "zirk_http_client_enable_cookies";
    pub const HTTP_CLIENT_CLEAR_COOKIES: &str = "zirk_http_client_clear_cookies";
    pub const HTTP_CLIENT_REQUEST: &str = "zirk_http_client_request";
    pub const HTTP_CLIENT_REQUEST_STREAM: &str = "zirk_http_client_request_stream";
    pub const HTTP_CLIENT_CLOSE: &str = "zirk_http_client_close";
    pub const HTTP_WEBSOCKET_CONNECT: &str = "zirk_http_websocket_connect";
    pub const HTTP_WEBSOCKET_CONNECT_WITH_TRUST_ROOTS: &str =
        "zirk_http_websocket_connect_with_trust_roots";
    pub const HTTP_WEBSOCKET_SEND: &str = "zirk_http_websocket_send";
    pub const HTTP_WEBSOCKET_RECEIVE: &str = "zirk_http_websocket_receive";
    pub const HTTP_WEBSOCKET_CLOSE: &str = "zirk_http_websocket_close";
    pub const HTTP_WEBSOCKET_CLOSE_WITH: &str = "zirk_http_websocket_close_with";
    pub const HTTP_REQUEST_CACHED: &str = "zirk_http_request_cached";
    pub const HTTP_REQUEST_SECRET: &str = "zirk_http_request_secret";
    pub const HTTP_HEAD: &str = "zirk_http_head";
    pub const HTTP_QUERY: &str = "zirk_http_query";
    pub const HTTP_QUERY_WITH_CONTENT_TYPE: &str = "zirk_http_query_with_content_type";
    pub const HTTP_POST: &str = "zirk_http_post";
    pub const HTTP_POST_JSON: &str = "zirk_http_post_json";
    pub const HTTP_POST_BYTES: &str = "zirk_http_post_bytes";
    pub const HTTP_GET_STREAM: &str = "zirk_http_get_stream";
    pub const HTTP_RESPONSE_TO_STREAM: &str = "zirk_http_response_to_stream";
    pub const HTTP_REQUEST_STREAM: &str = "zirk_http_request_stream";
    pub const HTTP_SERVER_PRODUCER_REGISTER: &str = "zirk_http_server_producer_register";
    pub const HTTP_STREAM_READ_CHUNK: &str = "zirk_http_stream_read_chunk";
    pub const HTTP_STREAM_READ_ALL: &str = "zirk_http_stream_read_all";
    pub const HTTP_STREAM_SSE_NEXT: &str = "zirk_http_stream_sse_next";
    pub const HTTP_STREAM_SSE_HEARTBEAT_COUNT: &str = "zirk_http_stream_sse_heartbeat_count";
    pub const HTTP_STREAM_CLOSE: &str = "zirk_http_stream_close";
    pub const HTTP_URL_NORMALIZE: &str = "zirk_http_url_normalize";
    pub const HTTP_URL_RESOLVE: &str = "zirk_http_url_resolve";
    pub const HTTP_RESPONSE_TEXT: &str = "zirk_http_response_text";
    pub const HTTP_RESPONSE_HEADER_VALUES: &str = "zirk_http_response_header_values";
    pub const HTTP_ROUTE_MATCH: &str = "zirk_http_route_match";
    pub const HTTP_FORM_PARSE: &str = "zirk_http_form_parse";
    pub const HTTP_MULTIPART_PARSE: &str = "zirk_http_multipart_parse";
    pub const HTTP_UPLOAD_READ: &str = "zirk_http_upload_read";
    pub const HTTP_UPLOAD_CLOSE: &str = "zirk_http_upload_close";
    pub const HTTP_SERVER_SERVE_ONCE: &str = "zirk_http_server_serve_once";
    pub const HTTP_SERVER_SERVE_HTTP3: &str = "zirk_http_server_serve_http3";
    pub const HTTP_SERVER_CONTROL_CREATE: &str = "zirk_http_server_control_create";
    pub const HTTP_SERVER_CONTROL_REQUEST_SHUTDOWN: &str =
        "zirk_http_server_control_request_shutdown";
    pub const HTTP_SERVER_WEBSOCKET_SERVE_ONCE: &str = "zirk_http_server_websocket_serve_once";
    pub const HTTP_SERVER_WEBSOCKET_SERVE_HTTP3_ONCE: &str =
        "zirk_http_server_websocket_serve_http3_once";
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
    /// Reports a corrupt compiler-generated union tag and terminates.
    pub const INVALID_UNION_TAG: &str = "zirk_rt_invalid_union_tag";
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
    /// Adds tag-aware union payloads to the current shadow-stack frame.
    pub const PUSH_UNION_ROOTS: &str = "zirk_rt_push_union_roots";
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
    pub const CANCEL_SHIELD_ENTER: &str = "zirk_rt_cancel_shield_enter";
    pub const CANCEL_SHIELD_EXIT: &str = "zirk_rt_cancel_shield_exit";
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
    /// `zirk_rt_array_new_with_layout(capacity, elem_size, elem_align, offsets, count)`.
    pub const ARRAY_NEW_WITH_LAYOUT: &str = "zirk_rt_array_new_with_layout";
    /// `zirk_rt_list_new(elem_size, elem_align, is_ref) -> *mut c_void`.
    pub const LIST_NEW: &str = "zirk_rt_list_new";
    /// `zirk_rt_list_new_with_layout(elem_size, elem_align, offsets, count) -> *mut c_void`.
    pub const LIST_NEW_WITH_LAYOUT: &str = "zirk_rt_list_new_with_layout";
    /// `zirk_rt_array_clone_with_layout(array, elem_size, elem_align, offsets, count)`.
    pub const ARRAY_CLONE_WITH_LAYOUT: &str = "zirk_rt_array_clone_with_layout";
    /// `zirk_rt_array_slice_with_layout(array, start, end, step, elem_size, elem_align, offsets, count)`.
    pub const ARRAY_SLICE_WITH_LAYOUT: &str = "zirk_rt_array_slice_with_layout";
    /// `zirk_rt_map_new_with_layout(key_offsets, key_count, value_offsets,
    /// value_count, string_content_keys) -> *mut c_void`.
    pub const MAP_NEW_WITH_LAYOUT: &str = "zirk_rt_map_new_with_layout";
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
    /// Allocates a managed dynamic object with stable reference identity.
    pub const OBJECT_NEW: &str = "zirk_rt_object_new";
    /// Returns one dynamic object's own property count.
    pub const OBJECT_LENGTH: &str = "zirk_rt_object_length";
    /// Returns a key at its deterministic insertion-order index.
    pub const OBJECT_KEY_AT: &str = "zirk_rt_object_key_at";
    /// Sets a tagged property and reports a sealed-key-set violation.
    pub const OBJECT_SET: &str = "zirk_rt_object_set";
    /// Reads a tagged property into caller-provided tag/value slots.
    pub const OBJECT_GET: &str = "zirk_rt_object_get";
    pub const OBJECT_GET_BYTES: &str = "zirk_rt_object_get_bytes";
    pub const IS_OPEN_OBJECT: &str = "zirk_rt_is_open_object";
    /// Removes a property unless the object has been sealed.
    pub const OBJECT_REMOVE: &str = "zirk_rt_object_remove";
    /// Seals an object's current property-key set.
    pub const OBJECT_SEAL: &str = "zirk_rt_object_seal";
    /// Checks contract property tags and seals the object on success.
    pub const OBJECT_CONFORM: &str = "zirk_rt_object_conform";
    /// `zirk_rt_dynamic_object_unbox(value, object_out) -> i32`.
    pub const DYNAMIC_OBJECT_UNBOX: &str = "zirk_rt_dynamic_object_unbox";
    /// `zirk_rt_object_spread(object) -> *mut c_void`.
    pub const OBJECT_SPREAD: &str = "zirk_rt_object_spread";
    /// `zirk_rt_object_rest(object, excluded_keys, count) -> *mut c_void`.
    pub const OBJECT_REST: &str = "zirk_rt_object_rest";
    /// `zirk_rt_object_merge(destination, source) -> i32`.
    pub const OBJECT_MERGE: &str = "zirk_rt_object_merge";
    /// Copies a typed Zirk value into a managed, collector-aware Dynamic box.
    pub const DYNAMIC_VALUE_NEW: &str = "zirk_rt_dynamic_value_new";
    /// Reads the runtime type tag from a Dynamic box.
    pub const DYNAMIC_VALUE_TAG: &str = "zirk_rt_dynamic_value_tag";
    /// Copies a Dynamic payload to a typed destination after tag validation.
    pub const DYNAMIC_VALUE_COPY_TO: &str = "zirk_rt_dynamic_value_copy_to";
    pub const JSON_STRINGIFY_DYNAMIC: &str = "zirk_rt_json_stringify_dynamic";
    pub const JSON_PARSE_DYNAMIC: &str = "zirk_rt_json_parse_dynamic";
    /// `zirk_rt_set_new_with_layout(element_offsets, count,
    /// string_content_elements) -> *mut c_void`.
    pub const SET_NEW_WITH_LAYOUT: &str = "zirk_rt_set_new_with_layout";
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
    pub gc_mutex_alloc: FunctionValue<'ctx>,
    pub gc_mutex_acquire: FunctionValue<'ctx>,
    pub gc_mutex_release: FunctionValue<'ctx>,
    pub global_root_add: FunctionValue<'ctx>,
    pub global_watch_reset: FunctionValue<'ctx>,
    pub global_watch_subscribe: FunctionValue<'ctx>,
    pub global_watch_publish: FunctionValue<'ctx>,
    pub global_watch_next: FunctionValue<'ctx>,
    pub global_watch_close: FunctionValue<'ctx>,
    pub gc_mutex_with: FunctionValue<'ctx>,
    pub atomic_managed_new: FunctionValue<'ctx>,
    pub atomic_managed_load: FunctionValue<'ctx>,
    pub atomic_managed_load_ordered: FunctionValue<'ctx>,
    pub atomic_managed_store: FunctionValue<'ctx>,
    pub atomic_managed_store_ordered: FunctionValue<'ctx>,
    pub atomic_managed_exchange: FunctionValue<'ctx>,
    pub atomic_managed_exchange_ordered: FunctionValue<'ctx>,
    pub atomic_managed_compare_exchange: FunctionValue<'ctx>,
    pub atomic_managed_fetch_add: FunctionValue<'ctx>,
    pub atomic_managed_fetch_add_ordered: FunctionValue<'ctx>,
    pub atomic_managed_fetch_sub: FunctionValue<'ctx>,
    pub atomic_managed_fetch_sub_ordered: FunctionValue<'ctx>,
    pub init: FunctionValue<'ctx>,
    pub run_main: FunctionValue<'ctx>,
    pub run_main_cli: FunctionValue<'ctx>,
    pub app_set_shutdown_grace: FunctionValue<'ctx>,
    pub app_set_background_error: FunctionValue<'ctx>,
    pub background_handler_failed: FunctionValue<'ctx>,
    pub service_register: FunctionValue<'ctx>,
    pub service_ready: FunctionValue<'ctx>,
    pub service_failed_current: FunctionValue<'ctx>,
    pub spawn: FunctionValue<'ctx>,
    pub background_spawn: FunctionValue<'ctx>,
    pub job_wait: FunctionValue<'ctx>,
    pub thread_run: FunctionValue<'ctx>,
    pub job_wait_any: FunctionValue<'ctx>,
    pub job_wait_any_with_timer: FunctionValue<'ctx>,
    pub job_outcome_wait_take: FunctionValue<'ctx>,
    pub job_outcome_take: FunctionValue<'ctx>,
    pub job_done: FunctionValue<'ctx>,
    pub parallel_for: FunctionValue<'ctx>,
    pub safepoint_poll: FunctionValue<'ctx>,
    pub scope_enter: FunctionValue<'ctx>,
    pub scope_enter_collect_all: FunctionValue<'ctx>,
    pub scope_exit: FunctionValue<'ctx>,
    pub scope_exit_collect_all: FunctionValue<'ctx>,
    pub scope_outcome_wait_take: FunctionValue<'ctx>,
    pub scope_cancel: FunctionValue<'ctx>,
    pub branch_register: FunctionValue<'ctx>,
    pub cancel: FunctionValue<'ctx>,
    pub cancel_with_cleanup_failure: FunctionValue<'ctx>,
    pub timer_after: FunctionValue<'ctx>,
    pub timer_every: FunctionValue<'ctx>,
    pub sleep: FunctionValue<'ctx>,
    pub chan_new: FunctionValue<'ctx>,
    pub chan_try_send: FunctionValue<'ctx>,
    pub chan_try_recv: FunctionValue<'ctx>,
    pub chan_send: FunctionValue<'ctx>,
    pub chan_recv: FunctionValue<'ctx>,
    pub chan_close: FunctionValue<'ctx>,
    pub chan_len: FunctionValue<'ctx>,
    pub chan_is_closed: FunctionValue<'ctx>,
    pub chan_capacity: FunctionValue<'ctx>,
    pub chan_drain: FunctionValue<'ctx>,
    pub list_new_with_layout: FunctionValue<'ctx>,
    pub array_new_with_layout: FunctionValue<'ctx>,
    pub array_clone_with_layout: FunctionValue<'ctx>,
    pub array_slice_with_layout: FunctionValue<'ctx>,
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
    pub float128_to_f64: FunctionValue<'ctx>,
    pub f64_to_float128: FunctionValue<'ctx>,
    pub float_format: FunctionValue<'ctx>,
    pub str_from_bool: FunctionValue<'ctx>,
    pub str_grapheme_offset: FunctionValue<'ctx>,
    pub str_grapheme_len_at: FunctionValue<'ctx>,
    pub str_grapheme_slice: FunctionValue<'ctx>,
    pub str_eq: FunctionValue<'ctx>,
    pub io_println: FunctionValue<'ctx>,
    pub io_print: FunctionValue<'ctx>,
    pub io_eprintln: FunctionValue<'ctx>,
    pub io_eprint: FunctionValue<'ctx>,
    pub io_stdout_flush: FunctionValue<'ctx>,
    pub io_stderr_flush: FunctionValue<'ctx>,
    pub io_stdin_read_line: FunctionValue<'ctx>,
    pub io_stdin_read_char: FunctionValue<'ctx>,
    pub io_stdin_read_bytes: FunctionValue<'ctx>,
    pub io_stdin_read_exact: FunctionValue<'ctx>,
    pub environment_get: FunctionValue<'ctx>,
    pub environment_get_secret: FunctionValue<'ctx>,
    pub permission_requester_set: FunctionValue<'ctx>,
    pub permission_requester_clear: FunctionValue<'ctx>,
    pub environment_contains: FunctionValue<'ctx>,
    pub environment_list_names: FunctionValue<'ctx>,
    pub filesystem_call: FunctionValue<'ctx>,
    pub network_tcp_connect: FunctionValue<'ctx>,
    pub network_udp_connect: FunctionValue<'ctx>,
    pub network_udp_bind: FunctionValue<'ctx>,
    pub network_udp_send: FunctionValue<'ctx>,
    pub network_udp_send_to: FunctionValue<'ctx>,
    pub network_udp_send_broadcast: FunctionValue<'ctx>,
    pub network_udp_send_multicast: FunctionValue<'ctx>,
    pub network_udp_join_multicast: FunctionValue<'ctx>,
    pub network_udp_leave_multicast: FunctionValue<'ctx>,
    pub network_udp_receive: FunctionValue<'ctx>,
    pub network_udp_local_address: FunctionValue<'ctx>,
    pub network_udp_close: FunctionValue<'ctx>,
    pub network_udp_is_closed: FunctionValue<'ctx>,
    pub network_local_connect: FunctionValue<'ctx>,
    pub network_local_bind: FunctionValue<'ctx>,
    pub network_local_accept: FunctionValue<'ctx>,
    pub network_local_read: FunctionValue<'ctx>,
    pub network_local_write: FunctionValue<'ctx>,
    pub network_local_write_all: FunctionValue<'ctx>,
    pub network_local_shutdown: FunctionValue<'ctx>,
    pub network_local_close_stream: FunctionValue<'ctx>,
    pub network_local_close_listener: FunctionValue<'ctx>,
    pub network_local_stream_is_closed: FunctionValue<'ctx>,
    pub network_local_listener_is_closed: FunctionValue<'ctx>,
    pub network_dns_resolve: FunctionValue<'ctx>,
    pub network_interfaces_snapshot: FunctionValue<'ctx>,
    pub network_ip_address_parse: FunctionValue<'ctx>,
    pub network_tcp_origin_normalize: FunctionValue<'ctx>,
    pub network_tls_connect: FunctionValue<'ctx>,
    pub network_tls_connect_with_roots: FunctionValue<'ctx>,
    pub network_tls_connect_with_alpn: FunctionValue<'ctx>,
    pub network_tls_upgrade_tcp: FunctionValue<'ctx>,
    pub network_tls_connect_with_client_identity: FunctionValue<'ctx>,
    pub network_tls_listen: FunctionValue<'ctx>,
    pub network_tls_listen_with_client_auth: FunctionValue<'ctx>,
    pub network_tls_listen_http: FunctionValue<'ctx>,
    pub network_tls_accept: FunctionValue<'ctx>,
    pub network_tls_accept_until_shutdown: FunctionValue<'ctx>,
    pub network_tls_close_listener: FunctionValue<'ctx>,
    pub network_tls_listener_is_closed: FunctionValue<'ctx>,
    pub network_tls_peer_address: FunctionValue<'ctx>,
    pub network_tls_negotiated_alpn: FunctionValue<'ctx>,
    pub network_tls_peer_certificate_sha256: FunctionValue<'ctx>,
    pub network_tls_read: FunctionValue<'ctx>,
    pub network_tls_write_all: FunctionValue<'ctx>,
    pub network_tls_shutdown: FunctionValue<'ctx>,
    pub network_tls_close: FunctionValue<'ctx>,
    pub network_tls_is_closed: FunctionValue<'ctx>,
    pub network_tcp_read: FunctionValue<'ctx>,
    pub network_tcp_write: FunctionValue<'ctx>,
    pub network_tcp_write_all: FunctionValue<'ctx>,
    pub network_tcp_shutdown: FunctionValue<'ctx>,
    pub network_tcp_close: FunctionValue<'ctx>,
    pub network_tcp_is_closed: FunctionValue<'ctx>,
    pub network_tcp_peer_address: FunctionValue<'ctx>,
    pub network_tcp_listen: FunctionValue<'ctx>,
    pub network_tcp_accept: FunctionValue<'ctx>,
    pub network_tcp_accept_until_shutdown: FunctionValue<'ctx>,
    pub network_tcp_close_listener: FunctionValue<'ctx>,
    pub network_tcp_listener_is_closed: FunctionValue<'ctx>,
    pub http_get: FunctionValue<'ctx>,
    pub http_get_cached: FunctionValue<'ctx>,
    pub http_request: FunctionValue<'ctx>,
    pub http_client_create: FunctionValue<'ctx>,
    pub http_client_create_with_options: FunctionValue<'ctx>,
    pub http_client_create_with_protocol_options: FunctionValue<'ctx>,
    pub http_client_create_with_trust_roots: FunctionValue<'ctx>,
    pub http_client_create_with_proxy: FunctionValue<'ctx>,
    pub http_client_create_with_proxy_and_trust_roots: FunctionValue<'ctx>,
    pub http_client_enable_cookies: FunctionValue<'ctx>,
    pub http_client_clear_cookies: FunctionValue<'ctx>,
    pub http_client_request: FunctionValue<'ctx>,
    pub http_client_request_stream: FunctionValue<'ctx>,
    pub http_client_close: FunctionValue<'ctx>,
    pub http_websocket_connect: FunctionValue<'ctx>,
    pub http_websocket_connect_with_trust_roots: FunctionValue<'ctx>,
    pub http_websocket_send: FunctionValue<'ctx>,
    pub http_websocket_receive: FunctionValue<'ctx>,
    pub http_websocket_close: FunctionValue<'ctx>,
    pub http_websocket_close_with: FunctionValue<'ctx>,
    pub http_request_cached: FunctionValue<'ctx>,
    pub http_request_secret: FunctionValue<'ctx>,
    pub http_head: FunctionValue<'ctx>,
    pub http_query: FunctionValue<'ctx>,
    pub http_query_with_content_type: FunctionValue<'ctx>,
    pub http_post: FunctionValue<'ctx>,
    pub http_post_json: FunctionValue<'ctx>,
    pub http_post_bytes: FunctionValue<'ctx>,
    pub http_get_stream: FunctionValue<'ctx>,
    pub http_response_to_stream: FunctionValue<'ctx>,
    pub http_request_stream: FunctionValue<'ctx>,
    pub http_server_producer_register: FunctionValue<'ctx>,
    pub http_stream_read_chunk: FunctionValue<'ctx>,
    pub http_stream_read_all: FunctionValue<'ctx>,
    pub http_stream_sse_next: FunctionValue<'ctx>,
    pub http_stream_sse_heartbeat_count: FunctionValue<'ctx>,
    pub http_stream_close: FunctionValue<'ctx>,
    pub http_url_normalize: FunctionValue<'ctx>,
    pub http_url_resolve: FunctionValue<'ctx>,
    pub http_response_text: FunctionValue<'ctx>,
    pub http_response_header_values: FunctionValue<'ctx>,
    pub http_route_match: FunctionValue<'ctx>,
    pub http_form_parse: FunctionValue<'ctx>,
    pub http_multipart_parse: FunctionValue<'ctx>,
    pub http_upload_read: FunctionValue<'ctx>,
    pub http_upload_close: FunctionValue<'ctx>,
    pub http_server_serve_once: FunctionValue<'ctx>,
    pub http_server_serve_http3: FunctionValue<'ctx>,
    pub http_server_control_create: FunctionValue<'ctx>,
    pub http_server_control_request_shutdown: FunctionValue<'ctx>,
    pub http_server_websocket_serve_once: FunctionValue<'ctx>,
    pub http_server_websocket_serve_http3_once: FunctionValue<'ctx>,
    pub alloc: FunctionValue<'ctx>,
    pub contract_table: FunctionValue<'ctx>,
    pub missing_contract: FunctionValue<'ctx>,
    pub str_concat: FunctionValue<'ctx>,
    pub str_repeat: FunctionValue<'ctx>,
    pub check_cast: FunctionValue<'ctx>,
    pub invalid_cast: FunctionValue<'ctx>,
    pub invalid_union_tag: FunctionValue<'ctx>,
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
    pub push_union_roots: FunctionValue<'ctx>,
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
    pub cancel_shield_enter: FunctionValue<'ctx>,
    pub cancel_shield_exit: FunctionValue<'ctx>,
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
    pub object_new: FunctionValue<'ctx>,
    pub object_length: FunctionValue<'ctx>,
    pub object_key_at: FunctionValue<'ctx>,
    pub object_set: FunctionValue<'ctx>,
    pub object_get: FunctionValue<'ctx>,
    pub object_get_bytes: FunctionValue<'ctx>,
    pub is_open_object: FunctionValue<'ctx>,
    pub object_remove: FunctionValue<'ctx>,
    pub object_seal: FunctionValue<'ctx>,
    pub object_conform: FunctionValue<'ctx>,
    pub dynamic_object_unbox: FunctionValue<'ctx>,
    pub object_spread: FunctionValue<'ctx>,
    pub object_rest: FunctionValue<'ctx>,
    pub object_merge: FunctionValue<'ctx>,
    pub dynamic_value_new: FunctionValue<'ctx>,
    pub dynamic_value_tag: FunctionValue<'ctx>,
    pub dynamic_value_copy_to: FunctionValue<'ctx>,
    pub json_stringify_dynamic: FunctionValue<'ctx>,
    pub json_parse_dynamic: FunctionValue<'ctx>,
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
    let i32 = context.i32_type();
    let ptr = context.ptr_type(AddressSpace::default());

    let external = Some(Linkage::External);

    // Mutex payload sizes/alignments/reference counts use native `usize`.
    // Source Mutex emission is still gated until borrowed projections and
    // all-exit cleanup are executable.
    let gc_mutex_alloc = module.add_function(
        symbols::GC_MUTEX_ALLOC,
        ptr.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let gc_mutex_acquire = module.add_function(
        symbols::GC_MUTEX_ACQUIRE,
        i32.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let gc_mutex_release = module.add_function(
        symbols::GC_MUTEX_RELEASE,
        i32.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let global_root_add = module.add_function(
        symbols::GLOBAL_ROOT_ADD,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let global_watch_reset = module.add_function(
        symbols::GLOBAL_WATCH_RESET,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let global_watch_subscribe = module.add_function(
        symbols::GLOBAL_WATCH_SUBSCRIBE,
        i32.fn_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let global_watch_publish = module.add_function(
        symbols::GLOBAL_WATCH_PUBLISH,
        i64.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
    let global_watch_next = module.add_function(
        symbols::GLOBAL_WATCH_NEXT,
        i32.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let global_watch_close = module.add_function(
        symbols::GLOBAL_WATCH_CLOSE,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let gc_mutex_with = module.add_function(
        symbols::GC_MUTEX_WITH,
        i32.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );

    let atomic_managed_new = module.add_function(
        symbols::ATOMIC_MANAGED_NEW,
        i32.fn_type(&[i32.into(), i64.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_load = module.add_function(
        symbols::ATOMIC_MANAGED_LOAD,
        i32.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_load_ordered = module.add_function(
        symbols::ATOMIC_MANAGED_LOAD_ORDERED,
        i32.fn_type(&[ptr.into(), i32.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_store = module.add_function(
        symbols::ATOMIC_MANAGED_STORE,
        i32.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let atomic_managed_store_ordered = module.add_function(
        symbols::ATOMIC_MANAGED_STORE_ORDERED,
        i32.fn_type(&[ptr.into(), i64.into(), i32.into()], false),
        external,
    );
    let atomic_managed_exchange = module.add_function(
        symbols::ATOMIC_MANAGED_EXCHANGE,
        i32.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_exchange_ordered = module.add_function(
        symbols::ATOMIC_MANAGED_EXCHANGE_ORDERED,
        i32.fn_type(&[ptr.into(), i64.into(), i32.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_compare_exchange = module.add_function(
        symbols::ATOMIC_MANAGED_COMPARE_EXCHANGE,
        i32.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                i32.into(),
                i32.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let atomic_managed_fetch_add = module.add_function(
        symbols::ATOMIC_MANAGED_FETCH_ADD,
        i32.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_fetch_add_ordered = module.add_function(
        symbols::ATOMIC_MANAGED_FETCH_ADD_ORDERED,
        i32.fn_type(&[ptr.into(), i64.into(), i32.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_fetch_sub = module.add_function(
        symbols::ATOMIC_MANAGED_FETCH_SUB,
        i32.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
        external,
    );
    let atomic_managed_fetch_sub_ordered = module.add_function(
        symbols::ATOMIC_MANAGED_FETCH_SUB_ORDERED,
        i32.fn_type(&[ptr.into(), i64.into(), i32.into(), ptr.into()], false),
        external,
    );

    let init = module.add_function(symbols::INIT, void.fn_type(&[], false), external);
    let run_main = module.add_function(
        symbols::RUN_MAIN,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let run_main_cli = module.add_function(
        symbols::RUN_MAIN_CLI,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let app_set_shutdown_grace = module.add_function(
        symbols::APP_SET_SHUTDOWN_GRACE,
        i32.fn_type(&[i64.into()], false),
        external,
    );
    let app_set_background_error = module.add_function(
        symbols::APP_SET_BACKGROUND_ERROR,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let background_handler_failed = module.add_function(
        symbols::BACKGROUND_HANDLER_FAILED,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let service_register = module.add_function(
        symbols::SERVICE_REGISTER,
        i64.fn_type(
            &[ptr.into(), ptr.into(), ptr.into(), i32.into(), ptr.into()],
            false,
        ),
        external,
    );
    let service_ready = module.add_function(
        symbols::SERVICE_READY,
        i32.fn_type(&[i64.into()], false),
        external,
    );
    let service_failed_current = module.add_function(
        symbols::SERVICE_FAILED_CURRENT,
        i32.fn_type(&[], false),
        external,
    );
    let spawn = module.add_function(
        symbols::SPAWN,
        i64.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let background_spawn = module.add_function(
        symbols::BACKGROUND_SPAWN,
        i32.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let job_wait = module.add_function(
        symbols::JOB_WAIT,
        i64.fn_type(&[i64.into()], false),
        external,
    );
    let thread_run = module.add_function(
        symbols::THREAD_RUN,
        context.i32_type().fn_type(
            &[ptr.into(), ptr.into(), ptr.into(), ptr.into(), ptr.into()],
            false,
        ),
        external,
    );
    let job_wait_any = module.add_function(
        symbols::JOB_WAIT_ANY,
        i64.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let job_wait_any_with_timer = module.add_function(
        symbols::JOB_WAIT_ANY_WITH_TIMER,
        i64.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let job_outcome_wait_take = module.add_function(
        symbols::JOB_OUTCOME_WAIT_TAKE,
        context
            .i32_type()
            .fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let job_outcome_take = module.add_function(
        symbols::JOB_OUTCOME_TAKE,
        context
            .i32_type()
            .fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
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
    let safepoint_poll =
        module.add_function(symbols::SAFEPOINT_POLL, void.fn_type(&[], false), external);
    let scope_enter = module.add_function(symbols::SCOPE_ENTER, i64.fn_type(&[], false), external);
    let scope_enter_collect_all = module.add_function(
        symbols::SCOPE_ENTER_COLLECT_ALL,
        i64.fn_type(&[], false),
        external,
    );
    let scope_exit = module.add_function(
        symbols::SCOPE_EXIT,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let scope_exit_collect_all = module.add_function(
        symbols::SCOPE_EXIT_COLLECT_ALL,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let scope_outcome_wait_take = module.add_function(
        symbols::SCOPE_OUTCOME_WAIT_TAKE,
        context
            .i32_type()
            .fn_type(&[i64.into(), ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let scope_cancel = module.add_function(
        symbols::SCOPE_CANCEL,
        void.fn_type(&[i64.into()], false),
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
    let cancel_with_cleanup_failure = module.add_function(
        symbols::CANCEL_WITH_CLEANUP_FAILURE,
        void.fn_type(&[i64.into()], false),
        external,
    );
    let timer_after = module.add_function(
        symbols::TIMER_AFTER,
        i64.fn_type(
            &[i64.into(), i64.into(), ptr.into(), ptr.into(), ptr.into()],
            false,
        ),
        external,
    );
    let timer_every = module.add_function(
        symbols::TIMER_EVERY,
        i64.fn_type(
            &[i64.into(), i64.into(), ptr.into(), ptr.into(), ptr.into()],
            false,
        ),
        external,
    );
    let sleep = module.add_function(symbols::SLEEP, i32.fn_type(&[i64.into()], false), external);
    // Channels carry values through caller-owned slots. The metadata supplied
    // to construction makes the runtime independent of every concrete `T`.
    let chan_new = module.add_function(
        symbols::CHAN_NEW,
        ptr.fn_type(
            &[
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        ),
        external,
    );
    let chan_try_send = module.add_function(
        symbols::CHAN_TRY_SEND,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let chan_try_recv = module.add_function(
        symbols::CHAN_TRY_RECV,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let chan_send = module.add_function(
        symbols::CHAN_SEND,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let chan_recv = module.add_function(
        symbols::CHAN_RECV,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let chan_close = module.add_function(
        symbols::CHAN_CLOSE,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let chan_len = module.add_function(
        symbols::CHAN_LEN,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let chan_is_closed = module.add_function(
        symbols::CHAN_IS_CLOSED,
        context.bool_type().fn_type(&[ptr.into()], false),
        external,
    );
    let chan_capacity = module.add_function(
        symbols::CHAN_CAPACITY,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let chan_drain = module.add_function(
        symbols::CHAN_DRAIN,
        void.fn_type(&[ptr.into(), ptr.into(), i64.into(), i64.into()], false),
        external,
    );
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
    let float128_to_f64 = module.add_function(
        symbols::FLOAT128_TO_F64,
        context.f64_type().fn_type(&[ptr.into()], false),
        external,
    );
    let f64_to_float128 = module.add_function(
        symbols::F64_TO_FLOAT128,
        context
            .void_type()
            .fn_type(&[ptr.into(), context.f64_type().into()], false),
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

    let io_print = module.add_function(
        symbols::IO_PRINT,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let io_eprintln = module.add_function(
        symbols::IO_EPRINTLN,
        context.void_type().fn_type(&[ptr.into()], false),
        Some(Linkage::External),
    );
    let io_eprint = module.add_function(
        symbols::IO_EPRINT,
        context.void_type().fn_type(&[ptr.into()], false),
        Some(Linkage::External),
    );
    let io_stdout_flush = module.add_function(
        symbols::IO_STDOUT_FLUSH,
        context.i32_type().fn_type(&[], false),
        external,
    );
    let io_stderr_flush = module.add_function(
        symbols::IO_STDERR_FLUSH,
        context.i32_type().fn_type(&[], false),
        external,
    );
    // Every `stdin.read_*` operation writes its outcome through a
    // `zirk_io::StdinReadOut` out-parameter (`{ i32 kind, i32 error, ptr
    // payload }`) rather than returning a value directly, so codegen can
    // read `kind`/`error`/`payload` back with ordinary loads
    // (`add-stdin-stdout-standard-library`).
    let io_stdin_read_line = module.add_function(
        symbols::IO_STDIN_READ_LINE,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let io_stdin_read_char = module.add_function(
        symbols::IO_STDIN_READ_CHAR,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let io_stdin_read_bytes = module.add_function(
        symbols::IO_STDIN_READ_BYTES,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let io_stdin_read_exact = module.add_function(
        symbols::IO_STDIN_READ_EXACT,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let environment_get = module.add_function(
        symbols::ENVIRONMENT_GET,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let environment_get_secret = module.add_function(
        symbols::ENVIRONMENT_GET_SECRET,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let permission_requester_set = module.add_function(
        symbols::PERMISSION_REQUESTER_SET,
        void.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let permission_requester_clear = module.add_function(
        symbols::PERMISSION_REQUESTER_CLEAR,
        void.fn_type(&[], false),
        external,
    );
    let environment_contains = module.add_function(
        symbols::ENVIRONMENT_CONTAINS,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let environment_list_names = module.add_function(
        symbols::ENVIRONMENT_LIST_NAMES,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let filesystem_call = module.add_function(
        symbols::FILESYSTEM_CALL,
        void.fn_type(&[i32.into(), ptr.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_connect = module.add_function(
        symbols::NETWORK_TCP_CONNECT,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_connect = module.add_function(
        symbols::NETWORK_UDP_CONNECT,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_bind = module.add_function(
        symbols::NETWORK_UDP_BIND,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_send = module.add_function(
        symbols::NETWORK_UDP_SEND,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_send_to = module.add_function(
        symbols::NETWORK_UDP_SEND_TO,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_send_broadcast = module.add_function(
        symbols::NETWORK_UDP_SEND_BROADCAST,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_send_multicast = module.add_function(
        symbols::NETWORK_UDP_SEND_MULTICAST,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_join_multicast = module.add_function(
        symbols::NETWORK_UDP_JOIN_MULTICAST,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_leave_multicast = module.add_function(
        symbols::NETWORK_UDP_LEAVE_MULTICAST,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_udp_receive = module.add_function(
        symbols::NETWORK_UDP_RECEIVE,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_udp_local_address = module.add_function(
        symbols::NETWORK_UDP_LOCAL_ADDRESS,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_udp_close = module.add_function(
        symbols::NETWORK_UDP_CLOSE,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_udp_is_closed = module.add_function(
        symbols::NETWORK_UDP_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let network_local_connect = module.add_function(
        symbols::NETWORK_LOCAL_CONNECT,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_local_bind = module.add_function(
        symbols::NETWORK_LOCAL_BIND,
        void.fn_type(&[ptr.into(), context.bool_type().into(), ptr.into()], false),
        external,
    );
    let network_local_accept = module.add_function(
        symbols::NETWORK_LOCAL_ACCEPT,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_local_read = module.add_function(
        symbols::NETWORK_LOCAL_READ,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_local_write = module.add_function(
        symbols::NETWORK_LOCAL_WRITE,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_local_write_all = module.add_function(
        symbols::NETWORK_LOCAL_WRITE_ALL,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_local_shutdown = module.add_function(
        symbols::NETWORK_LOCAL_SHUTDOWN,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_local_close_stream = module.add_function(
        symbols::NETWORK_LOCAL_CLOSE_STREAM,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_local_close_listener = module.add_function(
        symbols::NETWORK_LOCAL_CLOSE_LISTENER,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_local_stream_is_closed = module.add_function(
        symbols::NETWORK_LOCAL_STREAM_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let network_local_listener_is_closed = module.add_function(
        symbols::NETWORK_LOCAL_LISTENER_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let network_dns_resolve = module.add_function(
        symbols::NETWORK_DNS_RESOLVE,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_interfaces_snapshot = module.add_function(
        symbols::NETWORK_INTERFACES_SNAPSHOT,
        void.fn_type(&[ptr.into()], false),
        external,
    );
    let network_ip_address_parse = module.add_function(
        symbols::NETWORK_IP_ADDRESS_PARSE,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_tcp_origin_normalize = module.add_function(
        symbols::NETWORK_TCP_ORIGIN_NORMALIZE,
        void.fn_type(&[ptr.into(), context.i16_type().into(), ptr.into()], false),
        external,
    );
    let network_tls_connect = module.add_function(
        symbols::NETWORK_TLS_CONNECT,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_tls_connect_with_roots = module.add_function(
        symbols::NETWORK_TLS_CONNECT_WITH_ROOTS,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_connect_with_alpn = module.add_function(
        symbols::NETWORK_TLS_CONNECT_WITH_ALPN,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_upgrade_tcp = module.add_function(
        symbols::NETWORK_TLS_UPGRADE_TCP,
        void.fn_type(
            &[
                context.i64_type().into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_connect_with_client_identity = module.add_function(
        symbols::NETWORK_TLS_CONNECT_WITH_CLIENT_IDENTITY,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_listen = module.add_function(
        symbols::NETWORK_TLS_LISTEN,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_listen_http = module.add_function(
        symbols::NETWORK_TLS_LISTEN_HTTP,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_listen_with_client_auth = module.add_function(
        symbols::NETWORK_TLS_LISTEN_WITH_CLIENT_AUTH,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let network_tls_accept = module.add_function(
        symbols::NETWORK_TLS_ACCEPT,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_accept_until_shutdown = module.add_function(
        symbols::NETWORK_TLS_ACCEPT_UNTIL_SHUTDOWN,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_close_listener = module.add_function(
        symbols::NETWORK_TLS_CLOSE_LISTENER,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_listener_is_closed = module.add_function(
        symbols::NETWORK_TLS_LISTENER_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let network_tls_peer_address = module.add_function(
        symbols::NETWORK_TLS_PEER_ADDRESS,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_negotiated_alpn = module.add_function(
        symbols::NETWORK_TLS_NEGOTIATED_ALPN,
        void.fn_type(&[context.i64_type().into(), ptr.into()], false),
        external,
    );
    let network_tls_peer_certificate_sha256 = module.add_function(
        symbols::NETWORK_TLS_PEER_CERTIFICATE_SHA256,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_read = module.add_function(
        symbols::NETWORK_TLS_READ,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_write_all = module.add_function(
        symbols::NETWORK_TLS_WRITE_ALL,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_tls_shutdown = module.add_function(
        symbols::NETWORK_TLS_SHUTDOWN,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_close = module.add_function(
        symbols::NETWORK_TLS_CLOSE,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tls_is_closed = module.add_function(
        symbols::NETWORK_TLS_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let network_tcp_read = module.add_function(
        symbols::NETWORK_TCP_READ,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_write = module.add_function(
        symbols::NETWORK_TCP_WRITE,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_tcp_write_all = module.add_function(
        symbols::NETWORK_TCP_WRITE_ALL,
        void.fn_type(&[i64.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let network_tcp_shutdown = module.add_function(
        symbols::NETWORK_TCP_SHUTDOWN,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_close = module.add_function(
        symbols::NETWORK_TCP_CLOSE,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_is_closed = module.add_function(
        symbols::NETWORK_TCP_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let network_tcp_peer_address = module.add_function(
        symbols::NETWORK_TCP_PEER_ADDRESS,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_listen = module.add_function(
        symbols::NETWORK_TCP_LISTEN,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let network_tcp_accept = module.add_function(
        symbols::NETWORK_TCP_ACCEPT,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_accept_until_shutdown = module.add_function(
        symbols::NETWORK_TCP_ACCEPT_UNTIL_SHUTDOWN,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_close_listener = module.add_function(
        symbols::NETWORK_TCP_CLOSE_LISTENER,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let network_tcp_listener_is_closed = module.add_function(
        symbols::NETWORK_TCP_LISTENER_IS_CLOSED,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let http_get = module.add_function(
        symbols::HTTP_GET,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let http_get_cached = module.add_function(
        symbols::HTTP_GET_CACHED,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let http_request = module.add_function(
        symbols::HTTP_REQUEST,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_create = module.add_function(
        symbols::HTTP_CLIENT_CREATE,
        void.fn_type(&[context.i32_type().into(), ptr.into()], false),
        external,
    );
    let http_client_create_with_options = module.add_function(
        symbols::HTTP_CLIENT_CREATE_WITH_OPTIONS,
        void.fn_type(
            &[
                context.i32_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_create_with_protocol_options = module.add_function(
        symbols::HTTP_CLIENT_CREATE_WITH_PROTOCOL_OPTIONS,
        void.fn_type(
            &[
                context.i32_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i32_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_create_with_trust_roots = module.add_function(
        symbols::HTTP_CLIENT_CREATE_WITH_TRUST_ROOTS,
        void.fn_type(
            &[
                context.i32_type().into(),
                ptr.into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i32_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_create_with_proxy = module.add_function(
        symbols::HTTP_CLIENT_CREATE_WITH_PROXY,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                context.i32_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i32_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_create_with_proxy_and_trust_roots = module.add_function(
        symbols::HTTP_CLIENT_CREATE_WITH_PROXY_AND_TRUST_ROOTS,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                context.i32_type().into(),
                ptr.into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                context.i32_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_enable_cookies = module.add_function(
        symbols::HTTP_CLIENT_ENABLE_COOKIES,
        void.fn_type(&[context.i64_type().into(), ptr.into()], false),
        external,
    );
    let http_client_clear_cookies = module.add_function(
        symbols::HTTP_CLIENT_CLEAR_COOKIES,
        void.fn_type(&[context.i64_type().into(), ptr.into()], false),
        external,
    );
    let http_client_request = module.add_function(
        symbols::HTTP_CLIENT_REQUEST,
        void.fn_type(
            &[
                context.i64_type().into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_request_stream = module.add_function(
        symbols::HTTP_CLIENT_REQUEST_STREAM,
        void.fn_type(
            &[
                context.i64_type().into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.i32_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_client_close = module.add_function(
        symbols::HTTP_CLIENT_CLOSE,
        void.fn_type(&[context.i64_type().into(), ptr.into()], false),
        external,
    );
    let http_websocket_connect = module.add_function(
        symbols::HTTP_WEBSOCKET_CONNECT,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_websocket_connect_with_trust_roots = module.add_function(
        symbols::HTTP_WEBSOCKET_CONNECT_WITH_TRUST_ROOTS,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.bool_type().into(),
                context.bool_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_websocket_send = module.add_function(
        symbols::HTTP_WEBSOCKET_SEND,
        void.fn_type(&[context.i64_type().into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_websocket_receive = module.add_function(
        symbols::HTTP_WEBSOCKET_RECEIVE,
        void.fn_type(&[context.i64_type().into(), ptr.into()], false),
        external,
    );
    let http_websocket_close = module.add_function(
        symbols::HTTP_WEBSOCKET_CLOSE,
        void.fn_type(&[context.i64_type().into(), ptr.into()], false),
        external,
    );
    let http_websocket_close_with = module.add_function(
        symbols::HTTP_WEBSOCKET_CLOSE_WITH,
        void.fn_type(
            &[
                context.i64_type().into(),
                context.i16_type().into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_request_cached = module.add_function(
        symbols::HTTP_REQUEST_CACHED,
        void.fn_type(
            &[ptr.into(), ptr.into(), ptr.into(), ptr.into(), ptr.into()],
            false,
        ),
        external,
    );
    let http_request_secret = module.add_function(
        symbols::HTTP_REQUEST_SECRET,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_head = module.add_function(
        symbols::HTTP_HEAD,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let http_query = module.add_function(
        symbols::HTTP_QUERY,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_query_with_content_type = module.add_function(
        symbols::HTTP_QUERY_WITH_CONTENT_TYPE,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_post = module.add_function(
        symbols::HTTP_POST,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_post_json = module.add_function(
        symbols::HTTP_POST_JSON,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_post_bytes = module.add_function(
        symbols::HTTP_POST_BYTES,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_get_stream = module.add_function(
        symbols::HTTP_GET_STREAM,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_response_to_stream = module.add_function(
        symbols::HTTP_RESPONSE_TO_STREAM,
        void.fn_type(
            &[
                context.i32_type().into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_request_stream = module.add_function(
        symbols::HTTP_REQUEST_STREAM,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                context.i32_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_server_producer_register = module.add_function(
        symbols::HTTP_SERVER_PRODUCER_REGISTER,
        i64.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_stream_read_chunk = module.add_function(
        symbols::HTTP_STREAM_READ_CHUNK,
        void.fn_type(&[i64.into(), i64.into(), ptr.into()], false),
        external,
    );
    let http_stream_read_all = module.add_function(
        symbols::HTTP_STREAM_READ_ALL,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let http_stream_sse_next = module.add_function(
        symbols::HTTP_STREAM_SSE_NEXT,
        void.fn_type(&[i64.into(), ptr.into()], false),
        external,
    );
    let http_stream_sse_heartbeat_count = module.add_function(
        symbols::HTTP_STREAM_SSE_HEARTBEAT_COUNT,
        i64.fn_type(&[i64.into()], false),
        external,
    );
    let http_stream_close = module.add_function(
        symbols::HTTP_STREAM_CLOSE,
        context.bool_type().fn_type(&[i64.into()], false),
        external,
    );
    let http_url_normalize = module.add_function(
        symbols::HTTP_URL_NORMALIZE,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let http_url_resolve = module.add_function(
        symbols::HTTP_URL_RESOLVE,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_response_text = module.add_function(
        symbols::HTTP_RESPONSE_TEXT,
        void.fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let http_response_header_values = module.add_function(
        symbols::HTTP_RESPONSE_HEADER_VALUES,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_route_match = module.add_function(
        symbols::HTTP_ROUTE_MATCH,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_form_parse = module.add_function(
        symbols::HTTP_FORM_PARSE,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_multipart_parse = module.add_function(
        symbols::HTTP_MULTIPART_PARSE,
        void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let http_upload_read = module.add_function(
        symbols::HTTP_UPLOAD_READ,
        void.fn_type(
            &[
                context.i64_type().into(),
                context.i64_type().into(),
                context.i64_type().into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_upload_close = module.add_function(
        symbols::HTTP_UPLOAD_CLOSE,
        context
            .bool_type()
            .fn_type(&[context.i64_type().into()], false),
        external,
    );
    let http_server_control_create = module.add_function(
        symbols::HTTP_SERVER_CONTROL_CREATE,
        i64.fn_type(&[], false),
        external,
    );
    let http_server_control_request_shutdown = module.add_function(
        symbols::HTTP_SERVER_CONTROL_REQUEST_SHUTDOWN,
        i32.fn_type(&[i64.into()], false),
        external,
    );
    let http_server_serve_once = module.add_function(
        symbols::HTTP_SERVER_SERVE_ONCE,
        void.fn_type(
            &[
                i64.into(),
                i32.into(),
                i64.into(),
                i32.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_server_serve_http3 = module.add_function(
        symbols::HTTP_SERVER_SERVE_HTTP3,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_server_websocket_serve_once = module.add_function(
        symbols::HTTP_SERVER_WEBSOCKET_SERVE_ONCE,
        void.fn_type(
            &[
                i64.into(),
                i32.into(),
                ptr.into(),
                ptr.into(),
                i32.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let http_server_websocket_serve_http3_once = module.add_function(
        symbols::HTTP_SERVER_WEBSOCKET_SERVE_HTTP3_ONCE,
        void.fn_type(
            &[
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                i32.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        ),
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
    let cancel_shield_enter = module.add_function(
        symbols::CANCEL_SHIELD_ENTER,
        void.fn_type(&[], false),
        external,
    );
    let cancel_shield_exit = module.add_function(
        symbols::CANCEL_SHIELD_EXIT,
        void.fn_type(&[], false),
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
    // `Regex.find`'s extern declaration lives entirely in `emit.rs`'s
    // `declare_extern_fn` (through the `ir::ExternFn` list), not here: its
    // `Nullable<T>` return crosses by a leading out-pointer for ABI
    // reasons `declare_extern_fn`'s own doc covers, and every call site
    // goes through `self.functions`, never `self.runtime`.
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
    let array_new_with_layout = module.add_function(
        symbols::ARRAY_NEW_WITH_LAYOUT,
        ptr.fn_type(
            &[i64.into(), i64.into(), i64.into(), ptr.into(), i64.into()],
            false,
        ),
        external,
    );
    let list_new = module.add_function(
        symbols::LIST_NEW,
        ptr.fn_type(&[i64.into(), i64.into(), context.bool_type().into()], false),
        external,
    );
    let list_new_with_layout = module.add_function(
        symbols::LIST_NEW_WITH_LAYOUT,
        ptr.fn_type(&[i64.into(), i64.into(), ptr.into(), i64.into()], false),
        external,
    );
    let array_clone_with_layout = module.add_function(
        symbols::ARRAY_CLONE_WITH_LAYOUT,
        ptr.fn_type(
            &[ptr.into(), i64.into(), i64.into(), ptr.into(), i64.into()],
            false,
        ),
        external,
    );
    let array_slice_with_layout = module.add_function(
        symbols::ARRAY_SLICE_WITH_LAYOUT,
        ptr.fn_type(
            &[
                ptr.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        ),
        external,
    );
    let map_new = module.add_function(
        symbols::MAP_NEW_WITH_LAYOUT,
        ptr.fn_type(
            &[
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                context.bool_type().into(),
            ],
            false,
        ),
        external,
    );
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
    let object_new = module.add_function(symbols::OBJECT_NEW, ptr.fn_type(&[], false), external);
    let object_length = module.add_function(
        symbols::OBJECT_LENGTH,
        i64.fn_type(&[ptr.into()], false),
        external,
    );
    let object_key_at = module.add_function(
        symbols::OBJECT_KEY_AT,
        ptr.fn_type(&[ptr.into(), i64.into()], false),
        external,
    );
    let object_set = module.add_function(
        symbols::OBJECT_SET,
        context
            .i32_type()
            .fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let object_get = module.add_function(
        symbols::OBJECT_GET,
        context
            .i32_type()
            .fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
        external,
    );
    let object_get_bytes = module.add_function(
        symbols::OBJECT_GET_BYTES,
        context
            .i32_type()
            .fn_type(&[ptr.into(), ptr.into(), i64.into(), ptr.into()], false),
        external,
    );
    let is_open_object = module.add_function(
        symbols::IS_OPEN_OBJECT,
        context.i32_type().fn_type(&[ptr.into()], false),
        external,
    );
    let object_remove = module.add_function(
        symbols::OBJECT_REMOVE,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let object_seal = module.add_function(
        symbols::OBJECT_SEAL,
        context.i32_type().fn_type(&[ptr.into()], false),
        external,
    );
    let object_conform = module.add_function(
        symbols::OBJECT_CONFORM,
        context.i32_type().fn_type(
            &[
                ptr.into(),
                context.i32_type().into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
            ],
            false,
        ),
        external,
    );
    let dynamic_object_unbox = module.add_function(
        symbols::DYNAMIC_OBJECT_UNBOX,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let object_spread = module.add_function(
        symbols::OBJECT_SPREAD,
        ptr.fn_type(&[ptr.into()], false),
        external,
    );
    let object_rest = module.add_function(
        symbols::OBJECT_REST,
        ptr.fn_type(&[ptr.into(), ptr.into(), i64.into()], false),
        external,
    );
    let object_merge = module.add_function(
        symbols::OBJECT_MERGE,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let dynamic_value_new = module.add_function(
        symbols::DYNAMIC_VALUE_NEW,
        ptr.fn_type(
            &[
                context.i32_type().into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        ),
        external,
    );
    let dynamic_value_tag = module.add_function(
        symbols::DYNAMIC_VALUE_TAG,
        context.i32_type().fn_type(&[ptr.into()], false),
        external,
    );
    let dynamic_value_copy_to = module.add_function(
        symbols::DYNAMIC_VALUE_COPY_TO,
        context.i32_type().fn_type(
            &[
                ptr.into(),
                context.i32_type().into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        ),
        external,
    );
    let json_stringify_dynamic = module.add_function(
        symbols::JSON_STRINGIFY_DYNAMIC,
        context.i32_type().fn_type(&[ptr.into(), ptr.into()], false),
        external,
    );
    let json_parse_dynamic = module.add_function(
        symbols::JSON_PARSE_DYNAMIC,
        context
            .i32_type()
            .fn_type(&[ptr.into(), context.i32_type().into(), ptr.into()], false),
        external,
    );
    let set_new = module.add_function(
        symbols::SET_NEW_WITH_LAYOUT,
        ptr.fn_type(&[ptr.into(), i64.into(), context.bool_type().into()], false),
        external,
    );
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
    let invalid_union_tag = module.add_function(
        symbols::INVALID_UNION_TAG,
        void.fn_type(&[], false),
        external,
    );
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
    let push_union_roots = module.add_function(
        symbols::PUSH_UNION_ROOTS,
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
        invalid_union_tag,
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
        gc_mutex_alloc,
        gc_mutex_acquire,
        gc_mutex_release,
        global_root_add,
        global_watch_reset,
        global_watch_subscribe,
        global_watch_publish,
        global_watch_next,
        global_watch_close,
        gc_mutex_with,
        atomic_managed_new,
        atomic_managed_load,
        atomic_managed_load_ordered,
        atomic_managed_store,
        atomic_managed_store_ordered,
        atomic_managed_exchange,
        atomic_managed_exchange_ordered,
        atomic_managed_compare_exchange,
        atomic_managed_fetch_add,
        atomic_managed_fetch_add_ordered,
        atomic_managed_fetch_sub,
        atomic_managed_fetch_sub_ordered,
        init,
        run_main,
        run_main_cli,
        app_set_shutdown_grace,
        app_set_background_error,
        background_handler_failed,
        service_register,
        service_ready,
        service_failed_current,
        spawn,
        background_spawn,
        job_wait,
        thread_run,
        job_wait_any,
        job_wait_any_with_timer,
        job_outcome_wait_take,
        job_outcome_take,
        job_done,
        parallel_for,
        safepoint_poll,
        scope_enter,
        scope_enter_collect_all,
        scope_exit,
        scope_exit_collect_all,
        scope_outcome_wait_take,
        scope_cancel,
        branch_register,
        cancel,
        cancel_with_cleanup_failure,
        timer_after,
        timer_every,
        sleep,
        chan_new,
        chan_try_send,
        chan_try_recv,
        chan_send,
        chan_recv,
        chan_close,
        chan_len,
        chan_is_closed,
        chan_capacity,
        chan_drain,
        list_new_with_layout,
        array_new_with_layout,
        array_clone_with_layout,
        array_slice_with_layout,
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
        float128_to_f64,
        f64_to_float128,
        float_format,
        str_from_bool,
        str_grapheme_offset,
        str_grapheme_len_at,
        str_grapheme_slice,
        str_eq,
        io_println,
        io_print,
        io_eprintln,
        io_eprint,
        io_stdout_flush,
        io_stderr_flush,
        io_stdin_read_line,
        io_stdin_read_char,
        io_stdin_read_bytes,
        io_stdin_read_exact,
        environment_get,
        environment_get_secret,
        permission_requester_set,
        permission_requester_clear,
        environment_contains,
        environment_list_names,
        filesystem_call,
        network_tcp_connect,
        network_udp_connect,
        network_udp_bind,
        network_udp_send,
        network_udp_send_to,
        network_udp_send_broadcast,
        network_udp_send_multicast,
        network_udp_join_multicast,
        network_udp_leave_multicast,
        network_udp_receive,
        network_udp_local_address,
        network_udp_close,
        network_udp_is_closed,
        network_local_connect,
        network_local_bind,
        network_local_accept,
        network_local_read,
        network_local_write,
        network_local_write_all,
        network_local_shutdown,
        network_local_close_stream,
        network_local_close_listener,
        network_local_stream_is_closed,
        network_local_listener_is_closed,
        network_dns_resolve,
        network_interfaces_snapshot,
        network_ip_address_parse,
        network_tcp_origin_normalize,
        network_tls_connect,
        network_tls_connect_with_roots,
        network_tls_connect_with_alpn,
        network_tls_upgrade_tcp,
        network_tls_connect_with_client_identity,
        network_tls_listen,
        network_tls_listen_with_client_auth,
        network_tls_listen_http,
        network_tls_accept,
        network_tls_accept_until_shutdown,
        network_tls_close_listener,
        network_tls_listener_is_closed,
        network_tls_peer_address,
        network_tls_negotiated_alpn,
        network_tls_peer_certificate_sha256,
        network_tls_read,
        network_tls_write_all,
        network_tls_shutdown,
        network_tls_close,
        network_tls_is_closed,
        network_tcp_read,
        network_tcp_write,
        network_tcp_write_all,
        network_tcp_shutdown,
        network_tcp_close,
        network_tcp_is_closed,
        network_tcp_peer_address,
        network_tcp_listen,
        network_tcp_accept,
        network_tcp_accept_until_shutdown,
        network_tcp_close_listener,
        network_tcp_listener_is_closed,
        http_get,
        http_get_cached,
        http_request,
        http_client_create,
        http_client_create_with_options,
        http_client_create_with_protocol_options,
        http_client_create_with_trust_roots,
        http_client_create_with_proxy,
        http_client_create_with_proxy_and_trust_roots,
        http_client_enable_cookies,
        http_client_clear_cookies,
        http_client_request,
        http_client_request_stream,
        http_client_close,
        http_websocket_connect,
        http_websocket_connect_with_trust_roots,
        http_websocket_send,
        http_websocket_receive,
        http_websocket_close,
        http_websocket_close_with,
        http_request_cached,
        http_request_secret,
        http_head,
        http_query,
        http_query_with_content_type,
        http_post,
        http_post_json,
        http_post_bytes,
        http_get_stream,
        http_response_to_stream,
        http_request_stream,
        http_server_producer_register,
        http_stream_read_chunk,
        http_stream_read_all,
        http_stream_sse_next,
        http_stream_sse_heartbeat_count,
        http_stream_close,
        http_url_normalize,
        http_url_resolve,
        http_response_text,
        http_response_header_values,
        http_route_match,
        http_form_parse,
        http_multipart_parse,
        http_upload_read,
        http_upload_close,
        http_server_serve_once,
        http_server_serve_http3,
        http_server_control_create,
        http_server_control_request_shutdown,
        http_server_websocket_serve_once,
        http_server_websocket_serve_http3_once,
        alloc,
        contract_table,
        missing_contract,
        str_concat,
        str_repeat,
        check_cast,
        invalid_cast,
        invalid_union_tag,
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
        push_union_roots,
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
        cancel_shield_enter,
        cancel_shield_exit,
        resource_close_group,
        resource_transfer,
        pin_object,
        unpin_object,
        dependent_base,
        regex_from_pattern,
        regex_is_match,
        regex_replace,
        regex_to_string,
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
        object_new,
        object_length,
        object_key_at,
        object_set,
        object_get,
        object_get_bytes,
        is_open_object,
        object_remove,
        object_seal,
        object_conform,
        dynamic_object_unbox,
        object_spread,
        object_rest,
        object_merge,
        dynamic_value_new,
        dynamic_value_tag,
        dynamic_value_copy_to,
        json_stringify_dynamic,
        json_parse_dynamic,
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
