# HTTP/3 Dependency Review

Review date: 2026-10-21

This review covers the optional native HTTP/3 feature in `zirk-runtime`. The
feature is opt-in through Cargo's `http3` feature and is not part of the
default runtime dependency set.

## Selected components

| Component | Locked version | License | Declared minimum Rust | Responsibility |
| --- | --- | --- | --- | --- |
| `quinn` | 0.11.12 | MIT OR Apache-2.0 | 1.85 | QUIC endpoint, connection, TLS integration, and stream transport |
| `quinn-proto` | 0.11.19 | MIT OR Apache-2.0 | 1.85 | QUIC protocol state machine |
| `quinn-udp` | 0.5.16 | MIT OR Apache-2.0 | 1.85 | Platform UDP socket and packet I/O |
| `h3` | 0.0.8, vendored patch | MIT | 1.70 | HTTP/3 framing, QPACK, and request/response stream API |
| `h3-quinn` | 0.0.10 | MIT | 1.70 | Adapter between `h3` and Quinn |
| `rustls` | 0.23.46 | Apache-2.0 OR ISC OR MIT | 1.71 | TLS 1.3 handshake and certificate validation |
| `ring` | 0.17.14 | Apache-2.0 AND ISC | 1.66 | Cryptographic provider selected by rustls/Quinn |

Zirk's workspace MSRV is Rust 1.94, above the declared MSRV of these selected
crates. The dependency graph was checked with `cargo deny check all` and
`cargo audit`; both passed for the current lockfile (233 registry dependencies
in the audit scan). `cargo deny` reports duplicate crate versions as warnings,
but advisory, ban, license, and source checks pass. The repository's license policy allows
MIT, Apache-2.0, BSD, and ISC. The vendored `h3` copy retains its upstream MIT
license and changes only the registered WebSocket extended-CONNECT protocol
token; see `vendor/h3/PATCHES.md`.

The current `cargo audit` scan found no known advisories in the locked graph.
The reviewed `quinn-proto` 0.11.19 is newer than the upstream patched versions
for the 2026 stream-limit, retry-token, and pending-CID advisories; `rustls`
0.23.46 is newer than the 0.23.45 release that fixed the TLS 1.3
encryption-level acceptance issue. Re-run the audit whenever the lockfile or
the advisory database changes. See the [Quinn release notes](https://github.com/quinn-rs/quinn/releases)
and [rustls release notes](https://github.com/rustls/rustls/releases).

## Maintenance and target evidence

Quinn documents stable Rust support and testing on Linux, macOS, and Windows.
The repository has run its HTTP/3 adapter tests on macOS ARM64, Linux x86_64,
and Linux ARM64. On macOS ARM64, `scripts/test-http3-websocket-public-api.py`
now verifies RFC 9220 WebSocket echo and close in both directions against the
independent Python `aioquic` 1.7.0 implementation. It also verifies Zirk HTTP/3
GET and QUERY client requests against aioquic, including the QUERY body and
content type. The test-only peer and its resolver output are pinned in
`scripts/requirements-http3-interop.txt` and are installed by the native CI
workflow. An unapproved Origin receives `403` without invoking the Zirk
handler. CI run 39739088146 passed the focused HTTP/3 adapter, TLS, UDP, local
IPC, interface, and network-policy runtime commands on Windows x86_64 as well
as macOS ARM64 and both Linux targets. Its complete workspace suites and
independent-peer interoperability step passed on macOS ARM64, Linux x86_64,
and Linux ARM64. Browser Fetch verification also passed. Windows passed the
full workspace and peer steps in run 39702042337 at `634179a`; the current
commit only changes the test harness, and that focused regression test passes
on Windows in run 39748669844. A redundant Windows full-suite rerun in
39739088146 remains active and is not counted as a passing result.

Upstream `h3` describes the crate as experimental and warns that its API may
change. It remains the narrowest fit among the currently integrated engines,
but the experimental status and Zirk's small local patch are release risks.
Recheck upstream releases before each dependency refresh and remove the local
patch only after a published version supplies the WebSocket protocol token
and the full HTTP/3 test suite passes without it.

The independent interoperability peer is test-only: `aioquic` 1.7.0 is BSD
licensed and is not linked into Zirk or shipped in runtime artifacts. The
complete Python resolver set is pinned in
`scripts/requirements-http3-interop.txt`.

## Release-size measurement

On the review host (macOS ARM64), both artifacts were built with the workspace
release profile. The `http3` feature changes the runtime outputs as follows:

| Artifact | Without HTTP/3 | With HTTP/3 | Increase |
| --- | ---: | ---: | ---: |
| `libzirk_runtime.a` | 66,574,080 bytes | 74,736,968 bytes | 8,162,888 bytes (7.78 MiB) |
| `libzirk_runtime.rlib` | 17,386,416 bytes | 21,048,488 bytes | 3,662,072 bytes (3.49 MiB) |

The final linked executable was measured on macOS ARM64 with the workspace
release profile using `scripts/measure-http3-binary-size.sh`. The fixture at
`scripts/fixtures/http3-size/main.zrk` builds an application that calls the
public `HTTPClient` API with HTTP/3 selected. Both variants use the same source
and compiler; only the runtime archive's `http3` feature changes.

| Representative application | Without HTTP/3 | With HTTP/3 | Increase |
| --- | ---: | ---: | ---: |
| Compiled Zirk executable | 6,592,104 bytes (6.29 MiB) | 7,781,288 bytes (7.42 MiB) | 1,189,184 bytes (1.13 MiB, 18.0%) |

These are raw executable sizes before packaging or compression, freshly
measured on 2026-10-21 after the current implementation changes. The result is
specific to this small HTTP client fixture and macOS ARM64; other applications
may retain a different portion of the optional protocol engine after linking.

## Decision and remaining evidence

Keep HTTP/3 optional and keep 0-RTT disabled. The license and advisory checks,
workspace MSRV, release-size measurement, and successful HTTP/3-enabled
conformance builds on all four native targets support this dependency choice
for continued development. Independent-peer interoperability passes on macOS
ARM64, Linux x86_64, Linux ARM64, and Windows across runs 39739088146 and
39702042337; see the test-only revision qualification above.
Reassess the dependency choice if upstream `h3` remains experimental or its
patch cannot be retired before release.

## Primary sources

- [Quinn repository and platform notes](https://github.com/quinn-rs/quinn)
- [Quinn releases and security fixes](https://github.com/quinn-rs/quinn/releases)
- [Hyperium h3 repository, status, and license](https://github.com/hyperium/h3)
- [Hyperium h3 changelog](https://github.com/hyperium/h3/blob/master/changelog-h3.md)
- [aioquic repository, HTTP/3/RFC 9220 support, and BSD license](https://github.com/aiortc/aioquic)
- [aioquic 1.7.0 published artifacts](https://pypi.org/project/aioquic/1.7.0/)
- [RustSec advisory database](https://github.com/RustSec/advisory-db)
