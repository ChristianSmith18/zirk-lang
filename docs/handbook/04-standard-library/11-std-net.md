# `std.net`

`std.net` is Zirk's transport-level networking module. Its target contract
covers immutable address values, DNS resolution, TCP streams and listeners,
UDP sockets, local sockets, network-interface inspection, and authenticated
TLS connections. The current native preview exposes address values, bounded
DNS resolution, TCP streams and listeners, UDP datagrams, local streams and
listeners, permission-gated interface snapshots, and authenticated TLS
connections.
HTTP is a separate protocol layer documented in
[`std.http`](12-std-http.md).

Focused UDP, local IPC, and interface-enumeration runtime tests pass on macOS
ARM64, Linux ARM64, and Linux x86_64. The public interface API has approved and
denied end-to-end tests on macOS ARM64. Windows verification for these additions
remains pending. Client TLS configuration, client and server mutual TLS,
negotiated-peer metadata, and upgrading an existing TCP stream to TLS have
compiled-Zirk loopback evidence on macOS ARM64. Their cross-target verification
and HTTP/3 remain pending.
The full `zirk-runtime` library suite also passes in a Linux ARM64 container
(517 passed, 3 ignored); this is runtime-library evidence and does not establish
compiled-Zirk CLI behavior on that target.

The module follows the standard-library I/O contract: operations return typed
`Result` values, waiting suspends the current task rather than occupying a
scheduler thread, cancellation closes or restores owned resources, buffers are
bounded, and every external effect is checked against the effective project
permissions.

> **Implementation status:** this chapter documents the public API currently
> available in the native preview. Target-level evidence is listed above;
> Windows runtime behavior for UDP, local IPC, and interface inspection is
> pending. Unsupported operations must preserve the documented permissions,
> cancellation, and resource limits.
>
> The current native preview exposes `std.net::tcp.TCPStream` and
> `std.net::tcp.TCPListener`, both with managed
> `Resource<TCPResourceError>` scope cleanup. Streams support exact
> `tcp://host:port` connect grants, bounded read/write operations, and
> `TCPStream.peer_address()` for the numeric remote endpoint. Listeners
> support numeric-IP bind under a separate exact `network.listen` address grant
> and accept connections into streams owned by the accepting task. These `Resource<E>` handles are task-owned and cannot be captured across a `Thread.run` boundary; the checker rejects TCP and TLS resource captures with `E0482`, while runtime operations independently enforce the owner task. String
> endpoint entry points remain available. `std.net::address.IPAddress` parses
> canonical IPv4/IPv6 literals without DNS or scoped IPv6 zones, and
> `SocketAddress.parse(ip, port)` combines one such validated literal with a
> non-zero `UInt16` port. `TCPStream.connect_to` and `TCPListener.bind_to`
> accept that typed address and produce the same canonical permission-scoped
> TCP origin. `HostPortAddress.parse(host, port)` now creates a canonical
> outbound host/port authority, including IDNA normalization and bracketed
> IPv6 formatting, without resolving DNS. `TCPStream.connect_to_host` checks
> the exact canonical origin grant before resolving the host. The bundled
> `std.net::udp`, `std.net::local`, and `std.net::interfaces` modules expose
> bounded datagrams, local byte streams, and permission-gated interface
> snapshots. The `std.net::dns` module exposes
> `DNS.resolve(origin)`, which requires an exact approved `tcp://`, `http://`,
> or `https://` origin and returns a bounded `List<String>` of validated IP
> literals. It uses the host resolver for address records only. The
> `std.net::tls` preview exposes authenticated `TLSStream` client connections
> with system trust roots or explicit DER roots, TLS 1.3 defaults, and opt-in
> TLS 1.2. `TLSListener` exposes a server listener with explicit DER identity
> and the separate listen permission.

