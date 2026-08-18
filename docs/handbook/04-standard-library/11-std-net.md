# `std.net`

`std.net` is Zirk's transport-level networking module. It provides immutable
address values, DNS resolution, TCP streams and listeners, UDP sockets, local
sockets, network-interface inspection, and authenticated TLS connections. HTTP
is a separate protocol layer documented in [`std.http`](12-std-http.md).

The module follows the standard-library I/O contract: operations return typed
`Result` values, waiting suspends the current task rather than occupying a
scheduler thread, cancellation closes or restores owned resources, buffers are
bounded, and every external effect is checked against the effective project
permissions.

> **Implementation status:** this chapter defines the normative public contract.
> The current compiler and runtime may implement only a subset. Unsupported
> operations must fail explicitly; they must not silently weaken validation,
> TLS, permissions, cancellation, or resource limits.

## Imports and naming

The namespace and direct-import forms refer to the same symbols:

```zirk
import { Net } from std.net;
import { TCPStream, SocketAddress } from std.net;

mut first = Net.TCPStream.connect(
    Net.SocketAddress("example.com", 443),
);
mut second = TCPStream.connect(SocketAddress("example.com", 443));
```

Use `IPv4` and `IPv6` with a lowercase `v`. Other networking acronyms remain
uppercase: `DNS`, `TCP`, `UDP`, and `TLS`. TLS is available through both
`Net.TLS.*` and direct imports from `std.net.tls`.

## Address values

The address family is immutable and safe to share:

| Type | Meaning |
| --- | --- |
| `IPAddress` | An `IPv4Address` or `IPv6Address`; never a host name. |
| `IPv4Address` | A 32-bit IPv4 address in canonical dotted form. |
| `IPv6Address` | A 128-bit IPv6 address, including an optional zone/scope. |
| `HostName` | A validated DNS name represented canonically with IDNA2008. |
| `Port` | An integer from `0` through `65535`. |
| `SocketAddress` | A host name or IP address paired with a port. |
| `NetworkPrefix` | An IP network and prefix length, such as `10.0.0.0/8`. |

Parsing an `IPAddress` never performs DNS. A direct constructor and `parse`
provide equivalent validation styles:

```zirk
mut loopback = IPv4Address("127.0.0.1");

match IPAddress.parse("fe80::1%en0") {
    Value(address) => stdout.println(address);
    Error(error) => stderr.println(error);
}
```

Invalid syntax, an invalid IPv6 scope, or a port outside its range returns
`IPAddressError` or `HostNameError` as appropriate. `SocketAddress` preserves a
host name until connection time so policy and DNS are evaluated at the point of
use. IPv6 formatting uses brackets when combined with a port.

## DNS

`DNS.resolve` uses the operating system resolver by default. DNS-over-TLS and
DNS-over-HTTPS are explicit resolver configurations; they are never silently
selected. The module supports A, AAAA, CNAME, MX, TXT, and SRV records plus
reverse lookup. DNSSEC validation is reported only when the configured resolver
can provide trustworthy validation evidence.

```zirk
match await DNS.resolve("example.com") {
    Value(result) => {
        for address in result.addresses {
            stdout.println(address);
        }
    }
    Error(error) => stderr.println(error);
}
```

Resolution is task-aware and cancellation-safe. Positive and negative answers
may be cached according to their TTL, but both caches are bounded. A cached
answer does not bypass permissions: every resolved address is checked again
immediately before every connect attempt. This prevents a permitted host name
from using DNS rebinding to reach a forbidden local or private address.

`DNS.resolve` returns `Result<DNSResult, DNSError>`. The result preserves the
canonical name, ordered addresses, record metadata, expiration, and available
DNSSEC status. Timeouts, malformed replies, unavailable resolvers, negative
answers, and cancellation remain distinguishable error cases.

## TCP clients

`TCPStream.connect` accepts a `SocketAddress` and returns
`Task<Result<TCPStream, TCPConnectError>>`. For host names, connection racing
follows a Happy Eyeballs v2 strategy so IPv6 and IPv4 can be attempted without
making one family wait for a full timeout of the other.

```zirk
mut endpoint = SocketAddress("example.com", 443);

match await TCPStream.connect(endpoint, timeout: 5s) {
    Value(stream) => {
        using stream {
            match await stream.write_all(request_bytes) {
                Value(_) => stdout.println("request sent");
                Error(error) => stderr.println(error);
            }
        }
    }
    Error(error) => stderr.println(error);
}
```

A `TCPStream` is an ordered byte stream:

- `read(buffer)` returns `ReadResult<Bytes>` with `Value`, `End`, or `Error`;
- `write(bytes)` may succeed after writing only part of the input and reports
  the written byte count;
- `write_all(bytes)` applies backpressure and completes only after all bytes
  have been accepted or a typed error occurs;
- `shutdown(Read)`, `shutdown(Write)`, and `shutdown(Both)` perform explicit
  half-close or full-close operations;
- `close()` is idempotent and is also performed by `using`.

Options such as connect timeout, keepalive, no-delay, and bounded read/write
buffers are typed. Platform defaults may differ, so portable programs that rely
on a particular behavior set it explicitly. Cancellation never reports bytes as
unwritten if the runtime has already committed them to the operating system.

## TCP servers

`TCPListener.bind` validates the listen address and permission before opening a
socket. `accept` suspends its task and returns a stream plus its peer address.

