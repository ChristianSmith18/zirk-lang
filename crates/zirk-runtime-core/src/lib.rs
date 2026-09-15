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
pub mod range;
pub mod regex;
pub mod scalar;
pub mod text;
