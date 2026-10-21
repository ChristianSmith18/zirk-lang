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
        "std.environment" => Some(StandardModule {
            package: "std.environment",
            module: "environment",
        }),
        "std.environment::environment" => Some(StandardModule {
            package: "std.environment",
            module: "environment",
        }),
        "std.http" => Some(StandardModule {
            package: "std.http",
            module: "http",
        }),
        "std.http::http" => Some(StandardModule {
            package: "std.http",
            module: "http",
        }),
        "std.net" => Some(StandardModule {
            package: "std.net",
            module: "net",
        }),
        "std.net::tcp" => Some(StandardModule {
            package: "std.net",
            module: "tcp",
        }),
        "std.net::dns" => Some(StandardModule {
            package: "std.net",
            module: "dns",
        }),
        "std.net::address" => Some(StandardModule {
            package: "std.net",
            module: "address",
        }),
        "std.net::tls" => Some(StandardModule {
            package: "std.net",
            module: "tls",
        }),
        "std.net::udp" => Some(StandardModule {
            package: "std.net",
            module: "udp",
        }),
        "std.net::local" => Some(StandardModule {
            package: "std.net",
            module: "local",
        }),
        "std.net::interfaces" => Some(StandardModule {
            package: "std.net",
            module: "interfaces",
        }),
        _ => None,
    }
}

pub fn registry() -> Vec<StandardLibraryEntry> {
    let mut entries: Vec<StandardLibraryEntry> = [
        ("stdout", "print", "fn print(...args: Stringifiable): Void"),
        (
            "stdout",
            "println",
            "fn println(...args: Stringifiable): Void",
        ),
        ("stdout", "flush", "fn flush(): Result<Void, IoError>"),
        ("stderr", "print", "fn print(...args: Stringifiable): Void"),
        (
            "stderr",
            "println",
            "fn println(...args: Stringifiable): Void",
        ),
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
    .collect();
    entries.push(StandardLibraryEntry {
        key: StandardLibraryKey {
            package: "std.environment".into(),
            module: "environment".into(),
            name: "get".into(),
            signature: "static get<T from EnvironmentParsable<T> = String>(name: String): Result<T, EnvironmentError>".into(),
        },
        compiler_binding: "std.environment.Environment.get",
    });
    entries.push(StandardLibraryEntry {
        key: StandardLibraryKey {
            package: "std.environment".into(),
            module: "environment".into(),
            name: "list_names".into(),
            signature: "static list_names(): Result<List<String>, EnvironmentError>".into(),
        },
        compiler_binding: "std.environment.Environment.list_names",
    });
    entries
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
    // These are private implementation leaves, callable only while checking
    // the compiler-bundled source file. They are intentionally absent from
    // `registry()`, which describes the public package surface.
    if let ("stdout", "write_line") = (module, name) {
        return Some("std.io.stdout.write_line");
    }
    if let ("stdout", "write") = (module, name) {
        return Some("std.io.stdout.write");
    }
    if let ("stderr", "write_line") = (module, name) {
        return Some("std.io.stderr.write_line");
    }
    if let ("stderr", "write") = (module, name) {
        return Some("std.io.stderr.write");
    }
    registry()
        .into_iter()
        .find(|entry| entry.key.module == module && entry.key.name == name)
        .map(|entry| entry.compiler_binding)
}

/// Private environment implementation leaf. It is only considered while
/// checking the compiler-bundled source module; applications cannot import or
/// call it as a public host API.
pub fn compiler_binding_for_environment_member(module: &str, name: &str) -> Option<&'static str> {
    if module != "environment" {
        return None;
    }
    match name {
        "get_string_native" => Some("std.environment.Environment.get"),
        "get_secret_value" => Some("std.environment.Environment.get_secret_value"),
        "contains_authorized" => Some("std.environment.Environment.contains_authorized"),
        _ => None,
    }
}

