use {
  multihash::Multihash,
  serde::{Deserialize, Serialize},
};

/// Represents a single message relayed between two end-parties.
/// The endpoints are either a dApp or a client wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  sender: Multihash,
  topic: Multihash,
  content: Vec<u8>,
}