```zirk
match TCPListener.bind(SocketAddress(IPv4Address.any(), 8080)) {
    Value(listener) => {
        using listener {
            match await listener.accept() {
                Value(connection) => {
                    mut stream = connection.stream;
                    mut peer = connection.peer;
                    stdout.println("accepted", peer);
                }
                Error(error) => stderr.println(error);
            }
        }
    }
    Error(error) => stderr.println(error);
}
```

Canceling `accept` removes only that waiter unless its owning scope also closes
the listener. Accept queues and concurrent handlers are bounded by explicit
server configuration. The runtime uses its reactor; it does not create one
thread per listener or connection.

## UDP datagrams

UDP preserves message boundaries. `receive_from` returns the payload and source
address together; it never silently truncates a datagram. If a configured
buffer cannot hold a packet, the result identifies the required size or returns
a typed size-limit error according to the selected receive policy.

```zirk
match UDPSocket.bind(SocketAddress(IPv4Address.any(), 9000)) {
    Value(socket) => {
        using socket {
            match await socket.receive_from(max_bytes: 64KiB) {
                Value(datagram) => {
                    await socket.send_to(datagram.bytes, datagram.source);
                }
                Error(error) => stderr.println(error);
            }
        }
    }
    Error(error) => stderr.println(error);
}
```

An unconnected socket uses `send_to` and `receive_from`; a connected UDP socket
uses `send` and `receive` while retaining datagram semantics. Broadcast and
multicast participation require explicit options and matching permissions.
Datagram size, queue depth, and multicast membership are bounded resources.

## TLS

TLS wraps an existing or newly connected TCP stream. `TLSConfiguration` is the
long-form configuration type. The secure client defaults are TLS 1.3, system
trust roots, certificate-chain and host-name validation, SNI, safe cipher
suites, and bounded handshakes. TLS 1.2 is available only as explicit
interoperability configuration; older protocol versions are forbidden.

```zirk
import { TLS, TLSConfiguration } from std.net.tls;

mut configuration = TLSConfiguration.client();

match await TLS.connect(
    SocketAddress("example.com", 443),
    server_name: HostName("example.com"),
    configuration: configuration,
) {
    Value(stream) => {
        using stream {
            stdout.println(stream.negotiated_protocol);
        }
    }
    Error(error) => stderr.println(error);
}
```

Custom trust stores, local test certificate authorities, mutual TLS, ALPN, and
session resumption are typed configuration capabilities. Certificate validation
cannot be disabled by a general-purpose `insecure` switch. Tests that need a
private authority install that authority explicitly. Secret keys use secret/key
types and remain redacted. TLS key logging is confined to a conspicuous,
permission-checked dangerous-debug facility and is never enabled in ordinary
builds.

The TLS stream preserves the underlying backpressure, cancellation, and
resource-ownership rules. A failed or canceled handshake closes resources it
created; wrapping a caller-owned stream follows the explicit ownership option.

## Local sockets and interfaces

`LocalSocket` provides task-aware inter-process byte streams using Unix-domain
sockets where available and the platform-equivalent local transport elsewhere.
Names and paths are validated, cleanup ownership is explicit, and a stale socket
is never removed unless the caller selected and is authorized for that policy.

`NetworkInterfaces.list()` returns a snapshot of interface names, flags,
addresses, prefix lengths, and scope information. It does not continuously
watch system configuration. Interface inspection may require a platform or
permission-specific capability when it reveals sensitive topology.

Raw sockets, packet capture, Ethernet frames, DHCP, BGP, VPN construction, Tor,
and general kernel-network administration are not part of `std.net`. QUIC is an
official package rather than a core module so its release cadence can follow the
protocol ecosystem independently.

## Permissions

Network authority distinguishes outbound and inbound scope:

```zirk
permissions {
    network {
        connect {
            origins: ["https://example.com:443"];
        }

        listen {
            addresses: ["127.0.0.1:8080"];
        }
    }
}
```

`connect.origins` authorizes protocols, host names, and ports. It does not by
itself authorize an arbitrary resolved address: resolved IPs, redirects,
reconnects, proxy targets, and TLS server names are validated against the
effective grant at every boundary. `listen.addresses` separately authorizes
local bind addresses and ports. UDP broadcast/multicast, local sockets, custom
resolvers, and interface inspection use explicit scopes rather than inheriting
broad outbound authority.

Libraries declare required scopes with `requires`; only the application grants
them. Missing authority produces `NetworkPermissionError` through the operation
result before the external effect occurs. Runtime library code never prompts or
edits `init.zrk`; trusted tooling owns consent.

## Errors, limits, and portability

The error families are `IPAddressError`, `HostNameError`, `DNSError`,
`TCPError`, `UDPError`, `TLSError`, `LocalSocketError`, and
`NetworkPermissionError`. They implement `NetworkError` while preserving their
specific operation, endpoint, stable category, platform cause when safe, and a
redacted diagnostic message. `TCPConnectError` is the connect-specific TCP
variant. Programs may handle the precise type or the shared contract.

All operations enforce configurable safe defaults for DNS response size,
resolution time, connection attempts, accept queues, stream buffers, datagram
size/queues, TLS handshakes, and concurrent resources. Exhaustion returns a
typed limit error and applies backpressure where progress remains possible; it
never switches to unbounded allocation.

Exact operating-system error numbers, interface capabilities, local-socket
representation, and socket-option availability are platform-specific. The Zirk
error category and safety behavior are portable. Unsupported options return an
explicit error rather than being ignored.

---

**Previous:** [← std.parallel](10a-std-parallel.md) · **Next:** [std.http →](12-std-http.md)
