## ADDED Requirements

### Requirement: TLS metadata absence is distinct from TLS failure
Public TLS metadata APIs SHALL NOT use `Result<T?, TLSError>` to conflate a successful absence with a TLS operation failure. They SHALL return a Result whose non-null success payload explicitly represents metadata presence or absence; authentication, transport, and host failures SHALL remain typed `Error(TLSError)` outcomes.

#### Scenario: The TLS peer negotiated an ALPN protocol
- **WHEN** `TLSStream.negotiated_alpn()` has a selected protocol
- **THEN** it returns `Ok(TLSNegotiatedALPN.Negotiated(protocol))`

#### Scenario: The TLS peer selected no ALPN protocol
- **WHEN** the TLS handshake completes without a negotiated ALPN identifier
- **THEN** it returns `Ok(TLSNegotiatedALPN.NotNegotiated)`

#### Scenario: TLS metadata lookup fails
- **WHEN** reading TLS metadata fails because the stream or host operation is invalid
- **THEN** it returns `Error(TLSError)` distinct from absent metadata

#### Scenario: The TLS peer certificate fingerprint is unavailable
- **WHEN** the TLS role has no peer certificate fingerprint metadata
- **THEN** `peer_certificate_sha256()` returns `Ok(TLSPeerCertificateFingerprint.Unavailable)`

#### Scenario: The TLS peer certificate fingerprint is available
- **WHEN** the authenticated peer certificate has a SHA-256 fingerprint
- **THEN** `peer_certificate_sha256()` returns `Ok(TLSPeerCertificateFingerprint.Sha256(fingerprint))`
