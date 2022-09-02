use {crate::storage::PersistentStorage, std::net::SocketAddr};

pub struct RpcService {}

impl RpcService {
  pub fn new(addrs: Vec<SocketAddr>, state: PersistentStorage) -> Self {
    Self {}
  }
}
