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
#[cfg(feature = "native-http")]
pub mod http_cache;
#[cfg(feature = "native-http")]
pub mod http_cookie;
#[cfg(feature = "http-values")]
pub mod http_encoding;
#[cfg(feature = "native-http")]
pub mod http_form;
#[cfg(feature = "native-http")]
pub mod http_multipart;
#[cfg(feature = "native-http")]
pub mod http_proxy;
#[cfg(feature = "native-http")]
pub mod http_router;
#[cfg(feature = "native-http")]
pub mod http_sse;
#[cfg(feature = "http-values")]
pub mod http_values;
#[cfg(feature = "native-http")]
pub mod http_websocket;
pub mod json;
pub mod locale;
#[cfg(feature = "native-http")]
pub mod network_policy;
pub mod permission_grant;
pub mod range;
pub mod regex;
pub mod scalar;
pub mod stdin;
pub mod temporal;
pub mod text;
pub mod zoned_temporal;
