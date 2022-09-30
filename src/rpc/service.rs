use {
  super::RpcEvent,
  crate::{
    primitives::Pubkey,
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
  either::Either,
  futures::Stream,
  serde_json::json,
  std::{net::SocketAddr, sync::Arc, task::Poll},
  tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
  tracing::{debug, info, warn},
};

struct ServiceSharedState {
  identity: Pubkey,
  events_sender: UnboundedSender<RpcEvent>,
  events_out: UnboundedReceiver<RpcEvent>,
}

pub struct RpcService {
  shared_state: Arc<ServiceSharedState>,
}

impl RpcService {
  pub fn new(
    addrs: Vec<SocketAddr>,
    _state: PersistentStorage,
    identity: Pubkey,
  ) -> Self {
    let (events_sender, events_out) = unbounded_channel();

    let shared_state = Arc::new(ServiceSharedState {
      identity,
      events_out,
      events_sender,
    });

    let svc = Router::new()
      .route("/info", get(serve_info))
      .route("/rpc", get(serve_rpc))
      .layer(Extension(Arc::clone(&shared_state)));

    addrs.iter().cloned().for_each(|addr| {
      let svc = svc.clone();
      tokio::spawn(async move {
        axum::Server::bind(&addr)
          .serve(svc.into_make_service())
          .await
          .unwrap();
      });
    });

    Self { shared_state }
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
      match parse_request(&msg) {
        Ok(request) => match request {
          // some end-party is publishing a new message
          Either::Left(message) => {
            debug!("Received {message:?} through WebSocket API");
            state
              .events_sender
              .send(RpcEvent::Message(message))
              .unwrap();
          }

          // ome end-party is establishing a subscription on topic
          // and awaiting incoming messages
          Either::Right(subscription) => {
            debug!("Received {subscription:?} through WebSocket API");
            state
              .events_sender
              .send(RpcEvent::_Subscription(subscription, socket))
              .unwrap();
            // subscription created, transfer ownership of the underlying
            // connection socket out of the RPC module into the message bus.
            break;
          }
        },
        Err(e) => warn!("Invalid request: {e:?}"),
      }
    } else {
      warn!("Invalid message format: {msg:?}");
    }
  }
}

impl Unpin for RpcService {}
impl Stream for RpcService {
  type Item = RpcEvent;

  fn poll_next(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    if let Some(shared_state) = Arc::get_mut(&mut self.shared_state) {
      if let Poll::Ready(Some(event)) = shared_state.events_out.poll_recv(cx) {
        info!("popping an event from RPC Service: {event:?}");
        return Poll::Ready(Some(event));
      } else {
        warn!("Nothing in RPC events queue");
      }
    } else {
      warn!("Can't get mut arc");
    }
    Poll::Pending
  }
}
