//! P2P networking for the Pulse Network using libp2p.
//! Handles peer discovery, heartbeat propagation, and block gossip.

use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    mdns,
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, debug};

use crate::types::{Heartbeat, PulseBlock};

/// Topics for gossipsub
pub const HEARTBEAT_TOPIC: &str = "pulse/heartbeats/1.0.0";
pub const BLOCK_TOPIC: &str = "pulse/blocks/1.0.0";

/// Messages that can be sent over the network
#[derive(Debug, Clone)]
pub enum NetworkMessage {
    Heartbeat(Heartbeat),
    Block(PulseBlock),
}

/// Combined network behaviour
#[derive(NetworkBehaviour)]
pub struct PulseBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

/// P2P network handler
pub struct Network {
    swarm: Swarm<PulseBehaviour>,
    heartbeat_topic: IdentTopic,
    block_topic: IdentTopic,
}

impl Network {
    /// Create a new network instance
    pub async fn new() -> anyhow::Result<Self> {
        // Generate identity
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("🔑 Local peer ID: {}", local_peer_id);
        
        // Create transport
        let transport = tcp::tokio::Transport::default()
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();
        
        // Create gossipsub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .build()
            .expect("Valid gossipsub config");
        
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        ).map_err(|e| anyhow::anyhow!("Gossipsub error: {}", e))?;
        
        // Create mDNS for local peer discovery
        let mdns = mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        )?;
        
        // Combined behaviour
        let behaviour = PulseBehaviour { gossipsub, mdns };
        
        // Create swarm
        let swarm = Swarm::new(
            transport,
            behaviour,
            local_peer_id,
            libp2p::swarm::Config::with_tokio_executor(),
        );
        
        let heartbeat_topic = IdentTopic::new(HEARTBEAT_TOPIC);
        let block_topic = IdentTopic::new(BLOCK_TOPIC);
        
        Ok(Self {
            swarm,
            heartbeat_topic,
            block_topic,
        })
    }
    
    /// Start listening on the given address
    pub fn listen(&mut self, addr: &str) -> anyhow::Result<()> {
        let multiaddr: Multiaddr = addr.parse()?;
        self.swarm.listen_on(multiaddr)?;
        Ok(())
    }
    
    /// Subscribe to topics
    pub fn subscribe(&mut self) -> anyhow::Result<()> {
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.heartbeat_topic)?;
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.block_topic)?;
        info!("📡 Subscribed to gossip topics");
        Ok(())
    }
    
    /// Broadcast a heartbeat
    pub fn broadcast_heartbeat(&mut self, hb: &Heartbeat) -> anyhow::Result<()> {
        let data = serde_json::to_vec(hb)?;
        self.swarm.behaviour_mut().gossipsub.publish(
            self.heartbeat_topic.clone(),
            data,
        )?;
        debug!("📤 Broadcast heartbeat from {}...", &hb.device_pubkey[..8]);
        Ok(())
    }
    
    /// Broadcast a block
    pub fn broadcast_block(&mut self, block: &PulseBlock) -> anyhow::Result<()> {
        let data = serde_json::to_vec(block)?;
        self.swarm.behaviour_mut().gossipsub.publish(
            self.block_topic.clone(),
            data,
        )?;
        info!("📤 Broadcast block #{}", block.index);
        Ok(())
    }
    
    /// Get number of connected peers
    pub fn peer_count(&self) -> usize {
        self.swarm.connected_peers().count()
    }
    
    /// Run the network event loop
    pub async fn run(
        &mut self,
        incoming_tx: mpsc::Sender<NetworkMessage>,
    ) -> anyhow::Result<()> {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::Behaviour(PulseBehaviourEvent::Mdns(event)) => {
                    match event {
                        mdns::Event::Discovered(peers) => {
                            for (peer_id, addr) in peers {
                                info!("🔍 Discovered peer: {} at {}", peer_id, addr);
                                self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        mdns::Event::Expired(peers) => {
                            for (peer_id, _) in peers {
                                debug!("👋 Peer expired: {}", peer_id);
                            }
                        }
                    }
                }
                SwarmEvent::Behaviour(PulseBehaviourEvent::Gossipsub(event)) => {
                    if let gossipsub::Event::Message { message, .. } = event {
                        let topic = message.topic.as_str();
                        
                        if topic == HEARTBEAT_TOPIC {
                            if let Ok(hb) = serde_json::from_slice::<Heartbeat>(&message.data) {
                                let _ = incoming_tx.send(NetworkMessage::Heartbeat(hb)).await;
                            }
                        } else if topic == BLOCK_TOPIC {
                            if let Ok(block) = serde_json::from_slice::<PulseBlock>(&message.data) {
                                let _ = incoming_tx.send(NetworkMessage::Block(block)).await;
                            }
                        }
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("📡 Listening on {}", address);
                }
                _ => {}
            }
        }
    }
}
