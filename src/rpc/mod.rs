mod protocol;
mod service;
pub use service::RpcService;
use {
  crate::primitives::{Message, Subscription},
  axum::extract::ws::WebSocket,
};

#[derive(Debug)]
pub enum RpcEvent {
  Message(Message),
  Subscription(Subscription, WebSocket),
}
