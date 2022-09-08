use multihash::Multihash;

mod keys;
mod message;
mod subscription;

pub use {keys::*, message::Message, subscription::Subscription};

pub trait Addressable {
  fn multihash(&self) -> Multihash;
}
