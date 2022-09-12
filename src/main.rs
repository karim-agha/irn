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
  )
  .await?;

  // Connect to all known bootstrap nodes.
  // Bootstrap nodes will then introduce the current node
  // to the rest of the p2p mesh.
  for peer in opts.peers() {
    network.connect(peer)?;
  }

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

      // P2P networking
      Some(event) = network.poll() => {
        match event {
          NetworkEvent::MessageReceived(msg) => {
            info!("received message {msg:?}");
            bus.send_message(msg).await?;
          },
          NetworkEvent::MessageAcknowledged(hash) => info!("received ack for {hash:?}"),
          NetworkEvent::SubscriptionReceived(sub) => info!("received subscription {sub:?}"),
        }
      },

      // Message Bus
      Some(event) = bus.next() => {
        match event {
          MessageBusEvent::MessageDelivered(hash) => {
            info!("Message {hash:?} delivered");
            network.gossip_ack(hash)?;
          },
          MessageBusEvent::SubscriptionCreated(topic) => {
            info!("topic {topic:?} created");
            network.gossip_subscription(topic)?;
          }
          MessageBusEvent::SubscriptionDropped(topic) => {
            info!("topic {topic:?} dropped");
          }
        }
      }

      // optional services:

      // RPC WebSocket API
      Some(event) = apisvc.next() => {
        match event {
          RpcEvent::Message(msg) => {
            info!("rpc-event message: {msg:?}");
            network.gossip_message(msg)?;
          }
          RpcEvent::Subscription(sub, socket) => {
            info!("rpc-event subscription: {sub:?}");
            bus.create_subscription(sub, socket);
          }
        }
      }
    }
  }
}
