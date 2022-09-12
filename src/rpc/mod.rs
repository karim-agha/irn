mod protocol;
mod service;
pub use service::RpcService;
use {
  crate::primitives::{Message, Subscription},
  axum::extract::ws::WebSocket,
};

pub enum RpcEvent {
  Message(Message),
  Subscription(Subscription, WebSocket),
}