/// Private network implementation leaves. These names are bindable only
/// while checking their compiler-bundled `std.net` source module, by full
/// package provenance rather than by short name alone.
pub fn compiler_binding_for_network_member(module: &str, name: &str) -> Option<&'static str> {
    if module == "interfaces" {
        return match name {
            "snapshot_native" => Some("std.net.interfaces.snapshot"),
            _ => None,
        };
    }
    if module == "dns" {
        return match name {
            "resolve_native" => Some("std.net.dns.resolve"),
            _ => None,
        };
    }
    if module == "udp" {
        return match name {
            "connect_native" => Some("std.net.udp.connect"),
            "bind_native" => Some("std.net.udp.bind"),
            "send_native" => Some("std.net.udp.send"),
            "send_to_native" => Some("std.net.udp.send_to"),
            "send_broadcast_native" => Some("std.net.udp.send_broadcast"),
            "send_multicast_native" => Some("std.net.udp.send_multicast"),
            "join_multicast_native" => Some("std.net.udp.join_multicast"),
            "leave_multicast_native" => Some("std.net.udp.leave_multicast"),
            "receive_native" => Some("std.net.udp.receive"),
            "local_address_native" => Some("std.net.udp.local_address"),
            "close_native" => Some("std.net.udp.close"),
            "is_closed_native" => Some("std.net.udp.is_closed"),
            _ => None,
        };
    }
    if module == "local" {
        return match name {
            "connect_native" => Some("std.net.local.connect"),
            "bind_native" => Some("std.net.local.bind"),
            "accept_native" => Some("std.net.local.accept"),
            "read_native" => Some("std.net.local.read"),
            "write_native" => Some("std.net.local.write"),
            "write_all_native" => Some("std.net.local.write_all"),
            "shutdown_native" => Some("std.net.local.shutdown"),
            "close_stream_native" => Some("std.net.local.close_stream"),
            "is_stream_closed_native" => Some("std.net.local.is_stream_closed"),
            "close_listener_native" => Some("std.net.local.close_listener"),
            "is_listener_closed_native" => Some("std.net.local.is_listener_closed"),
            _ => None,
        };
    }
    if module == "address" {
        return match name {
            "parse_native" => Some("std.net.address.parse"),
            "origin_parse_native" => Some("std.net.address.origin_parse"),
            _ => None,
        };
    }
    if module == "tls" {
        return match name {
            "connect_native" => Some("std.net.tls.connect"),
            "connect_with_trust_roots_native" => Some("std.net.tls.connect_with_trust_roots"),
            "connect_with_options_native" => Some("std.net.tls.connect_with_alpn"),
            "connect_with_client_identity_native" => {
                Some("std.net.tls.connect_with_client_identity")
            }
            "bind_native" => Some("std.net.tls.listen"),
            "bind_with_client_auth_native" => Some("std.net.tls.listen_with_client_auth"),
            "bind_http_native" => Some("std.net.tls.listen_http"),
            "accept_native" => Some("std.net.tls.accept"),
            "close_listener_native" => Some("std.net.tls.close_listener"),
            "is_listener_closed_native" => Some("std.net.tls.is_listener_closed"),
            "read_native" => Some("std.net.tls.read"),
            "write_all_native" => Some("std.net.tls.write_all"),
            "shutdown_native" => Some("std.net.tls.shutdown"),
            "close_native" => Some("std.net.tls.close"),
            "is_closed_native" => Some("std.net.tls.is_closed"),
            "peer_address_native" => Some("std.net.tls.peer_address"),
            "negotiated_alpn_native" => Some("std.net.tls.negotiated_alpn"),
            "peer_certificate_sha256_native" => Some("std.net.tls.peer_certificate_sha256"),
            _ => None,
        };
    }
    if module != "tcp" {
        return None;
    }
    match name {
        "connect_native" => Some("std.net.tcp.connect"),
        "upgrade_tls_native" => Some("std.net.tcp.upgrade_tls"),
        "read_native" => Some("std.net.tcp.read"),
        "write_native" => Some("std.net.tcp.write"),
        "write_all_native" => Some("std.net.tcp.write_all"),
        "peer_address_native" => Some("std.net.tcp.peer_address"),
        "shutdown_native" => Some("std.net.tcp.shutdown"),
        "close_native" => Some("std.net.tcp.close"),
        "is_closed_native" => Some("std.net.tcp.is_closed"),
        "bind_native" => Some("std.net.tcp.listen"),
        "accept_native" => Some("std.net.tcp.accept"),
        "close_listener_native" => Some("std.net.tcp.close_listener"),
        "is_listener_closed_native" => Some("std.net.tcp.listener_is_closed"),
        _ => None,
    }
}

