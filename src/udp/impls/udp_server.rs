use std::io::Cursor;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;
use log::{debug, info};
use tokio::net::UdpSocket;
use crate::stats::enums::stats_event::StatsEvent;
use crate::tracker::enums::torrent_peers_type::TorrentPeersType;
use crate::tracker::structs::announce_query_request::AnnounceQueryRequest;
use crate::tracker::structs::info_hash::InfoHash;
use crate::tracker::structs::peer_id::PeerId;
use crate::tracker::structs::torrent_tracker::TorrentTracker;
use crate::tracker::structs::user_id::UserId;
use crate::udp::enums::request::Request;
use crate::udp::enums::response::Response;
use crate::udp::enums::server_error::ServerError;
use crate::udp::structs::announce_interval::AnnounceInterval;
use crate::udp::structs::announce_request::AnnounceRequest;
use crate::udp::structs::announce_response::AnnounceResponse;
use crate::udp::structs::announce_response_raw::AnnounceResponseRaw;
use crate::udp::structs::connect_request::ConnectRequest;
use crate::udp::structs::connect_response::ConnectResponse;
use crate::udp::structs::connection_id::ConnectionId;
use crate::udp::structs::error_response::ErrorResponse;
use crate::udp::structs::number_of_downloads::NumberOfDownloads;
use crate::udp::structs::number_of_peers::NumberOfPeers;
use crate::udp::structs::port::Port;
use crate::udp::structs::responce_peer_raw::ResponsePeerRaw;
use crate::udp::structs::response_peer::ResponsePeer;
use crate::udp::structs::scrape_request::ScrapeRequest;
use crate::udp::structs::scrape_response::ScrapeResponse;
use crate::udp::structs::torrent_scrape_statistics::TorrentScrapeStatistics;
use crate::udp::structs::transaction_id::TransactionId;
use crate::udp::structs::udp_server::UdpServer;
use crate::udp::udp::{MAX_PACKET_SIZE, MAX_SCRAPE_TORRENTS};

impl UdpServer {
    pub async fn new(tracker: Arc<TorrentTracker>, bind_address: SocketAddr, threads: u64) -> tokio::io::Result<UdpServer>
    {
        let socket = UdpSocket::bind(bind_address).await?;

        Ok(UdpServer {
            socket: Arc::new(socket),
            threads,
            tracker,
        })
    }

    pub async fn start(&self, rx: tokio::sync::watch::Receiver<bool>)
    {
        let threads = self.threads;
        for _index in 0..=threads {
            let socket_clone = self.socket.clone();
            let tracker = self.tracker.clone();
            let mut rx = rx.clone();
            let mut data = [0; 65507];
            tokio::spawn(async move {
                loop {
                    let udp_sock = socket_clone.local_addr().unwrap();
                    tokio::select! {
                        _ = rx.changed() => {
                            info!("Stopping UDP server: {}...", udp_sock);
                            break;
                        }
                        Ok((valid_bytes, remote_addr)) = socket_clone.recv_from(&mut data) => {
                            let payload = data[..valid_bytes].to_vec();

                            debug!("Received {} bytes from {}", payload.len(), remote_addr);
                            debug!("{:?}", payload);

                            let remote_addr_cloned = remote_addr;
                            let payload_cloned = payload.clone();
                            let tracker_cloned = tracker.clone();
                            let socket_cloned = socket_clone.clone();
                            let response = UdpServer::handle_packet(remote_addr_cloned, payload_cloned, tracker_cloned).await;
                            UdpServer::send_response(socket_cloned, remote_addr_cloned, response).await;
                        }
                    }
                }
            });
        }
    }

    pub async fn send_response(socket: Arc<UdpSocket>, remote_addr: SocketAddr, response: Response) {
        debug!("sending response to: {:?}", &remote_addr);

        let buffer = vec![0u8; MAX_PACKET_SIZE];
        let mut cursor = Cursor::new(buffer);

        match response.write(&mut cursor) {
            Ok(_) => {
                let position = cursor.position() as usize;
                let inner = cursor.get_ref();

                debug!("{:?}", &inner[..position]);
                UdpServer::send_packet(socket, &remote_addr, &inner[..position]).await;
            }
            Err(_) => {
                debug!("could not write response to bytes.");
            }
        }
    }

