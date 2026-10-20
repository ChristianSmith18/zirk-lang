//! Bounded, immutable snapshots of host network interface metadata.
//!
//! Callers must authorize interface inspection before invoking
//! [`snapshot_network_interfaces`]. The snapshot is metadata only and does
//! not grant bind, connect, or interface-selection authority.

use std::net::IpAddr;

use crate::network_transport::NetworkTransportError;
use zirk_runtime_core::network_policy::EffectiveNetworkGrant;

pub const MAX_NETWORK_INTERFACE_RECORDS: usize = 256;
pub const MAX_NETWORK_INTERFACE_NAME_BYTES: usize = 256;

/// One address assigned to an interface at snapshot time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInterfaceAddress {
    pub name: String,
    pub index: Option<u32>,
    pub operational_status: NetworkInterfaceStatus,
    pub is_loopback: bool,
    pub is_point_to_point: bool,
    pub address: IpAddr,
    pub prefix_length: u8,
    /// The interface index for IPv6 link-local addresses, where a zone is
    /// required to disambiguate the address. `None` for all other addresses.
    pub scope_id: Option<u32>,
}

/// Normalized operational state from RFC 2863.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkInterfaceStatus {
    Up,
    Down,
    Testing,
    Unknown,
    Dormant,
    NotPresent,
    LowerLayerDown,
}

impl NetworkInterfaceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Testing => "testing",
            Self::Unknown => "unknown",
            Self::Dormant => "dormant",
            Self::NotPresent => "not_present",
            Self::LowerLayerDown => "lower_layer_down",
        }
    }
}

/// Reads a stable, bounded snapshot. Any record that cannot be represented
/// within the public bounds fails the whole operation instead of returning a
/// silently truncated or partial view.
pub(crate) fn snapshot_network_interfaces(
    grant: &EffectiveNetworkGrant,
) -> Result<Vec<NetworkInterfaceAddress>, NetworkTransportError> {
    snapshot_network_interfaces_with(grant, if_addrs::get_if_addrs)
}

fn snapshot_network_interfaces_with(
    grant: &EffectiveNetworkGrant,
    enumerate: impl FnOnce() -> std::io::Result<Vec<if_addrs::Interface>>,
) -> Result<Vec<NetworkInterfaceAddress>, NetworkTransportError> {
    grant
        .authorize_interface_inspection()
        .map_err(NetworkTransportError::from)?;
    let interfaces = enumerate().map_err(|_| NetworkTransportError::Io)?;
    if interfaces.len() > MAX_NETWORK_INTERFACE_RECORDS {
        return Err(NetworkTransportError::BufferLimitExceeded);
    }

    let mut snapshot = Vec::with_capacity(interfaces.len());
    for interface in interfaces {
        if interface.name.is_empty()
            || interface.name.len() > MAX_NETWORK_INTERFACE_NAME_BYTES
            || interface.name.contains('\0')
        {
            return Err(NetworkTransportError::BufferLimitExceeded);
        }

        let status = match interface.oper_status {
            if_addrs::IfOperStatus::Up => NetworkInterfaceStatus::Up,
            if_addrs::IfOperStatus::Down => NetworkInterfaceStatus::Down,
            if_addrs::IfOperStatus::Testing => NetworkInterfaceStatus::Testing,
            if_addrs::IfOperStatus::Unknown => NetworkInterfaceStatus::Unknown,
            if_addrs::IfOperStatus::Dormant => NetworkInterfaceStatus::Dormant,
            if_addrs::IfOperStatus::NotPresent => NetworkInterfaceStatus::NotPresent,
            if_addrs::IfOperStatus::LowerLayerDown => NetworkInterfaceStatus::LowerLayerDown,
        };
        let address = interface.addr.ip();
        let prefix_length = match &interface.addr {
            if_addrs::IfAddr::V4(address) => address.prefixlen,
            if_addrs::IfAddr::V6(address) => address.prefixlen,
        };
        let scope_id = match address {
            IpAddr::V6(address) if address.is_unicast_link_local() => interface.index,
            _ => None,
        };
        let is_loopback = interface.is_loopback();
        let is_point_to_point = interface.is_p2p;
        snapshot.push(NetworkInterfaceAddress {
            name: interface.name,
            index: interface.index,
            operational_status: status,
            is_loopback,
            is_point_to_point,
            address,
            prefix_length,
            scope_id,
        });
    }

    snapshot.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.address.cmp(&right.address))
            .then_with(|| left.prefix_length.cmp(&right.prefix_length))
            .then_with(|| left.index.cmp(&right.index))
    });
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(inspect_interfaces: bool) -> EffectiveNetworkGrant {
        EffectiveNetworkGrant::from_verified_network_scopes(
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
            inspect_interfaces,
        )
        .unwrap()
    }

    #[test]
    fn host_snapshot_is_bounded_normalized_and_deterministically_sorted() {
        let snapshot = snapshot_network_interfaces(&grant(true)).expect("host interface snapshot");
        assert!(!snapshot.is_empty());
        assert!(snapshot.len() <= MAX_NETWORK_INTERFACE_RECORDS);
        assert!(snapshot.iter().all(|record| {
            !record.name.is_empty()
                && record.name.len() <= MAX_NETWORK_INTERFACE_NAME_BYTES
                && record.name.as_bytes().iter().all(|byte| *byte != 0)
                && record.prefix_length
                    <= if record.address.is_ipv4() { 32 } else { 128 }
                && (record.scope_id.is_none()
                    || (matches!(record.address, IpAddr::V6(address) if address.is_unicast_link_local())
                        && record.scope_id == record.index))
        }));
        assert!(snapshot.windows(2).all(|pair| {
            pair[0]
                .name
                .cmp(&pair[1].name)
                .then_with(|| pair[0].address.cmp(&pair[1].address))
                .then_with(|| pair[0].prefix_length.cmp(&pair[1].prefix_length))
                .then_with(|| pair[0].index.cmp(&pair[1].index))
                .is_le()
        }));
        assert!(
            snapshot
                .iter()
                .all(|record| !record.operational_status.as_str().is_empty())
        );
    }

    #[test]
    fn missing_inspection_authority_denies_before_interface_enumeration() {
        let mut enumerated = false;
        let result = snapshot_network_interfaces_with(&grant(false), || {
            enumerated = true;
            Err(std::io::Error::other("enumeration must not run"))
        });
        assert_eq!(result, Err(NetworkTransportError::PermissionDenied));
        assert!(!enumerated);
    }
}
