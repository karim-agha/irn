use {
  crate::optstream::OptionalStreamExt,
  clap::Parser,
  cli::CliOpts,
  network::{Network, NetworkEvent},
  rpc::RpcService,
  storage::PersistentStorage,
  tracing::{debug, info, Level},
  tracing_subscriber::{filter::filter_fn, prelude::*},
};

mod cli;
mod keys;
mod message;
mod network;
mod optstream;
mod rpc;
mod storage;
mod subscription;

fn print_essentials(opts: &CliOpts) -> anyhow::Result<()> {
  info!("Starting WalletConnect Inter-Relay Network node");
  info!("Version: {}", env!("CARGO_PKG_VERSION"));
  info!("Network Id: {}", opts.network_id);
  info!("Listen addresses: {:?}", opts.listen_multiaddrs());
  info!("Data directory: {}", opts.data_dir()?.display());
  info!("Node identity: {}", opts.identity());
  info!(
    "P2P identity: {}",
    opts.p2p_identity().public().to_peer_id()
  );
  info!("Bootstrap peers: {:?}", opts.peers());
  info!("RPC Endpoints: {:?}", opts.rpc_endpoints());

  Ok(())
}

fn initialize_logging(loglevel: Level) {
  tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer().with_filter(filter_fn(
      move |metadata| {
        !metadata.target().starts_with("netlink")
          && metadata.level() <= &loglevel
      },
    )))
    .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let opts = CliOpts::parse();

  initialize_logging(match opts.verbose {
    1 => Level::DEBUG,
    2 => Level::TRACE,
    _ => Level::INFO,
  });

  // Print general startup configuration information.
  print_essentials(&opts)?;

  // Create the P2P networking layer.
  // Networking runs on its own separate thread,
  // and emits events by calling .poll()
  let mut network = Network::new(
    opts.network_id.clone(),
    opts.secret.clone(),
    opts.listen_multiaddrs().into_iter(),
  )
  .await?;

  // Connect to all known bootstrap nodes.
  // Bootstrap nodes will then introduce the current node
  // to the rest of the p2p mesh.
  for peer in opts.peers() {
    network.connect(peer)?;
  }

  // the per-node local storage, responsible for
  // storing data that should survive crashes, such
  // as the mailbox
  let storage = PersistentStorage::new(opts.data_dir()?)?;

  // for nodes that expose an external WS rpc service
  let mut apisvc = opts
    .rpc_endpoints()
    .map(|addrs| RpcService::new(addrs, storage, opts.identity()));

  loop {
    tokio::select! {

      // core services:

      // P2P networking
      Some(event) = network.poll() => {
        match event {
          NetworkEvent::MessageReceived(msg) => info!("received message {msg:?}"),
          NetworkEvent::MessageAcknowledged(hash) => info!("received ack for {hash:?}"),
          NetworkEvent::SubscriptionReceived(sub) => info!("received subscription {sub:?}"),
        }
      },

      // optional services:

      // RPC WebSocket API
      Some(()) = apisvc.next() => {
        debug!("RpcService triggered an event");
      }
    }
  }
}
