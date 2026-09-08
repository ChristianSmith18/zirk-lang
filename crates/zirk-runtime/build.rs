//! Emits `task_context_native` when the target is one `corosensei` covers and
//! the toolchain supports for running the executor (see
//! `docs/decisions/ADR-004-portabilidad.md`, addendum "portability C"):
//! macOS aarch64, Linux x86_64, Linux aarch64, Windows x86_64.
//!
//! On any other host `crates/zirk-runtime/src/context.rs` uses a thread-backed
//! fallback with the same API, so `cargo test` still builds and passes. The
//! predicate here MUST stay in sync with the `[target.'cfg(...)']` dependency
//! block in `Cargo.toml`.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(task_context_native)");

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let native = matches!(
        (arch.as_str(), os.as_str()),
        ("x86_64", "linux")
            | ("x86_64", "macos")
            | ("x86_64", "windows")
            | ("aarch64", "linux")
            | ("aarch64", "macos")
    );

    if native {
        println!("cargo:rustc-cfg=task_context_native");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
