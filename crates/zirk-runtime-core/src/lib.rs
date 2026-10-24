//! # zirk-runtime-core
//!
//! **Responsibility:** platform-neutral Zirk runtime semantics shared by the
//! native runtime and the IR interpreter.
//!
//! **Boundary:** this crate never exposes a C ABI, allocator, process, native
//! handle, browser binding, or host effect. Extraction into this crate is
//! incremental and guarded by native/interpreter parity tests.

#![forbid(unsafe_code)]

pub mod binary_float;
pub mod char;
pub mod collections;
pub mod decimal;
pub mod duration;
pub mod environment;
pub mod http_cache;
pub mod http_cookie;
pub mod http_encoding;
pub mod http_form;
pub mod http_multipart;
pub mod http_proxy;
pub mod http_router;
pub mod http_sse;
pub mod http_values;
pub mod http_websocket;
pub mod json;
pub mod network_policy;
pub mod permission_grant;
pub mod range;
pub mod regex;
pub mod scalar;
pub mod stdin;
pub mod temporal;
pub mod text;
