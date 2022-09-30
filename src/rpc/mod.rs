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
  _Subscription(Subscription, WebSocket),
}

macro_rules! handle {
  ($event:ident, $bus: ident, $network: ident) => {
    match $event {
      RpcEvent::Message(msg) => {
        info!("rpc-event message: {msg:?}");
        $network.gossip_message(msg.clone())?;
        // bus.send_message(msg).await?;
      }
      RpcEvent::_Subscription(sub, socket) => {
        info!("rpc-event subscription: {sub:?}");
        $bus.create_subscription(sub, socket);
      }
    }
  };
}

pub(crate) use handle;