/// Private HTTP implementation leaves, bindable only from the compiler's
/// bundled `std.http` source module.
pub fn compiler_binding_for_http_member(name: &str) -> Option<&'static str> {
    match name {
        "get_native" => Some("std.http.get"),
        "get_cached_native" => Some("std.http.get_cached"),
        "request_native" => Some("std.http.request"),
        "request_cached_native" => Some("std.http.request_cached"),
        "request_secret_native" => Some("std.http.request_secret"),
        "head_native" => Some("std.http.head"),
        "query_native" => Some("std.http.query"),
        "query_with_content_type_native" => Some("std.http.query_with_content_type"),
        "post_native" => Some("std.http.post"),
        "post_json_native" => Some("std.http.post_json"),
        "post_bytes_native" => Some("std.http.post_bytes"),
        "request_stream_native" => Some("std.http.request_stream"),
        "request_stream_result_native" => Some("std.http.request_stream_result"),
        "request_stream_named_result_native" => Some("std.http.request_stream_named_result"),
        "register_server_producer_native" => Some("std.http.server_response_stream_register"),
        "stream_buffered_native" => Some("std.http.response_to_stream"),
        "get_stream_native" => Some("std.http.get_stream"),
        "read_chunk_native" => Some("std.http.response_read_chunk"),
        "read_all_native" => Some("std.http.response_read_all"),
        "read_sse_native" => Some("std.http.response_read_sse_event"),
        "sse_heartbeat_count_native" => Some("std.http.response_sse_heartbeat_count"),
        "close_native" => Some("std.http.response_stream_close"),
        "parse_native" => Some("std.http.url_normalize"),
        "resolve_native" => Some("std.http.url_resolve"),
        "text_native" => Some("std.http.response_text"),
        "header_values_native" => Some("std.http.response_header_values"),
        "request_header_values_native" => Some("std.http.request_header_values"),
        "route_match_native" => Some("std.http.route_match"),
        "decode_fields_intrinsic" => Some("std.http.form_parse"),
        "multipart_parse_intrinsic" => Some("std.http.multipart_parse"),
        "read_upload_native" => Some("std.http.upload_read"),
        "close_upload_native" => Some("std.http.upload_close"),
        "multipart_encode_intrinsic" => Some("std.http.multipart_encode"),
        "serve_once_native" => Some("std.http.server.serve_once"),
        "serve_http3_native" => Some("std.http.server.serve_http3"),
        "serve_http3_websocket_once_native" => Some("std.http.server.websocket_serve_http3_once"),
        "serve_websocket_once_native" => Some("std.http.server.websocket_serve_once"),
        "serve_tls_websocket_once_native" => Some("std.http.server.websocket_serve_tls_once"),
        "serve_tls_once_native" => Some("std.http.server.serve_tls_once"),
        "serve_tls_requests_native" => Some("std.http.server.serve_tls_requests"),
        "websocket_connect_native" => Some("std.http.websocket.connect"),
        "websocket_connect_http3_native" => Some("std.http.websocket.connect_http3"),
        "websocket_connect_http3_with_trust_roots_native" => {
            Some("std.http.websocket.connect_http3_with_trust_roots")
        }
        "websocket_connect_with_trust_roots_native" => {
            Some("std.http.websocket.connect_with_trust_roots")
        }
        "websocket_send_native" => Some("std.http.websocket.send"),
        "websocket_receive_native" => Some("std.http.websocket.receive"),
        "websocket_close_native" => Some("std.http.websocket.close"),
        "websocket_close_with_native" => Some("std.http.websocket.close_with"),
        "decode_text_native" => Some("std.http.response_text"),
        _ => None,
    }
}

/// Private reusable-client implementation leaves. These names are bindable
/// only from the compiler-bundled `std.http` source module.
pub fn compiler_binding_for_http_client_member(name: &str) -> Option<&'static str> {
    match name {
        "create_native" => Some("std.http.client_create"),
        "create_with_options_native" => Some("std.http.client_create_with_options"),
        "create_with_trust_roots_native" => Some("std.http.client_create_with_trust_roots"),
        "create_with_proxy_native" => Some("std.http.client_create_with_proxy"),
        "create_with_proxy_and_trust_roots_native" => {
            Some("std.http.client_create_with_proxy_and_trust_roots")
        }
        "request_native" => Some("std.http.client_request"),
        "request_stream_native" => Some("std.http.client_request_stream"),
        "enable_cookies_native" => Some("std.http.client_enable_cookies"),
        "clear_cookies_native" => Some("std.http.client_clear_cookies"),
        "close_native" => Some("std.http.client_close"),
        _ => None,
    }
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
        assert_eq!(standard_module("std.http").unwrap().module, "http");
        assert!(standard_module("std.unknown").is_none());
    }

    #[test]
    fn external_same_name_never_acquires_a_standard_binding() {
        assert!(
            compiler_binding(
                "std.io",
                "stdout",
                "println",
                "fn println(...args: Stringifiable): Void"
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
