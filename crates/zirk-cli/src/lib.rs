//! The `zirk-cli` library: common code for the `zirk` and `zirk-check`
//! executables.
//!
//! The frontend (lex, parse, resolve, check) is always available. The backend
//! driver (lower, emit, link) is only compiled when the `backend` feature is
//! enabled.

pub mod codes;
pub mod frontend;
pub mod modules;

#[cfg(feature = "backend")]
pub mod driver;
