# `std.crypto`

`std.crypto` exposes a deliberately small, versioned set of safe cryptographic
operations. It favors authenticated constructions, typed keys, automatic nonce
and salt generation, algorithm migration and audited native acceleration over
low-level configurability.

> **Implementation status:** accepted Zirk 1.x contract. An implementation must
> pass official algorithm test vectors and target/profile validation before the
> API can be reported as available.

## Namespaces and imports

`Crypto` is the canonical namespace. Algorithms may also be imported directly;
both spellings resolve to the same static symbol and behavior:

```zirk
import { Crypto, Ed25519, SHA_256 } from std.crypto;

inmut first = Crypto.SHA_256.hash(data);
inmut second = SHA_256.hash(data);
```

Algorithm names follow Zirk's extended acronym convention:
`SHA_256`, `SHA_3_256`, `AES_256_GCM`, `HMAC_SHA_256`, `ML_KEM_768` and
`ML_DSA_65`.

## Secret and algorithm-bound types

```zirk
SecretBytes
SecretString
SymmetricKey<AES_256_GCM>
PrivateKey<Ed25519>
PublicKey<Ed25519>
Nonce<AES_256_GCM>
Digest<SHA_256>
Signature<Ed25519>
SealedMessage<AES_256_GCM>
```

Secret strings/bytes, symmetric keys and private keys do not implement ordinary
`to_string`, `Clone` or equality. Diagnostics, logs, traces and debugger views
redact them. Runtime storage is cleared when released within the guarantees of
the allocator/target. Deliberate duplication is fallible and explicit.

```zirk
inmut copy = key.duplicate();
inmut public_data = key.export_public(format: KeyFormat.PEM);
inmut protected = key.export_private(
    format: KeyFormat.PEM,
    encryption: password,
);
inmut dangerous = key.export_private_unencrypted(); // Security warning.
```

Private export is encrypted by default. The visibly named unencrypted operation
exists for legitimate interoperability but cannot occur silently. `==` is not a
constant-time promise; secret/digest/tag comparison uses
`ConstantTime.equal(left, right)`.

## Cryptographically secure random

`SecureRandom` uses only the operating system CSPRNG and cannot be seeded:

```zirk
inmut bytes = SecureRandom.bytes(length: 32);
inmut token = SecureRandom.token(length: 32); // Base64URL, no padding.
inmut value = SecureRandom.int(range: 0..100);
```

Integer ranges use rejection sampling to avoid modulo bias. Reproducible random
belongs to non-cryptographic `std.math.Random`. CSPRNG access requires no
project permission; platform failure returns `SecureRandomError`.

## Hashing

The standard catalog is `SHA_256`, `SHA_384`, `SHA_512`, `SHA_3_256`,
`SHA_3_512` and performance-oriented `BLAKE_3`. SHA-2/SHA-3 are the portable
interoperable/FIPS families; BLAKE3 is not presented as FIPS-approved. MD5 and
SHA-1 are absent rather than deprecated conveniences.

```zirk
inmut digest: Digest<SHA_256> = Crypto.SHA_256.hash(data);

mut hasher = Crypto.SHA_256();
hasher.update(header);
hasher.update(body);
inmut streamed = hasher.finish(); // Consumes hasher.
```

Algorithm-bound digest types prevent cross-family confusion. Files are hashed
by feeding bounded `std.fs` chunks; crypto does not acquire filesystem authority.

## Password storage

```zirk
inmut stored = Crypto.Password.hash(secret);

match Crypto.Password.verify(secret, stored) {
    Valid(needs_rehash) => {
        if needs_rehash refresh_hash();
    },
    Invalid => reject_credentials(),
}
```

Argon2id is the standard profile default. The stored representation includes
algorithm, version, random salt and calibrated time/memory/parallelism costs.
Salt generation is automatic; optional pepper is a separately stored
`SecretBytes`. Verification deliberately does not reveal whether password,
format or pepper failed. `needs_rehash` supports transparent parameter upgrades.

The explicit FIPS profile uses PBKDF2-HMAC-SHA256 with current profile-owned
costs; it never replaces Argon2id silently at runtime. Bcrypt/scrypt creation,
legacy verification and migration live outside the initial core. Password
hashing is intentionally expensive CPU work: servers run batches through
`std.parallel` rather than blocking the task reactor.

## KDF and MAC

`HKDF_SHA_256` and `HKDF_SHA_512` implement extract-and-expand derivation. A
required `info` label separates protocol contexts; deliberately empty info must
still be passed explicitly.

```zirk
inmut derived = Crypto.HKDF_SHA_256.derive(
    input_key:,
    salt:,
    info: "zirk.session.key.v1",
    length: 32,
);
```

