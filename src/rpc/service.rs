use {
  super::RpcEvent,
  crate::{
    primitives::{Message, Pubkey, Subscription},
    rpc::protocol::parse_request,
    storage::PersistentStorage,
  },
  axum::{
    extract::{ws, ws::WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Extension,
    Router,
  },
  axum_extra::response::ErasedJson,
  crossbeam::queue::SegQueue,
  either::Either,
  futures::Stream,
  serde_json::json,
  std::{net::SocketAddr, sync::Arc, task::Poll},
  tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
  tracing::debug,
};

struct ServiceSharedState {
  identity: Pubkey,
  events_out: SegQueue<RpcEvent>,
  message_sender: UnboundedSender<Message>,
  subscription_sender: UnboundedSender<Subscription>,
}

pub struct RpcService {
  shared_state: Arc<ServiceSharedState>,
  message_recv: UnboundedReceiver<Message>,
  subscription_recv: UnboundedReceiver<Subscription>,
}

impl RpcService {
  pub fn new(
    addrs: Vec<SocketAddr>,
    _state: PersistentStorage,
    identity: Pubkey,
  ) -> Self {
    let (message_sender, message_recv) = unbounded_channel();
    let (subscription_sender, subscription_recv) = unbounded_channel();

    let shared_state = Arc::new(ServiceSharedState {
      identity,
      message_sender,
      subscription_sender,
      events_out: SegQueue::new(),
    });

    let svc = Router::new()
      .route("/info", get(serve_info))
      .route("/rpc", get(serve_rpc))
      .layer(Extension(shared_state.clone()));

    addrs.iter().cloned().for_each(|addr| {
      let svc = svc.clone();
      tokio::spawn(async move {
        axum::Server::bind(&addr)
          .serve(svc.into_make_service())
          .await
          .unwrap();
      });
    });

    Self {
      shared_state,
      message_recv,
      subscription_recv,
    }
  }
}

async fn serve_info(
  Extension(state): Extension<Arc<ServiceSharedState>>,
) -> impl IntoResponse {
  debug!("Serving info");
  ErasedJson::pretty(json!({
    "version": env!("VERGEN_GIT_SEMVER"),
    "identity": state.identity,
  }))
}

async fn serve_rpc(
  ws: WebSocketUpgrade,
  Extension(state): Extension<Arc<ServiceSharedState>>,
) -> impl IntoResponse {
  debug!("Got a websocket");
  ws.on_upgrade(|socket| serve_rpc_socket(socket, state))
}

async fn serve_rpc_socket(
  mut socket: WebSocket,
  state: Arc<ServiceSharedState>,
) {
  debug!("Starting a websocket");
  while let Some(msg) = socket.recv().await {
    if let Ok(ws::Message::Text(msg)) = msg {
      if let Ok(request) = parse_request(&msg) {
        match request {
          // some end-party is publishing a new message
          Either::Left(message) => {
            debug!("Received {message:?} through WebSocket API");
            state.events_out.push(RpcEvent::Message(message));
          }

          // ome end-party is establishing a subscription on topic
          // and awaiting incoming messages
          Either::Right(subscription) => {
            debug!("Received {subscription:?} through WebSocket API");
            state
              .events_out
              .push(RpcEvent::Subscription(subscription, socket));
            // subscription created, transfer ownership of the underlying
            // connection socket out of the RPC module into the message bus.
            break;
          }
        }
      }
    }
  }
}

impl Unpin for RpcService {}
impl Stream for RpcService {
  type Item = RpcEvent;

  fn poll_next(
    self: std::pin::Pin<&mut Self>,
    _: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    if let Some(event) = self.shared_state.events_out.pop() {
      return Poll::Ready(Some(event));
    }
    Poll::Pending
  }
}
