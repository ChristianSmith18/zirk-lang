//! Exact-origin and listener checks shared by native transport backends.

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

const GLOBALLY_REACHABLE_SPECIAL_V6_PREFIXES: [(Ipv6Addr, u32); 7] = [
    (Ipv6Addr::new(0x2001, 1, 0, 0, 0, 0, 0, 1), 128),
    (Ipv6Addr::new(0x2001, 1, 0, 0, 0, 0, 0, 2), 128),
    (Ipv6Addr::new(0x2001, 1, 0, 0, 0, 0, 0, 3), 128),
    (Ipv6Addr::new(0x2001, 3, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2001, 4, 0x112, 0, 0, 0, 0, 0), 48),
    (Ipv6Addr::new(0x2001, 0x20, 0, 0, 0, 0, 0, 0), 28),
    (Ipv6Addr::new(0x2001, 0x30, 0, 0, 0, 0, 0, 0), 28),
];

// Special-purpose IPv6 prefixes with `Globally Reachable = False` stay
// outside the general 2000::/3 path below. This includes translation,
// discard-only, protocol-assignment, documentation, and local-use ranges in
// the IANA registry: https://www.iana.org/assignments/iana-ipv6-special-registry

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkPolicyError {
    InvalidOrigin,
    InvalidEndpoint,
    PermissionDenied,
    AddressNotAllowed,
}

/// A canonical protocol/host/port authority used by `std.net` permissions.
/// HTTP URLs remain path-aware in the HTTP layer; this value deliberately
/// represents only the network origin checked before DNS and connect.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkOrigin {
    scheme: String,
    host: String,
    port: u16,
    canonical: String,
}

impl NetworkOrigin {
    pub fn parse(input: &str) -> Result<Self, NetworkPolicyError> {
        let parsed = url::Url::parse(input).map_err(|_| NetworkPolicyError::InvalidOrigin)?;
        if !matches!(parsed.scheme(), "http" | "https" | "tcp")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
            || parsed.host_str().is_none()
        {
            return Err(NetworkPolicyError::InvalidOrigin);
        }
        let port = parsed
            .port_or_known_default()
            .ok_or(NetworkPolicyError::InvalidOrigin)?;
        if parsed.scheme() == "tcp" && parsed.port().is_none() {
            return Err(NetworkPolicyError::InvalidOrigin);
        }
        let (host, authority_host) = match parsed.host() {
            Some(url::Host::Domain(host)) => {
                let host = host.to_ascii_lowercase();
                (host.clone(), host)
            }
            Some(url::Host::Ipv4(host)) => {
                let host = host.to_string();
                (host.clone(), host)
            }
            Some(url::Host::Ipv6(host)) => {
                let host = host.to_string();
                (host.clone(), format!("[{host}]"))
            }
            None => return Err(NetworkPolicyError::InvalidOrigin),
        };
        let canonical = format!("{}://{}:{}", parsed.scheme(), authority_host, port);
        if input != canonical {
            return Err(NetworkPolicyError::InvalidOrigin);
        }
        Ok(Self {
            scheme: parsed.scheme().to_owned(),
            host,
            port,
            canonical,
        })
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    pub fn host(&self) -> &str {
        &self.host
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn as_str(&self) -> &str {
        &self.canonical
    }
}

/// Builds a canonical TCP origin from a host and a non-zero port without
/// resolving DNS. Domain names are normalized through the URL crate's IDNA
/// processing; IP literals are formatted canonically, with IPv6 bracketed.
pub fn normalize_tcp_origin(host: &str, port: u16) -> Result<String, NetworkPolicyError> {
    if host.is_empty() || port == 0 {
        return Err(NetworkPolicyError::InvalidOrigin);
    }
    let authority_host = if let Ok(address) = host.parse::<std::net::Ipv6Addr>() {
        format!("[{address}]")
    } else {
        match url::Host::parse(host).map_err(|_| NetworkPolicyError::InvalidOrigin)? {
            url::Host::Domain(domain) => {
                if domain.len() > 253
                    || domain.split('.').any(|label| {
                        label.is_empty()
                            || label.len() > 63
                            || label.starts_with('-')
                            || label.ends_with('-')
                            || !label
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    })
                {
                    return Err(NetworkPolicyError::InvalidOrigin);
                }
                domain
            }
            url::Host::Ipv4(address) => address.to_string(),
            url::Host::Ipv6(address) => format!("[{address}]"),
        }
    };
    let canonical = format!("tcp://{authority_host}:{port}");
    NetworkOrigin::parse(&canonical)?;
    Ok(canonical)
}

/// A host-verified network grant. Manifest declarations alone must never be
/// used to construct this value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveNetworkGrant {
    connect_origins: BTreeSet<String>,
    listen_addresses: BTreeSet<SocketAddr>,
    udp_destinations: BTreeSet<SocketAddr>,
    udp_listen_addresses: BTreeSet<SocketAddr>,
    udp_broadcast_destinations: BTreeSet<SocketAddr>,
    udp_multicast_groups: BTreeSet<IpAddr>,
    local_connect_endpoints: BTreeSet<String>,
    local_listen_endpoints: BTreeSet<String>,
    inspect_interfaces: bool,
}

impl EffectiveNetworkGrant {
    /// Creates a policy object from scopes that have already passed approval
    /// verification in the trusted host.
    pub fn from_verified_scopes(
        connect_origins: impl IntoIterator<Item = String>,
        listen_addresses: impl IntoIterator<Item = String>,
    ) -> Result<Self, NetworkPolicyError> {
        Self::from_verified_network_scopes(
            connect_origins,
            listen_addresses,
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            false,
        )
    }

