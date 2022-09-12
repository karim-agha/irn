mod episub;

use {
  crate::primitives::{Keypair, Message, Subscription},
  episub::{Config, Episub, EpisubEvent, PeerAuthorizer},
  futures::StreamExt,
  libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed, upgrade::Version},
    dns::{DnsConfig, ResolverConfig, ResolverOpts},
    identity::{self, ed25519::SecretKey},
    multihash::Multihash,
    noise,
    swarm::SwarmEvent,
    tcp::{GenTcpConfig, TcpTransport},
    yamux::YamuxConfig,
    Multiaddr,
    PeerId,
    Swarm,
    Transport,
  },
  std::time::Duration,
  tokio::sync::mpsc::{
    error::SendError,
    unbounded_channel,
    UnboundedReceiver,
    UnboundedSender,
  },
  tracing::{debug, error, warn},
};

type BoxedTransport = Boxed<(PeerId, StreamMuxerBox)>;

async fn create_transport(
  keypair: &Keypair,
) -> std::io::Result<BoxedTransport> {
  let transport = {
    let tcp =
      TcpTransport::new(GenTcpConfig::new().nodelay(true).port_reuse(true));
    let dns_tcp = DnsConfig::custom(
      tcp,
      ResolverConfig::default(),
      ResolverOpts::default(),
    )
    .await?;
    dns_tcp
  };

  let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
    .into_authentic(&identity::Keypair::Ed25519(
      SecretKey::from_bytes(keypair.secret().to_bytes())
        .unwrap()
        .into(),
    ))
    .expect("Signing libp2p-noise static DH keypair failed.");

  Ok(
    transport
      .upgrade(Version::V1)
      .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
      .multiplex(YamuxConfig::default())
      .boxed(),
  )
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
  MessageReceived(Message),
  MessageAcknowledged(Multihash),
  SubscriptionReceived(Subscription),
}

// this is a bug in clippy, I filed an issue on GH:
// https://github.com/rust-lang/rust-clippy/issues/8321
// remove this when the issue gets closed.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum NetworkCommand {
  Connect(Multiaddr),
  GossipMessage(Message),
  GossipACK(Multihash),
  GossipSubscription(Subscription),
}

pub struct Network {
  netin: UnboundedReceiver<NetworkEvent>,
  netout: UnboundedSender<NetworkCommand>,
}

impl Network {
  pub async fn new(
    network_id: String,
    keypair: Keypair,
    listenaddrs: impl Iterator<Item = Multiaddr>,
  ) -> std::io::Result<Self> {
    let id = identity::Keypair::Ed25519(
      identity::ed25519::SecretKey::from_bytes(
        &mut keypair.secret().to_bytes(),
      )
      .unwrap()
      .into(),
    );

    // use an authentiator predicate that denies connections
    // to any peer id fails tests in this function. At this
    // moment it is an allow-all before we introduce a whitelisting
    // mechanism, either through stake, or some other means.
    let authorizer = PeerAuthorizer::new(move |_, _| true);

    let mut swarm = Swarm::new(
      create_transport(&keypair).await?,
      Episub::new(Config {
        authorizer,
        active_view_factor: 4,
        network_size: 20,
        max_transmit_size: 64 * 1024, // 64kb
        history_window: Duration::from_secs(30),
        lazy_push_window: Duration::from_secs(5),
        shuffle_probability: 0.3, // shuffle only 30% of peers at once
        ..Config::default()
      }),
      id.public().to_peer_id(),
    );

    // This is the topic where all messages send between
    // endpoints are published in case both endpoints are
    // not connected to the same relay.
    swarm
      .behaviour_mut()
      .subscribe(format!("/{}/message", network_id));

    // This is the topic where all subscription announcements
    // are published so that all nodes are aware that a subscription
    // has started or ended on a node.
    swarm
      .behaviour_mut()
      .subscribe(format!("/{}/subscribe", network_id));

    // This is the topic where all message delivery ACKs are
    // published.
    swarm
      .behaviour_mut()
      .subscribe(format!("/{}/ack", network_id));

    listenaddrs.for_each(|addr| {
      swarm.listen_on(addr).unwrap();
    });

    let (netin_tx, netin_rx) = unbounded_channel();
    let (netout_tx, mut netout_rx) = unbounded_channel();

    tokio::spawn(async move {
      loop {
        tokio::select! {
          Some(event) = swarm.next() => {
            if let SwarmEvent::Behaviour(EpisubEvent::Message {
              topic,
              payload,
              ..
            }) = event
            {
              if topic == format!("/{}/message", network_id) {
                match bincode::deserialize(&payload) {
                  Ok(msg) => {
                    netin_tx.send(NetworkEvent::MessageReceived(msg)).unwrap();
                  }
                  Err(e) => error!("Failed to deserialize incoming message: {e}"),
                }
              } else if topic == format!("/{}/subscribe", network_id) {
                match bincode::deserialize(&payload) {
                  Ok(subscription) => {
                    debug!("Updating subscription {subscription:?}");
                    netin_tx.send(NetworkEvent::SubscriptionReceived(subscription)).unwrap();
                  }
                  Err(e) => error!("Failed to deserialize subscription command: {e}"),
                }
              } else if topic == format!("/{}/ack", network_id) {
                match Multihash::from_bytes(&payload) {
                  Ok(msg_hash) => {
                    netin_tx.send(NetworkEvent::MessageAcknowledged(msg_hash)).unwrap();
                  }
                  Err(e) => error!("Failed to deserialize message hash: {e}"),
                }
              } else {
                warn!("Received a message on an unexpected topic {topic}");
              }
            }
          },
          Some(event) = netout_rx.recv() => {
            match event {
              NetworkCommand::Connect(addr)=>{
                if let Err(e) = swarm.dial(addr.clone()) {
                  error!("Dialing peer {addr} failed: {e}");
                }
              }
              NetworkCommand::GossipACK(msghash) => {
                swarm
                .behaviour_mut()
                .publish(
                  &format!("/{}/ack", network_id),
                  bincode::serialize(&msghash).expect("failed to serialize message"))
                .unwrap();
              }
              NetworkCommand::GossipMessage(msg) => {
                swarm
                .behaviour_mut()
                .publish(
                  &format!("/{}/message", network_id),
                  bincode::serialize(&msg).expect("failed to serialize message"))
                .unwrap();
              }
              NetworkCommand::GossipSubscription(sub) => {
                swarm
                .behaviour_mut()
                .publish(
                  &format!("/{}/subscribe", network_id),
                  bincode::serialize(&sub).expect("failed to serialize subscription info"))
                .unwrap();
              }
            }
          }
        }
      }
    });
    Ok(Self {
      netin: netin_rx,
      netout: netout_tx,
    })
  }

  pub fn connect(
    &mut self,
    addr: Multiaddr,
  ) -> Result<(), SendError<NetworkCommand>> {
    self.netout.send(NetworkCommand::Connect(addr))
  }

  pub fn gossip_message(
    &mut self,
    message: Message,
  ) -> Result<(), SendError<NetworkCommand>> {
    self.netout.send(NetworkCommand::GossipMessage(message))
  }

  pub fn gossip_subscription(
    &mut self,
    sub: Subscription,
  ) -> Result<(), SendError<NetworkCommand>> {
    self.netout.send(NetworkCommand::GossipSubscription(sub))
  }

  pub fn gossip_ack(
    &mut self,
    hash: Multihash,
  ) -> Result<(), SendError<NetworkCommand>> {
    self.netout.send(NetworkCommand::GossipACK(hash))
  }

  pub async fn poll(&mut self) -> Option<NetworkEvent> {
    self.netin.recv().await
  }
}
