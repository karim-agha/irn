use {
  super::Addressable,
  multihash::{Multihash, MultihashDigest},
  once_cell::sync::OnceCell,
  serde::{Deserialize, Serialize},
  sha3::{Digest, Sha3_256},
};

/// Represents a single message relayed between two end-parties.
/// The endpoints are either a dApp or a client wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
  sender: Multihash,
  topic: Multihash,
  content: Vec<u8>,
  #[serde(skip)]
  hashcache: OnceCell<Multihash>,
}

/// This is the unique identifier of a message.
///
/// We use hashing contents to identify a message and use that identity for ACKs
/// deduplication and in general referring to a particular message. Users have
/// no way of specifying the id of the message they are sending without
/// modifying its content.
impl Addressable for Message {
  fn multihash(&self) -> Multihash {
    *self.hashcache.get_or_init(|| {
      let mut hasher = Sha3_256::new();
      hasher.update(&self.sender.to_bytes());
      hasher.update(&self.topic.to_bytes());
      hasher.update(&self.content);
      multihash::Code::Sha3_256.wrap(&hasher.finalize()).unwrap()
    })
  }
}