mod protocol;
mod service;

pub use service::RpcService;

use crate::primitives::{Message, Subscription};

pub enum RpcEvent {
  Message(Message),
  Subscription(Subscription),
}