    /// Builds the full transport authority from scopes that have already
    /// passed signed approval verification in the trusted host.
    pub fn from_verified_network_scopes(
        connect_origins: impl IntoIterator<Item = String>,
        listen_addresses: impl IntoIterator<Item = String>,
        udp_destinations: impl IntoIterator<Item = String>,
        udp_listen_addresses: impl IntoIterator<Item = String>,
        local_connect_endpoints: impl IntoIterator<Item = String>,
        local_listen_endpoints: impl IntoIterator<Item = String>,
        inspect_interfaces: bool,
    ) -> Result<Self, NetworkPolicyError> {
        Self::from_verified_datagram_scopes(
            connect_origins,
            listen_addresses,
            udp_destinations,
            udp_listen_addresses,
            std::iter::empty(),
            std::iter::empty(),
            local_connect_endpoints,
            local_listen_endpoints,
            inspect_interfaces,
        )
    }

    /// Builds the complete UDP authority, including separately approved
    /// limited-broadcast destinations and multicast groups.
    pub fn from_verified_datagram_scopes(
        connect_origins: impl IntoIterator<Item = String>,
        listen_addresses: impl IntoIterator<Item = String>,
        udp_destinations: impl IntoIterator<Item = String>,
        udp_listen_addresses: impl IntoIterator<Item = String>,
        udp_broadcast_destinations: impl IntoIterator<Item = String>,
        udp_multicast_groups: impl IntoIterator<Item = String>,
        local_connect_endpoints: impl IntoIterator<Item = String>,
        local_listen_endpoints: impl IntoIterator<Item = String>,
        inspect_interfaces: bool,
    ) -> Result<Self, NetworkPolicyError> {
        let connect_origins = connect_origins
            .into_iter()
            .map(|origin| {
                NetworkOrigin::parse(&origin)?;
                Ok(origin)
            })
            .collect::<Result<_, _>>()?;
        let listen_addresses = listen_addresses
            .into_iter()
            .map(|address| {
                address
                    .parse()
                    .map_err(|_| NetworkPolicyError::AddressNotAllowed)
            })
            .collect::<Result<_, _>>()?;
        let udp_destinations = udp_destinations
            .into_iter()
            .map(|destination| {
                let value = destination
                    .strip_prefix("udp://")
                    .ok_or(NetworkPolicyError::AddressNotAllowed)?;
                let address = value
                    .parse::<SocketAddr>()
                    .map_err(|_| NetworkPolicyError::AddressNotAllowed)?;
                if address.port() == 0 || destination != format!("udp://{address}") {
                    return Err(NetworkPolicyError::AddressNotAllowed);
                }
                Ok(address)
            })
            .collect::<Result<_, _>>()?;
        let udp_listen_addresses = udp_listen_addresses
            .into_iter()
            .map(|address| {
                let parsed: SocketAddr = address
                    .parse()
                    .map_err(|_| NetworkPolicyError::AddressNotAllowed)?;
                if parsed.port() == 0 || address != parsed.to_string() {
                    return Err(NetworkPolicyError::AddressNotAllowed);
                }
                Ok(parsed)
            })
            .collect::<Result<_, _>>()?;
        let udp_broadcast_destinations = udp_broadcast_destinations
            .into_iter()
            .map(|destination| {
                let value = destination
                    .strip_prefix("udp://")
                    .ok_or(NetworkPolicyError::AddressNotAllowed)?;
                let address = value
                    .parse::<SocketAddr>()
                    .map_err(|_| NetworkPolicyError::AddressNotAllowed)?;
                if address.port() == 0
                    || !matches!(address.ip(), IpAddr::V4(ip) if ip.is_broadcast())
                    || destination != format!("udp://{address}")
                {
                    return Err(NetworkPolicyError::AddressNotAllowed);
                }
                Ok(address)
            })
            .collect::<Result<_, _>>()?;
        let udp_multicast_groups = udp_multicast_groups
            .into_iter()
            .map(|group| {
                let address = group
                    .parse::<IpAddr>()
                    .map_err(|_| NetworkPolicyError::AddressNotAllowed)?;
                if !address.is_multicast() || group != address.to_string() {
                    return Err(NetworkPolicyError::AddressNotAllowed);
                }
                Ok(address)
            })
            .collect::<Result<_, _>>()?;
        let local_connect_endpoints = local_connect_endpoints
            .into_iter()
            .map(|endpoint| {
                validate_local_endpoint(&endpoint)?;
                Ok(endpoint)
            })
            .collect::<Result<_, _>>()?;
        let local_listen_endpoints = local_listen_endpoints
            .into_iter()
            .map(|endpoint| {
                validate_local_endpoint(&endpoint)?;
                Ok(endpoint)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            connect_origins,
            listen_addresses,
            udp_destinations,
            udp_listen_addresses,
            udp_broadcast_destinations,
            udp_multicast_groups,
            local_connect_endpoints,
            local_listen_endpoints,
            inspect_interfaces,
        })
    }

