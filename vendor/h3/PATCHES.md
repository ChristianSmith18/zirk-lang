# Local patch record

This directory contains the source of the upstream `h3` crate version 0.0.8,
distributed under the included MIT license. Zirk adds only
`h3::ext::Protocol::WEBSOCKET` and parsing/serialization for the `websocket`
Extended CONNECT token required by RFC 9220. The patch does not alter HTTP/3
framing, QPACK, QUIC behavior, or other upstream protocol handling.

When a maintained upstream release exposes this standard protocol value, remove
the local patch and use that release after the normal dependency review.
