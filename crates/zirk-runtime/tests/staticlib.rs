//! Verifies that `zirk-runtime` produces a linkable static library and that
//! its symbols cross the C ABI boundary without mangling.
//!
//! Symbols are searched for as text inside the file: the symbol table of a `.a`
//! (Unix) and of a `.lib` (Windows) contains the literal name. This avoids
//! depending on `nm`, `objdump` or `dumpbin`, which differ across platforms.

use std::path::PathBuf;

/// Locates the artifact directory from the test executable, which Cargo places
/// in `<target>/<profile>/deps/`.
fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("could not locate the test executable");
    exe.parent()
        .and_then(|deps| deps.parent())
        .expect("unexpected Cargo directory layout")
        .to_path_buf()
}

fn staticlib_path() -> PathBuf {
    let dir = profile_dir();
    let candidates = if cfg!(windows) {
        vec![dir.join("zirk_runtime.lib")]
    } else {
        vec![dir.join("libzirk_runtime.a")]
    };

    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| {
            panic!(
                "static library not found in {}; \
                 is crate-type `staticlib` still declared in Cargo.toml?",
                dir.display()
            )
        })
}

#[test]
fn a_static_library_is_produced() {
    let lib = staticlib_path();
    let metadata = std::fs::metadata(&lib).expect("could not read the library");

    assert!(
        metadata.len() > 0,
        "the static library is empty: {}",
        lib.display()
    );
}

#[test]
fn the_lifecycle_symbols_carry_no_mangling() {
    let lib = staticlib_path();
    let bytes = std::fs::read(&lib).expect("could not read the library");

    for symbol in [
        "zirk_rt_init",
        "zirk_rt_shutdown",
        "zirk_str_from_utf8",
        "zirk_str_from_i32",
        "zirk_str_from_bool",
        "zirk_str_eq",
        "zirk_io_println",
        "zirk_rt_overflow",
        "zirk_rt_division_by_zero",
    ] {
        assert!(
            contains(&bytes, symbol.as_bytes()),
            "symbol `{symbol}` does not appear unmangled in {}",
            lib.display()
        );
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
