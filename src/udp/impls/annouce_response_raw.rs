use std::net::{Ipv4Addr, Ipv6Addr};

use crate::udp::structs::{announce_response::AnnounceResponse, announce_response_raw::AnnounceResponseRaw, responce_peer_raw::ResponsePeerRaw};

impl From<AnnounceResponse<Ipv4Addr>> for AnnounceResponseRaw {
    fn from(value: AnnounceResponse<Ipv4Addr>) -> Self {
        Self {
            transaction_id: value.transaction_id,
            announce_interval: value.announce_interval,
            leechers: value.leechers,
            seeders: value.seeders,
            peers: value.peers.iter().map(|it| ResponsePeerRaw::from(it.clone())).collect(),
        }
    }
}

impl From<AnnounceResponse<Ipv6Addr>> for AnnounceResponseRaw {
    fn from(value: AnnounceResponse<Ipv6Addr>) -> Self {
        Self {
            transaction_id: value.transaction_id,
            announce_interval: value.announce_interval,
            leechers: value.leechers,
            seeders: value.seeders,
            peers: value.peers.iter().map(|it| ResponsePeerRaw::from(it.clone())).collect(),
        }
    }
}