    /// Rechecks origin and every resolved candidate immediately before a
    /// connect attempt. Hostnames may resolve only to globally routable
    /// addresses; an explicitly granted IP-literal origin may target that
    /// exact IP, including loopback for controlled local services.
    pub fn authorize_connect(
        &self,
        origin: &str,
        candidate: SocketAddr,
    ) -> Result<(), NetworkPolicyError> {
        let parsed = NetworkOrigin::parse(origin)?;
        self.authorize_origin(origin)?;
        // The permission origin has no interface/scope identifier. Accepting
        // a scoped IPv6 SocketAddr here would therefore let the resolver add
        // interface-specific routing authority that the user never granted.
        if matches!(candidate, SocketAddr::V6(address) if address.scope_id() != 0) {
            return Err(NetworkPolicyError::AddressNotAllowed);
        }
        if candidate.port() != parsed.port() {
            return Err(NetworkPolicyError::AddressNotAllowed);
        }
        let host = parsed.host().trim_start_matches('[').trim_end_matches(']');
        if let Ok(literal) = host.parse::<IpAddr>() {
            if candidate.ip() != literal {
                return Err(NetworkPolicyError::AddressNotAllowed);
            }
        } else if !is_public_address(candidate.ip()) {
            return Err(NetworkPolicyError::AddressNotAllowed);
        }
        Ok(())
    }

