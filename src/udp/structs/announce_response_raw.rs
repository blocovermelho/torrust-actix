
use crate::udp::structs::announce_interval::AnnounceInterval;
use crate::udp::structs::number_of_peers::NumberOfPeers;
use crate::udp::structs::transaction_id::TransactionId;

use super::responce_peer_raw::ResponsePeerRaw;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct AnnounceResponseRaw {
    pub transaction_id: TransactionId,
    pub announce_interval: AnnounceInterval,
    pub leechers: NumberOfPeers,
    pub seeders: NumberOfPeers,
    pub peers: Vec<ResponsePeerRaw>,
}