`HMAC_SHA_256` and `HMAC_SHA_512` provide message authentication. Verification
returns only `Boolean` and uses constant-time tag comparison. KMAC is omitted
from the initial surface to keep the audited core focused.

## Authenticated encryption

Only AEAD constructions are available: `AES_256_GCM` and
`ChaCha20_Poly1305`. There is no ECB, configurable raw AES mode or unauthenticated
encryption.

```zirk
inmut sealed: SealedMessage<AES_256_GCM> = Crypto.AES_256_GCM.seal(
    plaintext:,
    key:,
    associated_data: metadata,
);

match Crypto.AES_256_GCM.open(
    sealed,
    key:,
    associated_data: metadata,
) {
    Ok(plaintext) => consume(plaintext),
    Error(AuthenticationFailed) => reject(),
}
```

`seal` generates a unique nonce and returns algorithm/version, nonce,
ciphertext and authentication tag together. Associated data defaults to empty
when omitted. Advanced nonce control accepts only `UniqueNonce<A>`, produced by
an approved generator/counter; raw bytes cannot impersonate a nonce. Open
returns one indistinguishable authentication failure for wrong key, tag,
associated data or modified ciphertext.

Large streams use framed authenticated encryption. Every frame binds version,
sequence and stream identity; no frame plaintext is released before its tag is
verified. Binary envelope encoding is standard; JSON/Base64 use explicit codecs.

## Signing and key establishment

```zirk
inmut keys = Crypto.Ed25519.generate_key_pair();

inmut signature = Crypto.Ed25519.sign(
    private_key: keys.private_key,
    context: "zirk.package.signature.v1",
    message:,
);

inmut valid = Crypto.Ed25519.verify(
    public_key: keys.public_key,
    context: "zirk.package.signature.v1",
    message:,
    signature:,
);
```

Ed25519 is the classical default; `ECDSA_P256` supports FIPS/interoperability.
`RSA_PSS` is explicit legacy interoperability, never the default or raw RSA.
`X25519` establishes classical shared secrets.

Zirk 1.x also exposes finalized post-quantum `ML_KEM_768`, `ML_DSA_65`, and
advanced `SLH_DSA`. Protocols such as TLS own classical/post-quantum hybrid
composition; application code does not concatenate primitive outputs by hand.
Generic signing requires a context/domain label. Verification returns
`Boolean` without an oracle-like failure breakdown.

## Key formats and stores

PKCS#8 encodes private keys, SPKI encodes public keys and PEM provides textual
wrapping. JWK is supplied through JSON codecs. Import validates algorithm,
size, structure and parameters before constructing a typed key. Raw forms exist
only where the algorithm standard explicitly defines them.

Platform Keychain/Credential Manager/Secret Service, HSM and remote KMS access
belong to a separate official `KeyStore` abstraction because they add platform,
network and permission effects. X.509 parsing and trust validation belong to
TLS/net rather than everyday crypto primitives.

## Profiles, errors and implementation guarantees

```zirk
crypto {
    profile: CryptoProfile.Standard;
}
```

```zirk
crypto {
    profile: CryptoProfile.FIPS;
}
```

The build rejects a disallowed primitive; profiles never downgrade or substitute
silently. Stored envelopes/hashes identify algorithm and version, and parsing
applies an allowlist without accepting an arbitrary externally supplied
algorithm name.

Errors remain family-specific—`SecureRandomError`, `HashError`, `PasswordError`,
`KDFError`, `MACError`, `AEADError`, `SignatureError`, and `KeyError`—while
implementing common `CryptoError`. Pure CPU crypto and CSPRNG need no permission;
filesystem, KeyStore, HSM or network operations carry their own authority.

Hardware acceleration and vetted native providers are transparent only when
results, side-channel guarantees, target packaging and version pinning remain
equivalent. Official test vectors run during build/CI; runtime self-tests are
required only by profiles such as FIPS. Unsupported targets fail clearly rather
than falling back to an unaudited implementation.

## Deliberate exclusions

Raw RSA, arbitrary curves, free-form Diffie-Hellman parameters, manually
configured AES modes, unauthenticated encryption, untyped nonces, MD5/SHA-1,
algorithm-by-untrusted-name dispatch and user-composed hybrid schemes are not
part of `std.crypto`.

Normative algorithm references include [RFC 9106 for Argon2](https://www.rfc-editor.org/rfc/rfc9106.html),
[RFC 5869 for HKDF](https://www.rfc-editor.org/info/rfc5869/),
[RFC 8439 for ChaCha20-Poly1305](https://www.rfc-editor.org/info/rfc8439/), and
[NIST's finalized post-quantum standards](https://csrc.nist.gov/Projects/Post-Quantum-Cryptography).

---

**Previous:** [← std.json](13-std-json.md) · **Next:** [ std.testing](15-std-testing.md)
