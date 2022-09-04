use {
  crate::{
    keys::Pubkey,
    message::Message,
    storage::PersistentStorage,
    subscription::Subscription,
  },
  axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Extension,
    Router,
  },
  axum_extra::response::ErasedJson,
  futures::Stream,
  serde_json::json,
  std::{net::SocketAddr, sync::Arc, task::Poll},
  tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
  tracing::debug,
};

struct ServiceSharedState {
  identity: Pubkey,
  message_sender: UnboundedSender<Message>,
  subscription_sender: UnboundedSender<Subscription>,
}

pub struct RpcService {
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
    });

    let svc = Router::new()
      .route("/info", get(serve_info))
      .route("/rpc", get(serve_rpc))
      .layer(Extension(shared_state));

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
      message_recv,
      subscription_recv,
    }
  }
}

async fn serve_info(
  Extension(state): Extension<Arc<ServiceSharedState>>,
) -> impl IntoResponse {
  println!("serving info");
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
  _state: Arc<ServiceSharedState>,
) {
  debug!("Starting a websocket");
  while let Some(msg) = socket.recv().await {
    debug!("ws-msg: {msg:?}");
  }
}

impl Unpin for RpcService {}
impl Stream for RpcService {
  type Item = ();

  fn poll_next(
    self: std::pin::Pin<&mut Self>,
    _: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    Poll::Pending
  }
}
