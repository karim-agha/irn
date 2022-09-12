use {
  super::Addressable,
  multihash::{Multihash, MultihashDigest},
  once_cell::sync::OnceCell,
  serde::{Deserialize, Serialize},
  sha3::{Digest, Sha3_256},
  std::fmt::Debug,
};

/// Represents a single message relayed between two end-parties.
/// The endpoints are either a dApp or a client wallet.
#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
  pub topic: Multihash,
  pub content: Vec<u8>,
  #[serde(skip)]
  hashcache: OnceCell<Multihash>,
}

impl Message {
  pub fn new(topic: Multihash, content: Vec<u8>) -> Self {
    Self {
      topic,
      content,
      hashcache: OnceCell::new(),
    }
  }
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
      hasher.update(&self.topic.to_bytes());
      hasher.update(&self.content);
      multihash::Code::Sha3_256.wrap(&hasher.finalize()).unwrap()
    })
  }
}

impl Debug for Message {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Message")
      .field("topic", &self.topic)
      .field("content", &self.content)
      .field("hash", &self.multihash())
      .finish()
  }
}