The TLS preview accepts an HTTPS origin string and uses the operating system's
trust roots by default. `TLSStream.connect_with_trust_roots(endpoint,
roots_der, allow_tls12)` accepts an explicit non-empty set of DER root
certificates; malformed roots are rejected and the aggregate trust store is
bounded. TLS 1.2 is enabled only when `allow_tls12` is `true`. Both connection
forms validate the certificate chain, host name, and SNI and require an exact
network permission grant. Streams expose bounded reads, backpressured writes,
explicit close-notify shutdown, and task-owned resource cleanup. `TLSListener.bind`
accepts a numeric `tcp://` endpoint, an explicit DER certificate chain and
private key, and the TLS 1.2 opt-in. Binding requires a separate exact
`network.listen.addresses` grant; accepted TLS streams belong to the accepting
task. `TLSStream.peer_address()` returns the numeric remote address.
`TLSStream.connect_with_options(endpoint, roots_der, allow_tls12, protocols)`
combines ALPN with an explicit trust store and TLS 1.2 opt-in; an empty trust
store uses system roots. `TLSStream.negotiated_alpn()` returns the selected
protocol or `null` when the peer selected none. `TCPStream.upgrade_tls(endpoint,
roots_der, allow_tls12, protocols)` transfers its already-connected socket to
an authenticated `TLSStream`; it does not open another TCP connection. The
caller must hold both the exact `tcp://` grant used to connect and the matching
`https://` grant for the TLS origin and peer. Invalid configuration or denied
authority leaves the TCP stream available. Once the TLS handshake begins, the
TCP handle is consumed, and a failed or cancelled handshake closes that socket.
For HTTP servers,
`TLSListener.bind_for_http` advertises `h2` and `http/1.1` through ALPN;
generic `TLSListener.bind` does not advertise an application protocol.

## Imports and naming

The namespace and direct-import forms refer to the same symbols:

```zirk
import { HostPortAddress, IPAddress, SocketAddress } from std.net::address;
import { TCPStream } from std.net::tcp;

mut outbound = HostPortAddress.parse("Exämple.com", 443);
mut literal = IPAddress.parse("127.0.0.1");
```

Use `IPv4` and `IPv6` with a lowercase `v`. Other networking acronyms remain
uppercase: `DNS`, `TCP`, `UDP`, and `TLS`. Import TLS declarations directly
from `std.net::tls`, for example `import { TLSStream } from std.net::tls;`.

## Address values

The current preview publishes these immutable address values:

| Type | Meaning |
| --- | --- |
| `IPAddress` | A validated canonical IPv4 or IPv6 literal. |
| `SocketAddress` | An IP literal paired with a non-zero port. |
| `HostPortAddress` | A canonical outbound DNS name or IP literal paired with a non-zero port. |

Parsing an `IPAddress` never performs DNS:

```zirk
match IPAddress.parse("2001:0db8::1") {
    Ok(address) => stdout.println(address.as_string());
    Error(error) => stderr.println(error);
}
```

Invalid syntax returns `AddressError`. The current `SocketAddress.parse(ip,
port)` preview accepts only an IP literal and a non-zero `UInt16` port.
`HostPortAddress.parse(host, port)`
normalizes domain names with IDNA and IP literals without DNS; IPv6 formatting
uses brackets when combined with a port. Resolution remains explicit at the
connection boundary: `TCPStream.connect_to_host` checks authorization before
the host resolver runs. Both address types expose `as_tcp_origin()` to return
their canonical `tcp://` origin; ordinary callers can use the typed
`connect_to`/`connect_to_host` methods and avoid handling that string directly.

## DNS

The target DNS contract includes configurable record types and resolver
security metadata. The current preview uses the operating system resolver for
address records only; custom DNS-over-TLS, DNS-over-HTTPS, reverse lookup, and
DNSSEC reporting are not exposed.

```zirk
match DNS.resolve("https://example.com:443") {
    Ok(addresses) => {
        for address in addresses {
            stdout.println(address);
        }
    }
    Error(error) => stderr.println(error);
}
```

Resolution is task-aware and cancellation-safe. Positive and negative answers
may be cached according to their TTL, but both caches are bounded. A cached
answer does not bypass permissions: every resolved address is checked again
immediately before every connect attempt. Hostname answers must be globally
routable. Special-purpose prefixes follow IANA's global-reachability
assignments, including globally reachable exceptions inside otherwise
restricted protocol-assignment blocks. IPv4/IPv6 documentation ranges,
benchmarking, private, shared, and local addresses are rejected. Translation
and transition prefixes remain blocked even when a registry permits forwarding,
because they can route a hostname destination into another address family or
scope. This prevents a permitted host name from using DNS rebinding to reach a
forbidden local or private address. A
fresh answer set is added to the cache only after every candidate passes the
origin and address policy checks; a mixed public/private set is rejected and
never retained. A runtime loopback regression seeds a cached host name with a
private listener address and confirms the transport rejects it before the
listener receives a connection; it passes on macOS ARM64, Linux ARM64, and
Linux x86_64 Docker emulation. Connect candidates with a nonzero IPv6 interface
scope ID are rejected because an origin grant does not identify a local
network interface. IPv4-mapped IPv6 answers follow the policy for their
embedded IPv4 address, so mapped private and documentation addresses remain
blocked.

