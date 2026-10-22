//! Laboratory listener and peer scope. Natively every laboratory listener and peer is
//! loopback. In the laboratory container the listeners bind all interfaces on their
//! configured ports and Docker publishes them only on the host loopback, so a client arrives
//! from the private bridge gateway; it is admitted as the loopback client. Advertised
//! addresses (channel, relay) stay the configured loopback endpoints.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::OnceLock;

static CONTAINER_PUBLICATION: OnceLock<bool> = OnceLock::new();

/// Called once at startup by `lab-frontend --container-listen`.
pub fn enable_container_publication() {
    let _ = CONTAINER_PUBLICATION.set(true);
}

fn container() -> bool {
    CONTAINER_PUBLICATION.get().copied().unwrap_or(false)
}

/// Where to bind a configured loopback endpoint.
pub fn listen_addr(configured: SocketAddr) -> SocketAddr {
    listen_addr_in(configured, container())
}

/// Whether a bound listener stays inside the laboratory scope.
pub fn admitted_listener(local: SocketAddr) -> bool {
    local.ip().is_loopback() || (container() && local.ip().is_unspecified())
}

/// The peer the session sees, or `None` when it is outside the laboratory scope.
pub fn effective_peer(peer: IpAddr) -> Option<IpAddr> {
    effective_peer_in(peer, container())
}

fn listen_addr_in(configured: SocketAddr, container: bool) -> SocketAddr {
    if container {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), configured.port())
    } else {
        configured
    }
}

fn effective_peer_in(peer: IpAddr, container: bool) -> Option<IpAddr> {
    let peer = match peer {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(peer, IpAddr::V4),
        v4 => v4,
    };
    if peer.is_loopback() {
        return Some(peer);
    }
    match peer {
        IpAddr::V4(v4) if container && v4.is_private() => Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_scope_admits_only_loopback() {
        let local: SocketAddr = "127.0.0.1:27015".parse().unwrap();
        assert_eq!(listen_addr_in(local, false), local);
        let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert_eq!(effective_peer_in(loopback, false), Some(loopback));
        assert_eq!(
            effective_peer_in("172.18.0.1".parse().unwrap(), false),
            None
        );
    }

    #[test]
    fn container_scope_binds_all_and_maps_the_bridge_gateway_to_loopback() {
        let local: SocketAddr = "127.0.0.1:27005".parse().unwrap();
        assert_eq!(
            listen_addr_in(local, true),
            "0.0.0.0:27005".parse::<SocketAddr>().unwrap()
        );
        let loopback = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(
            effective_peer_in("172.18.0.1".parse().unwrap(), true),
            loopback
        );
        assert_eq!(
            effective_peer_in("192.168.65.1".parse().unwrap(), true),
            loopback
        );
        assert_eq!(
            effective_peer_in("::ffff:10.0.0.2".parse().unwrap(), true),
            loopback
        );
        assert_eq!(effective_peer_in("8.8.8.8".parse().unwrap(), true), None);
        assert_eq!(effective_peer_in("fd00::1".parse().unwrap(), true), None);
    }
}
