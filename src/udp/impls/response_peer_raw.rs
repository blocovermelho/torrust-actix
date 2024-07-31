use std::net::{Ipv4Addr, Ipv6Addr};

use crate::udp::structs::{responce_peer_raw::ResponsePeerRaw, response_peer::ResponsePeer};

impl From<ResponsePeer<Ipv4Addr>> for ResponsePeerRaw {
    fn from(value: ResponsePeer<Ipv4Addr>) -> Self {
        Self {
            ip_address: value.ip_address.octets().to_vec(),
            port: value.port,
        }
    }
}

impl From<ResponsePeer<Ipv6Addr>> for ResponsePeerRaw {
    fn from(value: ResponsePeer<Ipv6Addr>) -> Self {
        Self {
            ip_address: value.ip_address.octets().to_vec(),
            port: value.port,
        }
    }
}