`DNS.resolve(origin: String): Result<List<String>, DNSError>` requires an
origin that exactly matches an approved network connect scope before the host
resolver runs. The runtime bounds the answer set and rejects forbidden
hostname addresses before returning any result. A later connect independently
rechecks the effective grant and each resolved address. Timeouts, resolver
failures, denied origins, and disallowed address sets remain typed errors.

## TCP clients

`TCPStream.connect` accepts a canonical `tcp://host:port` origin.
`TCPStream.connect_to` accepts a validated IP `SocketAddress`, while
`TCPStream.connect_to_host` accepts a `HostPortAddress`. Each form suspends the
current cooperative task while connecting; host-name connections check the
exact origin grant before DNS and validate every resolved address.

```zirk
import { HostPortAddress } from std.net::address;
import { TCPStream } from std.net::tcp;
import { stderr, stdout } from std.io;

fn main(): Void {
    match HostPortAddress.parse("example.com", 443) {
        Ok(address) => match TCPStream.connect_to_host(address) {
            Ok(stream) => {
                mut request_bytes: Array<Byte> = [71, 69, 84];
                match stream.write_all(request_bytes) {
                    Ok(_) => stdout.println("request sent");
                    Error(error) => stderr.println(error);
                }
                _ = stream.close();
                stdout.println("stream closed");
            }
            Error(error) => stderr.println(error);
        };
        Error(error) => stderr.println(error);
    }
}
```

A `TCPStream` is an ordered byte stream:

- `read(max_bytes)` returns `TCPReadResult<Array<Byte>>` with `Value`, `End`,
  or `Error`;
- `write(bytes)` may succeed after writing only part of the input and reports
  the written byte count;
- `write_all(bytes)` applies backpressure and completes only after all bytes
  have been accepted or a typed error occurs;
- `peer_address()` returns the numeric remote socket address as a string;
- `shutdown()` performs an explicit write-side shutdown;
- `close()` is idempotent; task-owned resource cleanup also closes an
  abandoned stream when its owning task exits.

The current source API does not expose socket options or configurable
timeouts. The runtime uses a fixed 30-second connect/TLS-handshake timeout and
caps each read or write buffer at 1 MiB. Operating-system keepalive and
no-delay settings remain platform defaults. Cancellation never reports bytes
as unwritten if the runtime has already committed them to the operating
system.

## TCP servers

`TCPListener.bind` validates the listen address and permission before opening a
socket. `accept` suspends its task and returns a stream owned by the accepting
task. The accepted `TCPStream.peer_address()` reports the connecting peer's
numeric address; outbound streams report the remote endpoint they connected to.

```zirk
import { TCPListener } from std.net::tcp;
import { stderr, stdout } from std.io;

fn main(): Void {
    match TCPListener.bind("tcp://0.0.0.0:8080") {
        Ok(listener) => {
            match listener.accept() {
                Ok(stream) => {
                    stdout.println("accepted connection");
                    _ = stream.close();
                }
                Error(error) => stderr.println(error);
            }
            _ = listener.close();
        }
        Error(error) => stderr.println(error);
    }
}
```

Canceling `accept` removes only that waiter unless its owning scope also closes
the listener. `std.net` exposes one accept operation on the caller's task; it
does not create an application-level accept queue or spawn handlers. The
pending-connection backlog is controlled by Tokio/the operating system and is
not configurable through this API. HTTP server request limits and HTTP/2 stream
concurrency are protocol-level policies documented in `std.http`. The runtime
uses its reactor; it does not create one thread per listener or connection.

## UDP datagrams

`UDPSocket.connect(endpoint)` creates a task-owned socket scoped to one
approved peer. `UDPSocket.bind(endpoint)` creates a receiver under the separate
UDP listen permission. `receive(max_bytes)` returns one complete `UDPDatagram`
with its canonical numeric peer address; oversized datagrams fail instead of
being silently truncated.

UDP preserves message boundaries. `receive_from` returns the payload and source
address together; it never silently truncates a datagram. If a configured
buffer cannot hold a packet, the result identifies the required size or returns
a typed size-limit error according to the selected receive policy.

An unconnected socket can reply only to destinations that the application
approved:

```zirk
import { UDPSocket } from std.net::udp;
import { stdout } from std.io;

fn main(): Void {
    match UDPSocket.bind("udp://127.0.0.1:9000") {
        Ok(socket) => {
            match socket.receive(65507 as UInt64) {
                Ok(datagram) => {
                    stdout.println(datagram.peer());
                    match socket.send_to(datagram.bytes(), datagram.peer()) {
                        Ok(_) => stdout.println("reply sent");
                        Error(error) => stdout.println(error.to_string());
                    }
                }
                Error(error) => stdout.println(error.to_string());
            }
            _ = socket.close();
        }
        Error(error) => stdout.println(error.to_string());
    }
}
```

