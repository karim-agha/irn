use {
  crate::{bus::MessageBusEvent, optstream::OptionalStreamExt, rpc::RpcEvent},
  bus::MessageBus,
  clap::Parser,
  cli::CliOpts,
  futures::StreamExt,
  network::{Network, NetworkEvent},
  rpc::RpcService,
  storage::PersistentStorage,
  tracing::{info, Level},
  tracing_subscriber::{filter::filter_fn, prelude::*},
};

mod bus;
mod cli;
mod network;
mod optstream;
mod primitives;
mod rpc;
mod storage;

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
    opts.peers(),
  )
  .await?;

  // routes messages to topics on the local node
  // if the subscription is managed by this node,
  // otherwise store the message until a new subscription
  // is established for its topic or the message is ACKd by
  // some other node as delivered.
  let mut bus = MessageBus::new();

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
      Some(event) = network.poll() => network::handle!(event, bus),
      Some(event) = bus.next() => bus::handle!(event, network),

      // optional services:
      Some(event) = apisvc.next() => rpc::handle!(event, bus, network)
    }
  }
}
