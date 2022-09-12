use {
  crate::primitives::{Addressable, Message},
  axum::extract::ws::WebSocket,
  crossbeam::queue::SegQueue,
  dashmap::DashMap,
  futures::Stream,
  multihash::Multihash,
  std::{fmt::Display, task::Poll},
  thiserror::Error,
};

pub enum MessageBusEvent {
  MessageDelivered(Multihash),
  SubscriptionCreated(Multihash),
  SubscriptionDropped(Multihash),
}

#[derive(Error, Debug)]
pub enum SendError {
  Serialization(#[from] serde_json::Error),
  WebSocket(#[from] axum::Error),
}

impl Display for SendError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}

pub struct MessageBus {
  topics: DashMap<Multihash, WebSocket>,
  events_out: SegQueue<MessageBusEvent>,
}

impl MessageBus {
  pub fn new() -> Self {
    Self {
      topics: DashMap::new(),
      events_out: SegQueue::new(),
    }
  }

  pub fn create_subscription(&self, topic: Multihash, socket: WebSocket) {
    self.topics.insert(topic, socket);
    self
      .events_out
      .push(MessageBusEvent::SubscriptionCreated(topic))
  }

  /// Occurs when the WebSocket connection through RPC is closed
  /// for whatever reason.
  pub fn drop_subscription(&self, topic: Multihash) {
    self.topics.remove(&topic);
    self
      .events_out
      .push(MessageBusEvent::SubscriptionDropped(topic));
  }

  /// Called whenever a message is gossiped through P2P and reaches the bus.
  /// If the targeted topic is maintained by this node, it will be immediately
  /// delivered to the subscriber, otherwise it will be placed in temporary
  /// storage until either it expires or a subscription with the target topic
  /// is created.
  pub async fn send_message(&self, message: Message) -> Result<(), SendError> {
    if let Some(mut socket) = self.topics.get_mut(&message.topic) {
      // the message is sent to a subscription managed by this node.
      socket
        .send(axum::extract::ws::Message::Text(
          serde_json::to_string_pretty(&message)?,
        ))
        .await?;

      // inform the rest of the system that this message was successfully
      // delivered
      self
        .events_out
        .push(MessageBusEvent::MessageDelivered(message.multihash()));
    } else {
      // todo: implement persisting a message for some time (TTL)
    }

    Ok(())
  }
}

impl Unpin for MessageBus {}
impl Stream for MessageBus {
  type Item = MessageBusEvent;

  fn poll_next(
    self: std::pin::Pin<&mut Self>,
    _: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    if let Some(event) = self.events_out.pop() {
      return Poll::Ready(Some(event));
    }
    Poll::Pending
  }
}