An unconnected socket uses `send_to` and `receive_from`; a connected UDP socket
uses `send` and `receive` while retaining datagram semantics. Broadcast and
multicast participation require explicit options and matching permissions.
Datagram size, queue depth, and multicast membership are bounded resources.
Broadcast and multicast require their separate manifest grants. `UDPError`
reports denied destinations, timeouts, cancellation, capacity, and transport
failures; `UDPResourceError` is returned by close and membership operations.

## TLS

The current preview exposes authenticated `TLSStream.connect` and
`TLSStream.connect_with_trust_roots` operations. The default uses system trust
roots, validates the certificate chain and host name, sends SNI, and negotiates
TLS 1.3. TLS 1.2 can be enabled only through the explicit trust-root overload.
Server-side TLS uses `TLSListener.bind` with caller-supplied DER identity
material. `TLSListener.bind` creates a general TLS listener without advertising
an application protocol. For an HTTP server, use
`TLSListener.bind_for_http(endpoint, certificate_chain_der, private_key_der,
allow_tls12)`: it advertises `h2` and `http/1.1` through ALPN so
`HTTPServer.serve_tls` or `HTTPRouter.serve_tls` can select the negotiated
protocol. `HTTPServer.bind_tls` is the HTTP-facing convenience constructor for
the same listener setup. Both listener constructors validate the explicit
identity and check the exact `network.listen.addresses` grant before binding.
TLS 1.2 remains opt-in through the `allow_tls12` argument.

```zirk
import { TLSStream } from std.net::tls;
import { stdout } from std.io;

fn main(): Void {
    match TLSStream.connect("https://example.com") {
        Ok(stream) => {
            stdout.println(stream.is_closed());
            _ = stream.close();
        }
        Error(_) => stdout.println("TLS connection failed");
    }
}
```

The trust-root overload accepts explicit DER roots and an opt-in TLS 1.2 flag.
`TLSStream.connect_with_alpn(endpoint, protocols)` uses system trust roots and
TLS 1.3 while sending the caller's ordered ALPN identifiers.
`TLSStream.connect_with_options(endpoint, roots_der, allow_tls12, protocols)`
combines those settings; an empty roots array selects system roots. After the
handshake, `stream.negotiated_alpn()` returns the selected protocol or `null`.
The runtime accepts
at most 16 unique printable ASCII identifiers, each at most 255 bytes, with a
1 KiB aggregate limit; invalid lists fail before socket creation. Certificate
validation cannot be disabled by a general-purpose `insecure` switch. The
`TLSStream.connect_with_client_identity(endpoint, roots_der,
client_certificate_chain_der, client_private_key_der, allow_tls12, protocols)`
overload presents an explicit DER client identity to servers that require
mutual authentication. The key is not discovered from the environment or a
file; the runtime bounds and clears its temporary DER copy after constructing
the TLS signer. The signer remains private to the stream's TLS configuration.
For servers, `TLSListener.bind_with_client_auth(endpoint,
certificate_chain_der, private_key_der, client_roots_der, allow_tls12)` requires
an explicit non-empty DER trust-root list and rejects clients that do not
present a valid certificate chain. On an authenticated stream,
`peer_certificate_sha256()` returns the lowercase SHA-256 fingerprint of the
validated peer leaf certificate, or `null` when no peer certificate is
available for that TLS role. Session resumption and TLS key logging are not
part of the current preview.

The TLS stream preserves backpressure, cancellation, and resource ownership. A
failed or canceled handshake closes resources it owns. When an existing TCP
stream is upgraded, a validation or permission error occurs before ownership
transfer; after the handshake starts, the TCP stream belongs to the TLS attempt
and cannot return to plaintext use.

## Local sockets

`LocalStream` and `LocalListener` provide task-owned inter-process byte
streams. Unix targets use domain sockets; Windows uses named pipes with the
`npipe://./pipe/name` endpoint form. Connect and listen require distinct exact
endpoint grants. Reads and writes are bounded, `read` reports EOF through
`LocalReadResult.End`, and closing a listener removes only the socket entry it
owns. `LocalListener.bind` requires an explicit
`LocalStaleEndpointPolicy`: `RejectExisting` is the usual choice;
`RemoveStaleSocket` removes only an inactive Unix socket entry. Windows named
pipes do not support stale filesystem cleanup.

```zirk
import { LocalListener, LocalStaleEndpointPolicy } from std.net::local;
import { stdout } from std.io;

match LocalListener.bind("unix:///tmp/zirk.sock", LocalStaleEndpointPolicy.RejectExisting) {
    Ok(listener) => {
        match listener.accept() {
            Ok(stream) => _ = stream.close();
            Error(error) => stdout.println(error.to_string());
        }
        _ = listener.close();
    }
    Error(error) => stdout.println(error.to_string());
}
```