    /// Checks origin authority before the host performs DNS resolution.
    pub fn authorize_origin(&self, origin: &str) -> Result<(), NetworkPolicyError> {
        NetworkOrigin::parse(origin)?;
        self.connect_origins
            .contains(origin)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_listen(&self, address: SocketAddr) -> Result<(), NetworkPolicyError> {
        self.listen_addresses
            .contains(&address)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_udp_send(&self, destination: SocketAddr) -> Result<(), NetworkPolicyError> {
        self.udp_destinations
            .contains(&destination)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_udp_bind(&self, address: SocketAddr) -> Result<(), NetworkPolicyError> {
        self.udp_listen_addresses
            .contains(&address)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_udp_broadcast(
        &self,
        destination: SocketAddr,
    ) -> Result<(), NetworkPolicyError> {
        self.udp_broadcast_destinations
            .contains(&destination)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_udp_multicast(&self, group: IpAddr) -> Result<(), NetworkPolicyError> {
        self.udp_multicast_groups
            .contains(&group)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_local_connect(&self, endpoint: &str) -> Result<(), NetworkPolicyError> {
        validate_local_endpoint(endpoint)?;
        self.local_connect_endpoints
            .contains(endpoint)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_local_listen(&self, endpoint: &str) -> Result<(), NetworkPolicyError> {
        validate_local_endpoint(endpoint)?;
        self.local_listen_endpoints
            .contains(endpoint)
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }

    pub fn authorize_interface_inspection(&self) -> Result<(), NetworkPolicyError> {
        self.inspect_interfaces
            .then_some(())
            .ok_or(NetworkPolicyError::PermissionDenied)
    }
}

/// Validates the canonical platform endpoint forms accepted in `.zkinit`.
pub fn validate_local_endpoint(value: &str) -> Result<(), NetworkPolicyError> {
    let valid = if let Some(path) = value.strip_prefix("unix://") {
        path.starts_with('/')
            && path.len() > 1
            && !path.ends_with('/')
            && !path.contains('%')
            && path[1..]
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
    } else if let Some(name) = value.strip_prefix("npipe://./pipe/") {
        !name.is_empty()
            && name != "."
            && name != ".."
            && name.len() <= 240
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    } else {
        false
    };
    valid
        .then_some(())
        .ok_or(NetworkPolicyError::InvalidEndpoint)
}

fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

fn in_v4_prefix(ip: Ipv4Addr, network: [u8; 4], prefix: u8) -> bool {
    let ip = u32::from(ip);
    let network = u32::from(Ipv4Addr::from(network));
    let mask = u32::MAX << (32 - prefix);
    ip & mask == network & mask
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    if ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_unspecified()
        || ip.is_broadcast()
    {
        return false;
    }
    // IANA marks these two anycast assignments inside 192.0.0.0/24 as
    // globally reachable; the rest of the protocol-assignment block remains
    // non-global. See the IPv4 Special-Purpose Address Registry.
    if ip == Ipv4Addr::new(192, 0, 0, 9) || ip == Ipv4Addr::new(192, 0, 0, 10) {
        return true;
    }
    ![
        ([0, 0, 0, 0], 8),
        ([100, 64, 0, 0], 10),
        ([192, 0, 0, 0], 24),
        ([192, 0, 2, 0], 24),
        ([192, 88, 99, 0], 24),
        ([198, 18, 0, 0], 15),
        ([198, 51, 100, 0], 24),
        ([203, 0, 113, 0], 24),
        ([240, 0, 0, 0], 4),
    ]
    .into_iter()
    .any(|(network, prefix)| in_v4_prefix(ip, network, prefix))
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_v4(mapped);
    }
    let value = u128::from(ip);
    let in_prefix = |network: u128, prefix: u32| value & (u128::MAX << (128 - prefix)) == network;
    // These allocations are explicitly marked globally reachable by IANA
    // inside the otherwise blocked 2001::/23 protocol-assignment space.
    if GLOBALLY_REACHABLE_SPECIAL_V6_PREFIXES
        .iter()
        .any(|&(network, prefix)| in_prefix(u128::from(network), prefix))
    {
        return true;
    }
    // DNS hostnames may reach only ordinary global unicast addresses. This
    // excludes translation, transition, protocol-assignment, and documentation
    // ranges that can route to a different address family or have local scope.
    value & (u128::MAX << 125)
        == u128::from(Ipv6Addr::from([
            0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))
        && !in_prefix(
            u128::from(Ipv6Addr::from([
                0x20, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])),
            23,
        )
        && !in_prefix(
            u128::from(Ipv6Addr::from([
                0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])),
            32,
        )
        && !in_prefix(
            u128::from(Ipv6Addr::from([
                0x20, 0x01, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])),
            48,
        )
        && !in_prefix(
            u128::from(Ipv6Addr::from([
                0x20, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])),
            16,
        )
        && !in_prefix(
            u128::from(Ipv6Addr::from([
                0x3f, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])),
            20,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> EffectiveNetworkGrant {
        EffectiveNetworkGrant::from_verified_scopes(
            [
                "https://api.example.com:443".into(),
                "http://127.0.0.1:8080".into(),
                "tcp://127.0.0.1:9000".into(),
            ],
            ["127.0.0.1:8080".into()],
        )
        .unwrap()
    }

    #[test]
    fn tcp_origin_is_canonical_and_port_pinned_for_std_net() {
        let grant = grant();
        assert_eq!(
            grant.authorize_connect("tcp://127.0.0.1:9000", "127.0.0.1:9000".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_connect("tcp://127.0.0.1:9000", "127.0.0.1:9001".parse().unwrap()),
            Err(NetworkPolicyError::AddressNotAllowed)
        );
        for invalid in [
            "tcp://127.0.0.1",
            "tcp://127.0.0.1:9000/path",
            "TCP://127.0.0.1:9000",
            "tcp://user@127.0.0.1:9000",
            "tcp://[fe80::1%25en0]:9000",
        ] {
            assert_eq!(
                NetworkOrigin::parse(invalid),
                Err(NetworkPolicyError::InvalidOrigin),
                "{invalid} must not become an authority scope"
            );
        }
    }

    #[test]
    fn host_port_origin_normalizes_idna_and_ip_literals_without_dns() {
        assert_eq!(
            normalize_tcp_origin("Exämple.com", 443).unwrap(),
            "tcp://xn--exmple-cua.com:443"
        );
        assert_eq!(
            normalize_tcp_origin("2001:0db8::1", 443).unwrap(),
            "tcp://[2001:db8::1]:443"
        );
        assert_eq!(
            normalize_tcp_origin("127.0.0.1", 8080).unwrap(),
            "tcp://127.0.0.1:8080"
        );
        for (host, port) in [
            ("", 80),
            ("example.com", 0),
            ("user@example.com", 80),
            ("example.com/path", 80),
            ("bad..example", 80),
            ("-bad.example", 80),
            ("bad_name.example", 80),
            ("fe80::1%en0", 80),
            ("fe80::1%25en0", 80),
        ] {
            assert_eq!(
                normalize_tcp_origin(host, port),
                Err(NetworkPolicyError::InvalidOrigin),
                "accepted {host:?}:{port}"
            );
        }
    }

    #[test]
    fn connection_requires_an_exact_canonical_origin_and_port() {
        let grant = grant();
        assert_eq!(
            grant.authorize_connect(
                "https://api.example.com:443",
                "93.184.216.34:443".parse().unwrap()
            ),
            Ok(())
        );
        assert_eq!(
            grant.authorize_connect(
                "https://other.example:443",
                "93.184.216.34:443".parse().unwrap()
            ),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(
            grant.authorize_connect(
                "https://api.example.com:443",
                "93.184.216.34:444".parse().unwrap()
            ),
            Err(NetworkPolicyError::AddressNotAllowed)
        );
    }

    #[test]
    fn connection_rejects_ipv6_interface_scope_not_represented_by_grant() {
        let origin = "tcp://[fe80::1]:9000";
        let grant = EffectiveNetworkGrant::from_verified_scopes([origin.into()], []).unwrap();
        let address = SocketAddr::V6(std::net::SocketAddrV6::new(
            "fe80::1".parse().unwrap(),
            9000,
            0,
            7,
        ));

        assert_eq!(
            grant.authorize_connect(origin, address),
            Err(NetworkPolicyError::AddressNotAllowed)
        );
    }

    #[test]
    fn hostname_authority_rejects_private_addresses_and_ip_literals_are_pinned() {
        let grant = grant();
        assert_eq!(
            grant.authorize_connect(
                "https://api.example.com:443",
                "127.0.0.1:443".parse().unwrap()
            ),
            Err(NetworkPolicyError::AddressNotAllowed)
        );
        assert_eq!(
            grant.authorize_connect("http://127.0.0.1:8080", "127.0.0.1:8080".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_connect("http://127.0.0.1:8080", "127.0.0.2:8080".parse().unwrap()),
            Err(NetworkPolicyError::AddressNotAllowed)
        );
    }

    #[test]
    fn hostname_authority_applies_ipv4_policy_to_ipv4_mapped_ipv6_answers() {
        let grant = grant();
        for address in [
            "[::ffff:127.0.0.1]:443",
            "[::ffff:10.1.2.3]:443",
            "[::ffff:169.254.1.1]:443",
            "[::ffff:192.0.2.1]:443",
        ] {
            assert_eq!(
                grant.authorize_connect("https://api.example.com:443", address.parse().unwrap()),
                Err(NetworkPolicyError::AddressNotAllowed),
                "mapped non-global IPv4 address {address} must remain blocked"
            );
        }

        for address in ["[::ffff:8.8.8.8]:443", "[::ffff:192.0.0.9]:443"] {
            assert_eq!(
                grant.authorize_connect("https://api.example.com:443", address.parse().unwrap()),
                Ok(()),
                "mapped globally reachable IPv4 address {address} follows the IPv4 policy"
            );
        }
    }

    #[test]
    fn hostname_authority_rejects_non_global_translated_and_transition_addresses() {
        let grant = grant();
        for address in [
            "0.1.2.3:443",
            "240.1.2.3:443",
            "[64:ff9b::7f00:1]:443",
            "[64:ff9b:1::1]:443",
            "[100::1]:443",
            "[100:0:0:1::1]:443",
            "[2002:7f00:1::1]:443",
            "[2001:db8::1]:443",
            "[2001:2::1]:443",
            "[2001::1]:443",
            "[2001:10::1]:443",
            "[2001:1::4]:443",
            "[3fff::1]:443",
            "[5f00::1]:443",
            "[fc00::1]:443",
            "[fe80::1]:443",
            "[ff02::1]:443",
        ] {
            assert_eq!(
                grant.authorize_connect("https://api.example.com:443", address.parse().unwrap()),
                Err(NetworkPolicyError::AddressNotAllowed),
                "accepted {address}"
            );
        }
        for address in [
            "[2606:4700:4700::1111]:443",
            "[2001:1::1]:443",
            "[2001:1::2]:443",
            "[2001:1::3]:443",
            "[2001:3::1]:443",
            "[2001:3:ffff:ffff::1]:443",
            "[2001:4:112::1]:443",
            "[2001:4:112:ffff::1]:443",
            "[2001:20::1]:443",
            "[2001:2f:ffff:ffff::1]:443",
            "[2001:30::1]:443",
            "[2001:3f:ffff:ffff::1]:443",
            "[2620:4f:8000::1]:443",
        ] {
            assert_eq!(
                grant.authorize_connect("https://api.example.com:443", address.parse().unwrap()),
                Ok(()),
                "IANA marks {address} globally reachable"
            );
        }
    }

    #[test]
    fn hostname_authority_allows_only_globally_reachable_special_purpose_addresses() {
        let grant = grant();
        for address in [
            "93.184.216.34:443",
            "192.0.0.9:443",
            "192.0.0.10:443",
            "192.31.196.1:443",
            "192.52.193.1:443",
            "192.175.48.1:443",
        ] {
            assert_eq!(
                grant.authorize_connect("https://api.example.com:443", address.parse().unwrap()),
                Ok(()),
                "IANA or ordinary global unicast marks {address} globally reachable"
            );
        }
        for address in [
            "0.1.2.3:443",
            "10.1.2.3:443",
            "100.64.0.1:443",
            "127.0.0.1:443",
            "169.254.1.1:443",
            "172.16.0.1:443",
            "192.0.0.0:443",
            "192.0.0.8:443",
            "192.0.0.170:443",
            "192.0.0.171:443",
            "192.0.2.1:443",
            "192.88.99.1:443",
            "192.168.1.1:443",
            "198.18.0.1:443",
            "198.51.100.1:443",
            "203.0.113.1:443",
            "240.0.0.1:443",
            "255.255.255.255:443",
        ] {
            assert_eq!(
                grant.authorize_connect("https://api.example.com:443", address.parse().unwrap()),
                Err(NetworkPolicyError::AddressNotAllowed),
                "non-global or special-purpose address {address} must remain blocked"
            );
        }
    }

    #[test]
    fn listeners_have_separate_exact_authority() {
        assert_eq!(
            grant().authorize_listen("127.0.0.1:8080".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant().authorize_listen("0.0.0.0:8080".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
    }

    #[test]
    fn datagram_local_and_interface_authority_are_independent() {
        let grant = EffectiveNetworkGrant::from_verified_network_scopes(
            std::iter::empty(),
            std::iter::empty(),
            ["udp://192.0.2.9:5353".into()],
            ["127.0.0.1:5353".into()],
            ["unix:///tmp/zirk.sock".into()],
            ["unix:///tmp/zirk-listener.sock".into()],
            true,
        )
        .unwrap();

        assert_eq!(
            grant.authorize_udp_send("192.0.2.9:5353".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_udp_send("192.0.2.10:5353".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(
            grant.authorize_udp_bind("127.0.0.1:5353".parse().unwrap()),
            Ok(())
        );
        assert_eq!(
            grant.authorize_udp_bind("127.0.0.1:5354".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(
            grant.authorize_listen("127.0.0.1:5353".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied),
            "UDP bind authority must not imply TCP/HTTP listen authority"
        );
        assert_eq!(
            grant.authorize_local_connect("unix:///tmp/zirk.sock"),
            Ok(())
        );
        assert_eq!(
            grant.authorize_local_listen("unix:///tmp/zirk.sock"),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(grant.authorize_interface_inspection(), Ok(()));

        let no_inspection = EffectiveNetworkGrant::default();
        assert_eq!(
            no_inspection.authorize_interface_inspection(),
            Err(NetworkPolicyError::PermissionDenied)
        );
    }

    #[test]
    fn interface_inspection_does_not_grant_connect_or_listen() {
        let inspection_only = EffectiveNetworkGrant::from_verified_network_scopes(
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            true,
        )
        .unwrap();

        assert_eq!(inspection_only.authorize_interface_inspection(), Ok(()));
        assert_eq!(
            inspection_only.authorize_origin("tcp://192.0.2.9:443"),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(
            inspection_only.authorize_listen("127.0.0.1:8080".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
    }

    #[test]
    fn broadcast_and_multicast_authority_is_explicit_and_disjoint() {
        let broadcast: SocketAddr = "255.255.255.255:9000".parse().unwrap();
        let group: IpAddr = "239.1.2.3".parse().unwrap();
        let grant = EffectiveNetworkGrant::from_verified_datagram_scopes(
            std::iter::empty(),
            std::iter::empty(),
            [format!("udp://{broadcast}")],
            std::iter::empty(),
            [format!("udp://{broadcast}")],
            [group.to_string()],
            std::iter::empty(),
            std::iter::empty(),
            false,
        )
        .unwrap();
        assert!(grant.authorize_udp_send(broadcast).is_ok());
        assert!(grant.authorize_udp_broadcast(broadcast).is_ok());
        assert!(grant.authorize_udp_multicast(group).is_ok());
        assert_eq!(
            grant.authorize_udp_broadcast("255.255.255.255:9001".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert_eq!(
            grant.authorize_udp_multicast("239.1.2.4".parse().unwrap()),
            Err(NetworkPolicyError::PermissionDenied)
        );
        assert!(
            EffectiveNetworkGrant::from_verified_datagram_scopes(
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                std::iter::empty(),
                ["udp://192.0.2.1:9000".to_owned()],
                ["192.0.2.1".to_owned()],
                std::iter::empty(),
                std::iter::empty(),
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn local_endpoint_scope_rejects_traversal_and_noncanonical_forms() {
        for endpoint in [
            "unix:///tmp/../etc/passwd",
            "unix:///tmp/%2e%2e/secret",
            "npipe://./pipe/name/child",
        ] {
            assert_eq!(
                validate_local_endpoint(endpoint),
                Err(NetworkPolicyError::InvalidEndpoint)
            );
        }
    }

    #[test]
    fn scope_parser_rejects_noncanonical_origins() {
        assert_eq!(
            EffectiveNetworkGrant::from_verified_scopes(
                ["https://api.example.com/path".into()],
                std::iter::empty()
            ),
            Err(NetworkPolicyError::InvalidOrigin)
        );
    }
}
