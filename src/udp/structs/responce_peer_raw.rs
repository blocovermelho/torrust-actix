use crate::udp::structs::port::Port;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ResponsePeerRaw {
    pub ip_address: Vec<u8>,
    pub port: Port,
}
