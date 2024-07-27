use std::collections::BTreeMap;
use serde::Serialize;
use serde_with::serde_as;
use crate::tracker::structs::peer_id::PeerId;
use crate::tracker::structs::torrent_peer::TorrentPeer;

#[serde_as]
#[derive(Serialize, Clone, Debug)]
pub struct TorrentEntry {
    #[serde_as(as = "BTreeMap<serde_with::hex::Hex, _>")]
    pub seeds: BTreeMap<PeerId, TorrentPeer>,
    #[serde_as(as = "BTreeMap<serde_with::hex::Hex, _>")]
    pub peers: BTreeMap<PeerId, TorrentPeer>,
    pub completed: u64,
    #[serde(with = "serde_millis")]
    pub updated: std::time::Instant
}