    pub async fn send_packet(socket: Arc<UdpSocket>, remote_addr: &SocketAddr, payload: &[u8]) {
        let _ = socket.send_to(payload, remote_addr).await;
    }

    pub async fn get_connection_id(remote_address: &SocketAddr) -> ConnectionId {
        match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => ConnectionId(((duration.as_secs() / 3600) | ((remote_address.port() as u64) << 36)) as i64),
            Err(_) => ConnectionId(0x7FFFFFFFFFFFFFFF)
        }
    }

    pub async fn handle_packet(remote_addr: SocketAddr, payload: Vec<u8>, tracker: Arc<TorrentTracker>) -> Response {
        match Request::from_bytes(&payload[..payload.len()], MAX_SCRAPE_TORRENTS).map_err(|_| ServerError::InternalServerError) {
            Ok(request) => {
                let transaction_id = match &request {
                    Request::Connect(connect_request) => {
                        connect_request.transaction_id
                    }
                    Request::Announce(announce_request) => {
                        announce_request.transaction_id
                    }
                    Request::Scrape(scrape_request) => {
                        scrape_request.transaction_id
                    }
                };

                match UdpServer::handle_request(request, remote_addr, tracker).await {
                    Ok(response) => response,
                    Err(e) => UdpServer::handle_udp_error(e, transaction_id).await
                }
            }
            Err(_) => UdpServer::handle_udp_error(ServerError::BadRequest, TransactionId(0)).await
        }
    }

    pub async fn handle_request(request: Request, remote_addr: SocketAddr, tracker: Arc<TorrentTracker>) -> Result<Response, ServerError> {
        match request {
            Request::Connect(connect_request) => {
                UdpServer::handle_udp_connect(remote_addr, &connect_request, tracker).await
            }
            Request::Announce(announce_request) => {
                UdpServer::handle_udp_announce_dualstack(remote_addr, &announce_request, tracker).await
            }
            Request::Scrape(scrape_request) => {
                UdpServer::handle_udp_scrape(remote_addr, &scrape_request, tracker).await
            }
        }
    }

    pub async fn handle_udp_connect(remote_addr: SocketAddr, request: &ConnectRequest, tracker: Arc<TorrentTracker>) -> Result<Response, ServerError> {
        let connection_id = UdpServer::get_connection_id(&remote_addr).await;
        let response = Response::from(ConnectResponse {
            transaction_id: request.transaction_id,
            connection_id
        });
        match remote_addr {
            SocketAddr::V4(_) => {
                tracker.update_stats(StatsEvent::Udp4ConnectionsHandled, 1);
            }
            SocketAddr::V6(_) => {
                tracker.update_stats(StatsEvent::Udp6ConnectionsHandled, 1);
            }
        };
        Ok(response)
    }

    pub async fn handle_udp_announce_dualstack(remote_addr: SocketAddr, request: &AnnounceRequest, tracker: Arc<TorrentTracker>) -> Result<Response, ServerError> {
        let (peer, entry) = match tracker.handle_announce(tracker.clone(), AnnounceQueryRequest {
            info_hash: request.info_hash,
            peer_id: request.peer_id,
            port: request.port.0,
            uploaded: request.bytes_uploaded.0 as u64,
            downloaded: request.bytes_downloaded.0 as u64,
            left: request.bytes_downloaded.0 as u64,
            compact: false,
            no_peer_id: false,
            event: request.event,
            remote_addr: remote_addr.ip(),
            numwant: request.peers_wanted.0 as u64,
        }, None).await {
            Ok(result) => result,
            Err(_) => return Err(ServerError::InternalServerError),
        };

        let torrent_peers = tracker.get_torrent_peers_id(request.info_hash, 72, TorrentPeersType::All, peer.peer_id);
        let mut peers: Vec<ResponsePeerRaw> = Vec::new();
        let mut count = 0;

        let supports_v4 = peer.peer_addr_v4.is_some();
        let supports_v6 = peer.peer_addr_v6.is_some();

        // If there is still data left to be dowloaded
        if request.bytes_left.0 as u64 != 0 {
            match torrent_peers {
                None => {},
                Some(ref torrent_peers_unwrapped) => {
                    let mut all_seeds = torrent_peers_unwrapped.seeds_ipv4.clone();
                    all_seeds.append(&mut torrent_peers_unwrapped.seeds_ipv6.clone());

                    for (_,peer) in all_seeds.iter() {
                        if count > 72 {
                            break;
                        }

                        if supports_v4 {
                            match peer.peer_addr_v4 {
                                Some(v4) =>  {
                                    peers.push(ResponsePeer {
                                        ip_address: v4.ip().to_string().parse::<Ipv4Addr>().unwrap(),
                                        port: Port(v4.port()),
                                    }.into());

                                    count += 1;
                                },
                                None => {},
                            }
                        }

                        if supports_v6 {
                            match peer.peer_addr_v6 {
                                Some(v6) => {
                                    peers.push(ResponsePeer {
                                        ip_address: v6.ip().to_string().parse::<Ipv6Addr>().unwrap(),
                                        port: Port(v6.port()),
                                    }.into());

                                    count += 1;
                                },
                                None => {},
                            }
                        }
                    }
                },
            }
        } else {
            // Add to seeds
            match torrent_peers {
                None => {},
                Some(ref torrent_peers_unwrapped) => {
                    let mut all_peers = torrent_peers_unwrapped.peers_ipv4.clone();
                    all_peers.append(&mut torrent_peers_unwrapped.peers_ipv6.clone());

                    for (_,peer) in all_peers.iter() {
                        if count > 72 {
                            break;
                        }

                        if supports_v4 {
                            match peer.peer_addr_v4 {
                                Some(v4) =>  {
                                    peers.push(ResponsePeer {
                                        ip_address: v4.ip().to_string().parse::<Ipv4Addr>().unwrap(),
                                        port: Port(v4.port()),
                                    }.into());

                                    count += 1;
                                },
                                None => {},
                            }
                        }

                        if supports_v6 {
                            match peer.peer_addr_v6 {
                                Some(v6) => {
                                    peers.push(ResponsePeer {
                                        ip_address: v6.ip().to_string().parse::<Ipv6Addr>().unwrap(),
                                        port: Port(v6.port()),
                                    }.into());

                                    count += 1;
                                },
                                None => {},
                            }
                        }
                    }
                },
            }
        }

        // let combined_peers : Vec<ResponsePeer<I>> = peers4.append(&mut peers6);

        let announce_response = Response::from(AnnounceResponseRaw {
            transaction_id: request.transaction_id,
            announce_interval: AnnounceInterval(tracker.config.interval.unwrap() as i32),
            leechers: NumberOfPeers(entry.peers.len() as i32),
            seeders: NumberOfPeers(entry.seeds.len() as i32),
            peers
        });


        if remote_addr.is_ipv4() {
            tracker.update_stats(StatsEvent::Udp4AnnouncesHandled, 1);
        } else {
            tracker.update_stats(StatsEvent::Udp6AnnouncesHandled, 1);
        }

        Ok(announce_response)
    }

    pub async fn handle_udp_announce(remote_addr: SocketAddr, request: &AnnounceRequest, tracker: Arc<TorrentTracker>) -> Result<Response, ServerError> {
        if tracker.config.whitelist && !tracker.check_whitelist(InfoHash(request.info_hash.0)).await {
            debug!("[UDP ERROR] Torrent Not Whitelisted");
            return Err(ServerError::TorrentNotWhitelisted);
        }
        if tracker.config.blacklist && tracker.check_blacklist(InfoHash(request.info_hash.0)).await {
            debug!("[UDP ERROR] Torrent Blacklisted");
            return Err(ServerError::TorrentBlacklisted);
        }
        if tracker.config.keys {
            if request.path.len() < 50 {
                debug!("[UDP ERROR] Unknown Key");
                return Err(ServerError::UnknownKey);
            }
            let key_path_extract = &request.path[10..50];
            match hex::decode(key_path_extract) {
                Ok(result) => {
                    let key = <[u8; 20]>::try_from(result[0..20].as_ref()).unwrap();
                    if !tracker.check_key(InfoHash::from(key)).await {
                        debug!("[UDP ERROR] Unknown Key");
                        return Err(ServerError::UnknownKey);
                    }
                }
                Err(_) => {
                    debug!("[UDP ERROR] Unknown Key");
                    return Err(ServerError::UnknownKey);
                }
            }
        }
        let mut user_key: Option<UserId> = None;
        if tracker.config.users {
            let user_key_path_extract: &str = if tracker.config.keys {
                if request.path.len() < 91 {
                    debug!("[UDP ERROR] Peer Key Not Valid");
                    return Err(ServerError::PeerKeyNotValid);
                }
                &request.path[51..91]
            } else {
                if request.path.len() < 50 {
                    debug!("[UDP ERROR] Peer Key Not Valid");
                    return Err(ServerError::PeerKeyNotValid);
                }
                &request.path[10..50]
            };
            match hex::decode(user_key_path_extract) {
                Ok(result) => {
                    let key = <[u8; 20]>::try_from(result[0..20].as_ref()).unwrap();
                    if !tracker.check_user_key(UserId::from(key)).await {
                        debug!("[UDP ERROR] Peer Key Not Valid");
                        return Err(ServerError::PeerKeyNotValid);
                    }
                    user_key = Some(UserId::from(key));
                }
                Err(error) => {
                    debug!("[UDP ERROR] Hex Decode Error");
                    debug!("{:#?}", error);
                    return Err(ServerError::PeerKeyNotValid);
                }
            }
        }
        let torrent = match tracker.handle_announce(tracker.clone(), AnnounceQueryRequest {
            info_hash: InfoHash(request.info_hash.0),
            peer_id: PeerId(request.peer_id.0),
            port: request.port.0,
            uploaded: request.bytes_uploaded.0 as u64,
            downloaded: request.bytes_downloaded.0 as u64,
            left: request.bytes_left.0 as u64,
            compact: false,
            no_peer_id: false,
            event: request.event,
            remote_addr: remote_addr.ip(),
            numwant: request.peers_wanted.0 as u64,
        }, user_key).await {
            Ok(result) => { result.1 }
            Err(error) => {
                debug!("[UDP ERROR] Handle Announce - Internal Server Error");
                debug!("{:#?}", error);
                return Err(ServerError::InternalServerError);
            }
        };
        let torrent_peers = tracker.get_torrent_peers(request.info_hash, 72, TorrentPeersType::All, Some(remote_addr.ip()));
        let mut peers: Vec<ResponsePeer<Ipv4Addr>> = Vec::new();
        let mut peers6: Vec<ResponsePeer<Ipv6Addr>> = Vec::new();
        let mut count = 0;
        if request.bytes_left.0 as u64 != 0 {
            if remote_addr.is_ipv4()  {
                match torrent_peers {
                    None => {}
                    Some(ref torrent_peers_unwrapped) => {
                        for (_, torrent_peer) in torrent_peers_unwrapped.seeds_ipv4.iter() {
                            if count > 72 {
                                break;
                            }
                            peers.push(ResponsePeer::<Ipv4Addr> {
                                ip_address: torrent_peer.peer_addr_v4.unwrap().ip().to_string().parse::<Ipv4Addr>().unwrap(),
                                port: Port(torrent_peer.peer_addr_v4.unwrap().port()),
                            });
                            count += 1;
                        }
                    }
                }
            } else {
                match torrent_peers {
                    None => {}
                    Some(ref torrent_peers_unwrapped) => {
                        for (_, torrent_peer) in torrent_peers_unwrapped.seeds_ipv6.iter() {
                            if count > 72 {
                                break;
                            }
                            peers6.push(ResponsePeer::<Ipv6Addr> {
                                ip_address: torrent_peer.peer_addr_v6.unwrap().ip().to_string().parse::<Ipv6Addr>().unwrap(),
                                port: Port(torrent_peer.peer_addr_v6.unwrap().port()),
                            });
                            count += 1;
                        }
                    }
                }
            }
        }
        if remote_addr.is_ipv4() {
            match torrent_peers {
                None => {}
                Some(ref torrent_peers_unwrapped) => {
                    for (_, torrent_peer) in torrent_peers_unwrapped.peers_ipv4.iter() {
                        if count > 72 {
                            break;
                        }
                        peers.push(ResponsePeer::<Ipv4Addr> {
                            ip_address: torrent_peer.peer_addr_v4.unwrap().ip().to_string().parse::<Ipv4Addr>().unwrap(),
                            port: Port(torrent_peer.peer_addr_v4.unwrap().port()),
                        });
                        count += 1;
                    }
                }
            }
        } else {
            match torrent_peers {
                None => {}
                Some(ref torrent_peers_unwrapped) => {
                    for (_, torrent_peer) in torrent_peers_unwrapped.peers_ipv6.iter() {
                        if count > 72 {
                            break;
                        }
                        peers6.push(ResponsePeer::<Ipv6Addr> {
                            ip_address: torrent_peer.peer_addr_v6.unwrap().ip().to_string().parse::<Ipv6Addr>().unwrap(),
                            port: Port(torrent_peer.peer_addr_v6.unwrap().port()),
                        });
                        count += 1;
                    }
                }
            }
        }

        let mut announce_response = Response::from(AnnounceResponse {
            transaction_id: request.transaction_id,
            announce_interval: AnnounceInterval(tracker.config.interval.unwrap() as i32),
            leechers: NumberOfPeers(torrent.peers.len() as i32),
            seeders: NumberOfPeers(torrent.seeds.len() as i32),
            peers,
        });
        if remote_addr.is_ipv6() {
            announce_response = Response::from(AnnounceResponse {
                transaction_id: request.transaction_id,
                announce_interval: AnnounceInterval(tracker.config.interval.unwrap() as i32),
                leechers: NumberOfPeers(torrent.peers.len() as i32),
                seeders: NumberOfPeers(torrent.seeds.len() as i32),
                peers: peers6
            });
        }
        if remote_addr.is_ipv4() {
            tracker.update_stats(StatsEvent::Udp4AnnouncesHandled, 1);
        } else {
            tracker.update_stats(StatsEvent::Udp6AnnouncesHandled, 1);
        }
        Ok(announce_response)
    }

    pub async fn handle_udp_scrape(remote_addr: SocketAddr, request: &ScrapeRequest, tracker: Arc<TorrentTracker>) -> Result<Response, ServerError> {
        let mut torrent_stats: Vec<TorrentScrapeStatistics> = Vec::new();
        for info_hash in request.info_hashes.iter() {
            let info_hash = InfoHash(info_hash.0);
            let scrape_entry = match tracker.get_torrent(InfoHash(info_hash.0)) {
                None => {
                    TorrentScrapeStatistics {
                        seeders: NumberOfPeers(0),
                        completed: NumberOfDownloads(0),
                        leechers: NumberOfPeers(0),
                    }
                }
                Some(torrent_info) => {
                    TorrentScrapeStatistics {
                        seeders: NumberOfPeers(torrent_info.seeds.len() as i32),
                        completed: NumberOfDownloads(torrent_info.completed as i32),
                        leechers: NumberOfPeers(torrent_info.peers.len() as i32),
                    }
                }
            };
            torrent_stats.push(scrape_entry);
        }
        if remote_addr.is_ipv4() {
            tracker.update_stats(StatsEvent::Udp4ScrapesHandled, 1);
        } else {
            tracker.update_stats(StatsEvent::Udp6ScrapesHandled, 1);
        }
        Ok(Response::from(ScrapeResponse {
            transaction_id: request.transaction_id,
            torrent_stats,
        }))
    }

    pub async fn handle_udp_error(e: ServerError, transaction_id: TransactionId) -> Response {
        let message = e.to_string();
        Response::from(ErrorResponse { transaction_id, message: message.into() })
    }
}
