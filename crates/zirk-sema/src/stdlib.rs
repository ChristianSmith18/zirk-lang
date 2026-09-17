//! Compiler-owned registry for the trusted Zirk standard library boundary.
//!
//! This is deliberately metadata, not language syntax: user packages cannot
//! register entries and no Zirk declaration exposes a Rust/native marker.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StandardLibraryKey {
    pub package: String,
    pub module: String,
    pub name: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardLibraryEntry {
    pub key: StandardLibraryKey,
    pub compiler_binding: &'static str,
}

/// A public module offered by the compiler-provided standard package set. This
/// is resolver metadata only: source authors still see ordinary packages and
/// modules, never a host-language binding marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardModule {
    pub package: &'static str,
    pub module: &'static str,
}

pub fn standard_module(path: &str) -> Option<StandardModule> {
    match path {
        "std.io" => Some(StandardModule {
            package: "std.io",
            module: "io",
        }),
        "std.io::stdin" => Some(StandardModule {
            package: "std.io",
            module: "stdin",
        }),
        "std.io::stdout" => Some(StandardModule {
            package: "std.io",
            module: "stdout",
        }),
        "std.io::stderr" => Some(StandardModule {
            package: "std.io",
            module: "stderr",
        }),
        _ => None,
    }
}

pub fn registry() -> Vec<StandardLibraryEntry> {
    [
        ("stdout", "print", "fn print(...args: String): Void"),
        ("stdout", "println", "fn println(...args: String): Void"),
        ("stdout", "flush", "fn flush(): Result<Void, IoError>"),
        ("stderr", "print", "fn print(...args: String): Void"),
        ("stderr", "println", "fn println(...args: String): Void"),
        ("stderr", "flush", "fn flush(): Result<Void, IoError>"),
        ("stdin", "read_line", "fn read_line(): String?"),
        (
            "stdin",
            "read_line_result",
            "fn read_line_result(): ReadResult<String>",
        ),
        ("stdin", "read_char", "fn read_char(): ReadResult<Char>"),
        (
            "stdin",
            "read_bytes",
            "fn read_bytes(max: UInt64): ReadResult<Array<Byte>>",
        ),
        (
            "stdin",
            "read_exact",
            "fn read_exact(count: UInt64): Result<Array<Byte>, IoError>",
        ),
    ]
    .into_iter()
    .map(|(module, name, signature)| StandardLibraryEntry {
        key: StandardLibraryKey {
            package: "std.io".into(),
            module: module.into(),
            name: name.into(),
            signature: signature.into(),
        },
        compiler_binding: match (module, name) {
            ("stdout", "print") => "std.io.stdout.print",
            ("stdout", "println") => "std.io.stdout.println",
            ("stdout", "flush") => "std.io.stdout.flush",
            ("stderr", "print") => "std.io.stderr.print",
            ("stderr", "println") => "std.io.stderr.println",
            ("stderr", "flush") => "std.io.stderr.flush",
            ("stdin", "read_line") => "std.io.stdin.read_line",
            ("stdin", "read_line_result") => "std.io.stdin.read_line_result",
            ("stdin", "read_char") => "std.io.stdin.read_char",
            ("stdin", "read_bytes") => "std.io.stdin.read_bytes",
            ("stdin", "read_exact") => "std.io.stdin.read_exact",
            _ => unreachable!("the registry table is closed"),
        },
    })
    .collect()
}

/// Looks up only the compiler-owned standard surface. Callers must provide all
/// provenance fields; an external declaration sharing a short name never
/// reaches this registry.
pub fn compiler_binding(
    package: &str,
    module: &str,
    name: &str,
    signature: &str,
) -> Option<&'static str> {
    registry()
        .into_iter()
        .find(|entry| {
            entry.key.package == package
                && entry.key.module == module
                && entry.key.name == name
                && entry.key.signature == signature
        })
        .map(|entry| entry.compiler_binding)
}

/// Returns the closed compiler binding for a member already known to have
/// originated in `std.io`. Import resolution uses this to preserve that
/// provenance at a call site; signatures remain validated separately against
/// the parsed standard source surface.
pub fn compiler_binding_for_member(module: &str, name: &str) -> Option<&'static str> {
    registry()
        .into_iter()
        .find(|entry| entry.key.module == module && entry.key.name == name)
        .map(|entry| entry.compiler_binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entries_are_keyed_by_full_provenance_and_signature() {
        let entries = registry();
        assert!(entries.iter().any(|entry| {
            entry.key.package == "std.io"
                && entry.key.module == "stdout"
                && entry.key.name == "println"
        }));
        assert_eq!(standard_module("std.io::stdin").unwrap().module, "stdin");
        assert!(standard_module("std.unknown").is_none());
    }

    #[test]
    fn external_same_name_never_acquires_a_standard_binding() {
        assert!(
            compiler_binding(
                "std.io",
                "stdout",
                "println",
                "fn println(...args: String): Void"
            )
            .is_some()
        );
        assert!(
            compiler_binding(
                "acme.io",
                "stdout",
                "println",
                "fn println(...args: String): Void"
            )
            .is_none()
        );
        assert!(
            compiler_binding(
                "std.io",
                "stdout",
                "println",
                "fn println(text: String): Void"
            )
            .is_none()
        );
    }
}