## Network-interface snapshots

`NetworkInterfaces.snapshot()` requires the explicit runtime permission
`network.inspect.interfaces` and returns a point-in-time list of typed
`NetworkInterfaceAddress` records. Each record includes the interface name and
index, operational status, loopback and point-to-point flags, IP address,
prefix length, and an optional scope ID for IPv6 link-local addresses. The
snapshot is bounded and sorted deterministically. Inspection grants no
permission to connect, listen, or select a network interface.

```zirk
import { NetworkInterfaces } from std.net::interfaces;
import { stdout } from std.io;

match NetworkInterfaces.snapshot() {
    Ok(addresses) => {
        for address in addresses {
            stdout.println(address.name + " " + address.address.as_string());
        }
    }
    Error(error) => stdout.println(error.to_string());
}
```

Raw sockets, packet capture, Ethernet frames, DHCP, BGP, VPN construction, Tor,
QUIC, and general kernel-network administration are not part of the current
`std.net` preview.

## Permissions

Network authority distinguishes outbound and inbound scope:

```zirk
permissions {
    network {
        connect: { origins: ["https://example.com:443"]; during: runtime; }
        listen: { addresses: ["127.0.0.1:8080"]; during: runtime; }
        udp_send: { destinations: ["udp://192.0.2.9:5353"]; during: runtime; }
        udp_listen: { addresses: ["127.0.0.1:5353"]; during: runtime; }
        local_connect: { endpoints: ["unix:///tmp/zirk.sock"]; during: runtime; }
        local_listen: { endpoints: ["unix:///tmp/zirk.sock"]; during: runtime; }
        inspect_interfaces: { enabled: true; during: runtime; }
    }
}
```

`connect.origins` authorizes protocols, host names, and ports. It does not by
itself authorize an arbitrary resolved address: resolved IPs, redirects,
reconnects, proxy targets, and TLS server names are validated against the
effective grant at every boundary. Raw TCP-only connections use a canonical
`tcp://host:port` origin so a TCP grant cannot be confused with an HTTP or TLS
origin. `listen.addresses` separately authorizes
local bind addresses and ports. UDP broadcast/multicast, local sockets, custom
resolvers, and interface inspection use explicit scopes rather than inheriting
broad outbound authority.

`udp_send` and `udp_listen` are separate. `local_connect` and `local_listen`
are separate exact endpoint grants. `inspect_interfaces` grants only the
metadata snapshot capability; it does not grant network traffic or binding.

Package `.zkinit` files declare required scopes; they do not grant them. The
application approval command collects requirements across the locked dependency
graph and requests consent for their union with application scopes. Missing
authority produces the operation-specific permission error before the external
effect occurs. Errors are reported through each operation's enum, including
`DNSError`, `TCPError`, `TLSError`, `UDPError`, `LocalError`, and
`NetworkInterfaceError`. Runtime library code never prompts or edits
`.zkinit`; trusted tooling owns consent.

## Errors, limits, and portability

The current preview exposes `AddressError`, `DNSError`, `TCPError`, `TLSError`,
`UDPError`, `LocalError`, `NetworkInterfaceError`, `TCPReadResult<T>`,
`TLSReadResult<T>`, `LocalReadResult<T>`, `TCPResourceError`,
`TLSResourceError`, and `UDPResourceError`. Operation results distinguish permission denial, invalid
addresses, disallowed resolved addresses, DNS failure and answer limits,
transport errors, capacity limits, timeout, cancellation, closed resources,
and wrong-task ownership where applicable. Closing a resource wraps its
transport error in the corresponding resource error. `TCPResourceError` and
`TLSResourceError` implement `Error`; their public constructors accept the
matching transport error enum, while `message()`, `code()`, and `cause()` expose
the standard error interface.

UDP datagrams are capped at 65,507 bytes for IPv4 and 65,527 bytes for IPv6.
TCP and local stream reads and writes are capped at 1 MiB per call. Interface
snapshots are capped at 256 address records and 256 UTF-8 bytes per interface
name; exceeding a bound returns an error rather than a partial list. The
runtime also bounds DNS answers and resource-handle registries. These limits
are runtime policy and are not publicly configurable. Exact operating-system
error numbers and some transport capabilities remain platform-specific; the
source API does not expose a general platform-cause field.

---

**Previous:** [← Parallel CPU operations](10a-std-parallel.md) · **Next:** [std.http →](12-std-http.md)
