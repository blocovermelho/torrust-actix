use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;
use log::debug;
use crate::common::structs::custom_error::CustomError;
use crate::common::structs::number_of_bytes::NumberOfBytes;
use crate::tracker::enums::announce_event::AnnounceEvent;
use crate::tracker::structs::announce_query_request::AnnounceQueryRequest;
use crate::tracker::structs::info_hash::InfoHash;
use crate::tracker::structs::peer_id::PeerId;
use crate::tracker::structs::scrape_query_request::ScrapeQueryRequest;
use crate::tracker::structs::torrent_entry::TorrentEntry;
use crate::tracker::structs::torrent_peer::TorrentPeer;
use crate::tracker::structs::torrent_tracker::TorrentTracker;
use crate::tracker::structs::user_id::UserId;

impl TorrentTracker {
    pub async fn validate_announce(&self, remote_addr: IpAddr, query: HashMap<String, Vec<Vec<u8>>>) -> Result<AnnounceQueryRequest, CustomError>
    {
        // Validate info_hash
        let info_hash: Vec<Vec<u8>> = match query.get("info_hash") {
            None => {
                return Err(CustomError::new("missing info_hash"));
            }
            Some(result) => {
                if result.is_empty() {
                    return Err(CustomError::new("no info_hash given"));
                }
                if let Some(result_array) = result.first() {
                    if result_array.len() != 20 {
                        return Err(CustomError::new("invalid info_hash size"));
                    }
                    result.clone()
                } else {
                    return Err(CustomError::new("no info_hash given"));
                }
            }
        };

        // Validate peer_id
        let peer_id: Vec<Vec<u8>> = match query.get("peer_id") {
            None => {
                return Err(CustomError::new("missing peer_id"));
            }
            Some(result) => {
                if result.is_empty() {
                    return Err(CustomError::new("no peer_id given"));
                }
                if let Some(result_array) = result.first() {
                    if result_array.len() != 20 {
                        return Err(CustomError::new("invalid peer_id size"));
                    }
                    result.clone()
                } else {
                    return Err(CustomError::new("no peer_id given"));
                }
            }
        };

        // Validate port
        let port_integer = match query.get("port") {
            None => {
                return Err(CustomError::new("missing port"));
            }
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let port = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid port"))
                    };
                    match port.parse::<u16>() {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("missing or invalid port"))
                    }
                } else {
                    return Err(CustomError::new("missing port"));
                }
            }
        };

        // Validate uploaded
        let uploaded_integer = match query.get("uploaded") {
            None => {
                return Err(CustomError::new("missing uploaded"));
            }
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let uploaded = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid uploaded"))
                    };
                    match uploaded.parse::<u64>() {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("missing or invalid uploaded"))
                    }
                } else {
                    return Err(CustomError::new("missing uploaded"));
                }
            }
        };

        // Validate downloaded
        let downloaded_integer = match query.get("downloaded") {
            None => {
                return Err(CustomError::new("missing downloaded"));
            }
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let downloaded = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid downloaded"))
                    };
                    match downloaded.parse::<u64>() {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("missing or invalid downloaded"))
                    }
                } else {
                    return Err(CustomError::new("missing downloaded"));
                }
            }
        };

        // Validate left
        let left_integer = match query.get("left") {
            None => {
                return Err(CustomError::new("missing left"));
            }
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let left = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid left"))
                    };
                    match left.parse::<u64>() {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("missing or invalid left"))
                    }
                } else {
                    return Err(CustomError::new("missing left"));
                }
            }
        };

        // Validate compact
        let mut compact_bool = false;
        match query.get("compact") {
            None => {}
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let compact = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid compact"))
                    };
                    let compact_integer = match compact.parse::<u8>() {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("missing or invalid compact"))
                    };
                    if compact_integer == 1 {
                        compact_bool = true;
                    }
                }
            }
        }

        // Validate event
        let mut event_integer: AnnounceEvent = AnnounceEvent::Started;
        match query.get("event") {
            None => {}
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let event = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid event"))
                    };
                    match event.as_str().to_lowercase().as_str() {
                        "started" => {
                            event_integer = AnnounceEvent::Started;
                        }
                        "stopped" => {
                            event_integer = AnnounceEvent::Stopped;
                        }
                        "completed" => {
                            event_integer = AnnounceEvent::Completed;
                        }
                        _ => {
                            event_integer = AnnounceEvent::Started;
                        }
                    }
                } else {
                    event_integer = AnnounceEvent::Started;
                }
            }
        }

        // Validate no_peer_id
        let mut no_peer_id_bool = false;
        match query.get("no_peer_id") {
            None => {}
            Some(_) => {
                no_peer_id_bool = true;
            }
        }

        // Validate numwant
        let mut numwant_integer = self.config.peers_returned.unwrap();
        match query.get("numwant") {
            None => {}
            Some(result) => {
                if let Some(result_array) = result.first() {
                    let numwant = match String::from_utf8(result_array.to_vec()) {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("invalid numwant"))
                    };
                    numwant_integer = match numwant.parse::<u64>() {
                        Ok(v) => v,
                        Err(_) => return Err(CustomError::new("missing or invalid numwant"))
                    };
                    if numwant_integer == 0 || numwant_integer > self.config.peers_returned.unwrap() {
                        numwant_integer = self.config.peers_returned.unwrap();
                    }
                }
            }
        }

        let announce_data = AnnounceQueryRequest {
            info_hash: InfoHash::from(&info_hash[0] as &[u8]),
            peer_id: PeerId::from(&peer_id[0] as &[u8]),
            port: port_integer,
            uploaded: uploaded_integer,
            downloaded: downloaded_integer,
            left: left_integer,
            compact: compact_bool,
            no_peer_id: no_peer_id_bool,
            event: event_integer,
            remote_addr,
            numwant: numwant_integer,
        };

        Ok(announce_data)
    }

    pub async fn handle_announce(&self, data: Arc<TorrentTracker>, announce_query: AnnounceQueryRequest, user_key: Option<UserId>) -> Result<(TorrentPeer, TorrentEntry), CustomError>
    {

        let mut torrent_peer : TorrentPeer  = TorrentPeer {
            peer_id: announce_query.peer_id,
            peer_addr_v4: None,
            peer_addr_v6: None,
            peer_offer_id: None,
            peer_offer: None,
            updated: std::time::Instant::now(),
            uploaded: NumberOfBytes(announce_query.uploaded as i64),
            downloaded: NumberOfBytes(announce_query.downloaded as i64),
            left: NumberOfBytes(announce_query.left as i64),
            event: AnnounceEvent::None,
        };

        if let Some(torrent) = data.get_torrent(announce_query.info_hash) {
            let peer = torrent.peers.get(&announce_query.peer_id);
            let seed = torrent.seeds.get(&announce_query.peer_id);

            if let Some(tp) = peer.or(seed) {
                torrent_peer = tp.clone();
            }
        }

        let addr = SocketAddr::new(announce_query.remote_addr, announce_query.port);
        let ip_type = if addr.is_ipv4() { "4" } else { "6" }; 

        match addr {
            SocketAddr::V4(_) => torrent_peer.peer_addr_v4 = Some(addr.clone()),
            SocketAddr::V6(_) => torrent_peer.peer_addr_v6 = Some(addr.clone()),
        }

        match announce_query.event {
            AnnounceEvent::Started | AnnounceEvent::None => {
                torrent_peer.event = AnnounceEvent::Started;
                debug!("[HANDLE ANNOUNCE::Started] Adding to infohash {} peerid {} with IPv{}", announce_query.info_hash, announce_query.peer_id.to_string(), ip_type);
                debug!("[DEBUG] Calling add_torrent_peer");
                let torrent_entry = data.add_torrent_peer(
                    announce_query.info_hash,
                    announce_query.peer_id,
                    torrent_peer.clone(),
                    false
                );
                if data.config.users && user_key.is_some() {
                    if let Some(mut user) = data.get_user(user_key.unwrap()).await {
                        user.updated = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                        let mut torrents_active = user.torrents_active.clone();
                        torrents_active.insert(announce_query.info_hash, std::time::Instant::now());
                        user.torrents_active = torrents_active;
                        data.add_user(user_key.unwrap(), user).await;
                    }
                }
                Ok((torrent_peer, TorrentEntry {
                    seeds: torrent_entry.1.seeds,
                    peers: torrent_entry.1.peers,
                    completed: torrent_entry.1.completed,
                    updated: torrent_entry.1.updated
                }))
            }
            AnnounceEvent::Stopped => {
                torrent_peer.event = AnnounceEvent::Stopped;
                debug!("[HANDLE ANNOUNCE::Stopped] Removing from infohash {} peerid {}", announce_query.info_hash, announce_query.peer_id.to_string());
                debug!("[DEBUG] Calling remove_torrent_peer");
                let torrent_entry = match data.remove_torrent_peer(
                    announce_query.info_hash,
                    announce_query.peer_id,
                    data.config.persistence
                ) {
                    (Some(_), None) => {
                        TorrentEntry::new()
                    }
                    (Some(_), Some(new_torrent)) => {
                        if data.config.users && user_key.is_some(){
                            if let Some(mut user) = data.get_user(user_key.unwrap()).await {
                                user.uploaded += announce_query.uploaded;
                                user.downloaded += announce_query.downloaded;
                                user.updated = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                                let mut torrents_active = user.torrents_active.clone();
                                torrents_active.remove(&announce_query.info_hash);
                                user.torrents_active = torrents_active;
                                data.add_user(user_key.unwrap(), user.clone()).await;
                                data.add_users_update(user_key.unwrap(), user).await;
                            }
                        }
                        new_torrent
                    }
                    _ => {
                        TorrentEntry::new()
                    }
                };
                Ok((torrent_peer, torrent_entry))
            }
            AnnounceEvent::Completed => {
                torrent_peer.event = AnnounceEvent::Completed;
                debug!("[HANDLE ANNOUNCE::Completed] Adding to infohash {} peerid {} with IPv{}", announce_query.info_hash, announce_query.peer_id.to_string(), ip_type);
                debug!("[DEBUG] Calling add_torrent_peer");
                let torrent_entry = data.add_torrent_peer(
                    announce_query.info_hash,
                    announce_query.peer_id,
                    torrent_peer.clone(),
                    true
                );
                if data.config.users && user_key.is_some(){
                    if let Some(mut user) = data.get_user(user_key.unwrap()).await {
                        user.completed += 1;
                        user.updated = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
                        data.add_user(user_key.unwrap(), user).await;
                    }
                }
                Ok((torrent_peer, torrent_entry.1))
            }
        }
    }

    pub async fn validate_scrape(&self, query: HashMap<String, Vec<Vec<u8>>>) -> Result<ScrapeQueryRequest, CustomError>
    {
        // Validate info_hash
        let mut info_hash: Vec<InfoHash> = Vec::new();
        match query.get("info_hash") {
            None => {
                Err(CustomError::new("missing info_hash"))
            }
            Some(result) => {
                if result.is_empty() {
                    return Err(CustomError::new("no info_hash given"));
                }
                for hash in result.iter() {
                    if hash.len() != 20 {
                        return Err(CustomError::new("an invalid info_hash was given"));
                    }
                    info_hash.push(InfoHash::from(hash as &[u8]));
                }
                let scrape_data = ScrapeQueryRequest {
                    info_hash
                };
                Ok(scrape_data)
            }
        }
    }

    pub async fn handle_scrape(&self, data: Arc<TorrentTracker>, scrape_query: ScrapeQueryRequest) -> BTreeMap<InfoHash, TorrentEntry>
    {
        // We generate the output and return it, even if it's empty...
        let mut return_data = BTreeMap::new();
        for info_hash in scrape_query.info_hash.iter() {
            debug!("[DEBUG] Calling get_torrent");
            match data.get_torrent(*info_hash) {
                None => { return_data.insert(*info_hash, TorrentEntry::new()); }
                Some(result) => {
                    return_data.insert(*info_hash, result);
                }
            }
        }
        return_data
    }
}
