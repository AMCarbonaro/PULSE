//! P2P networking for the Pulse Network using libp2p.
//! 
//! Architecture: The Network struct owns the libp2p swarm and runs in its own
//! dedicated tokio task. All other components communicate with it via channels:
//! 
//! - `NetworkCommand` channel (mpsc): other tasks send commands TO the network
//!   (broadcast heartbeat, broadcast block, dial peer, request chain sync, etc.)
//! - `NetworkMessage` channel (mpsc): network sends received messages FROM peers
//!   to the consensus/processing task
//! - `NetworkHandle`: cheaply cloneable handle for sending commands + querying state

use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    mdns,
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug, warn, error};

use crate::types::{Heartbeat, PulseBlock};

/// Topics for gossipsub
pub const HEARTBEAT_TOPIC: &str = "pulse/heartbeats/1.0.0";
pub const BLOCK_TOPIC: &str = "pulse/blocks/1.0.0";
pub const CHAIN_SYNC_TOPIC: &str = "pulse/chain-sync/1.0.0";

/// Chain sync request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSyncRequest {
    pub from_height: u64,
}

/// Chain sync response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSyncResponse {
    pub blocks: Vec<PulseBlock>,
}

/// Messages received FROM the network (peers → us)
#[derive(Debug, Clone)]
pub enum NetworkMessage {
    Heartbeat(Heartbeat),
    Block(PulseBlock),
    ChainSyncRequest(ChainSyncRequest),
    ChainSyncResponse(ChainSyncResponse),
}

/// Commands sent TO the network (us → swarm)
#[derive(Debug)]
pub enum NetworkCommand {
    BroadcastHeartbeat(Heartbeat),
    BroadcastBlock(PulseBlock),
    BroadcastChainSyncRequest(ChainSyncRequest),
    BroadcastChainSyncResponse(ChainSyncResponse),
    DialPeer(String),
}

/// Shared peer info (atomics + RwLock for lock-free reads)
#[derive(Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    peer_count: Arc<AtomicUsize>,
    peer_list: Arc<RwLock<Vec<String>>>,
}

impl PeerInfo {
    fn new(peer_id: String) -> Self {
        Self {
            peer_id,
            peer_count: Arc::new(AtomicUsize::new(0)),
            peer_list: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn peer_count(&self) -> usize {
        self.peer_count.load(Ordering::Relaxed)
    }

    pub async fn connected_peers(&self) -> Vec<String> {
        self.peer_list.read().await.clone()
    }
}

/// Cheaply cloneable handle for interacting with the network from any task.
/// Does NOT hold a lock on the swarm — sends commands via channel.
#[derive(Clone)]
pub struct NetworkHandle {
    cmd_tx: mpsc::Sender<NetworkCommand>,
    pub info: PeerInfo,
}

impl NetworkHandle {
    pub async fn broadcast_heartbeat(&self, hb: &Heartbeat) {
        let _ = self.cmd_tx.send(NetworkCommand::BroadcastHeartbeat(hb.clone())).await;
    }

    pub async fn broadcast_block(&self, block: &PulseBlock) {
        let _ = self.cmd_tx.send(NetworkCommand::BroadcastBlock(block.clone())).await;
    }

    pub async fn broadcast_chain_sync_request(&self, req: &ChainSyncRequest) {
        let _ = self.cmd_tx.send(NetworkCommand::BroadcastChainSyncRequest(req.clone())).await;
    }

    pub async fn broadcast_chain_sync_response(&self, resp: &ChainSyncResponse) {
        let _ = self.cmd_tx.send(NetworkCommand::BroadcastChainSyncResponse(resp.clone())).await;
    }

    pub async fn dial_peer(&self, addr: &str) {
        let _ = self.cmd_tx.send(NetworkCommand::DialPeer(addr.to_string())).await;
    }
}

/// Combined network behaviour
#[derive(NetworkBehaviour)]
struct PulseBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

/// Start the P2P network. Returns a handle for other tasks to use, 
/// and the receiver for incoming messages from peers.
/// The network runs in a background task — caller does NOT need to poll it.
pub async fn start(
    port: u16,
) -> anyhow::Result<(NetworkHandle, mpsc::Receiver<NetworkMessage>)> {
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

    // Create gossipsub with relaxed mesh settings for small networks
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        // Lower mesh params so 2-node networks can relay messages
        .mesh_n_low(1)
        .mesh_n(2)
        .mesh_outbound_min(1)
        .mesh_n_high(12)
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

    let behaviour = PulseBehaviour { gossipsub, mdns };

    let mut swarm = Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        libp2p::swarm::Config::with_tokio_executor(),
    );

    // Listen
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;
    swarm.listen_on(listen_addr)?;

    // Subscribe to topics
    let heartbeat_topic = IdentTopic::new(HEARTBEAT_TOPIC);
    let block_topic = IdentTopic::new(BLOCK_TOPIC);
    let chain_sync_topic = IdentTopic::new(CHAIN_SYNC_TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&heartbeat_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&block_topic)?;
    swarm.behaviour_mut().gossipsub.subscribe(&chain_sync_topic)?;
    info!("📡 Subscribed to gossip topics");

    // Channels
    let (cmd_tx, cmd_rx) = mpsc::channel::<NetworkCommand>(256);
    let (msg_tx, msg_rx) = mpsc::channel::<NetworkMessage>(256);

    let peer_info = PeerInfo::new(local_peer_id.to_string());
    let handle = NetworkHandle {
        cmd_tx,
        info: peer_info.clone(),
    };

    // Spawn the event loop as a background task
    tokio::spawn(run_event_loop(
        swarm,
        heartbeat_topic,
        block_topic,
        chain_sync_topic,
        cmd_rx,
        msg_tx,
        peer_info,
    ));

    Ok((handle, msg_rx))
}

/// The network event loop — runs forever in its own task.
/// Owns the swarm exclusively (no Mutex needed).
async fn run_event_loop(
    mut swarm: Swarm<PulseBehaviour>,
    heartbeat_topic: IdentTopic,
    block_topic: IdentTopic,
    chain_sync_topic: IdentTopic,
    mut cmd_rx: mpsc::Receiver<NetworkCommand>,
    msg_tx: mpsc::Sender<NetworkMessage>,
    peer_info: PeerInfo,
) {
    loop {
        tokio::select! {
            // Process incoming swarm events
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(PulseBehaviourEvent::Mdns(mdns_event)) => {
                        match mdns_event {
                            mdns::Event::Discovered(peers) => {
                                for (peer_id, addr) in peers {
                                    info!("🔍 Discovered peer: {} at {}", peer_id, addr);
                                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                }
                            }
                            mdns::Event::Expired(peers) => {
                                for (peer_id, _) in peers {
                                    debug!("👋 Peer expired: {}", peer_id);
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(PulseBehaviourEvent::Gossipsub(gs_event)) => {
                        if let gossipsub::Event::Message { message, .. } = gs_event {
                            let topic = message.topic.as_str();

                            if topic == HEARTBEAT_TOPIC {
                                if let Ok(hb) = serde_json::from_slice::<Heartbeat>(&message.data) {
                                    let _ = msg_tx.send(NetworkMessage::Heartbeat(hb)).await;
                                }
                            } else if topic == BLOCK_TOPIC {
                                if let Ok(block) = serde_json::from_slice::<PulseBlock>(&message.data) {
                                    let _ = msg_tx.send(NetworkMessage::Block(block)).await;
                                }
                            } else if topic == CHAIN_SYNC_TOPIC {
                                // Discriminate request vs response: try request first (smaller)
                                if let Ok(req) = serde_json::from_slice::<ChainSyncRequest>(&message.data) {
                                    // Make sure it's actually a request (has from_height, no blocks field)
                                    if serde_json::from_slice::<ChainSyncResponse>(&message.data).is_err() {
                                        let _ = msg_tx.send(NetworkMessage::ChainSyncRequest(req)).await;
                                    } else {
                                        // Both parsed — it's a response (has blocks field)
                                        if let Ok(resp) = serde_json::from_slice::<ChainSyncResponse>(&message.data) {
                                            let _ = msg_tx.send(NetworkMessage::ChainSyncResponse(resp)).await;
                                        }
                                    }
                                } else if let Ok(resp) = serde_json::from_slice::<ChainSyncResponse>(&message.data) {
                                    let _ = msg_tx.send(NetworkMessage::ChainSyncResponse(resp)).await;
                                } else {
                                    warn!("📨 Unrecognized chain sync message");
                                }
                            }
                        }
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("📡 Listening on {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!("🤝 Connected to peer: {}", peer_id);
                        let peers: Vec<String> = swarm.connected_peers().map(|p| p.to_string()).collect();
                        peer_info.peer_count.store(peers.len(), Ordering::Relaxed);
                        *peer_info.peer_list.write().await = peers;
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        info!("👋 Disconnected from peer: {}", peer_id);
                        let peers: Vec<String> = swarm.connected_peers().map(|p| p.to_string()).collect();
                        peer_info.peer_count.store(peers.len(), Ordering::Relaxed);
                        *peer_info.peer_list.write().await = peers;
                    }
                    _ => {}
                }
            }

            // Process outgoing commands from other tasks
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(NetworkCommand::BroadcastHeartbeat(hb)) => {
                        if let Ok(data) = serde_json::to_vec(&hb) {
                            if let Err(e) = swarm.behaviour_mut().gossipsub.publish(
                                heartbeat_topic.clone(), data
                            ) {
                                debug!("P2P heartbeat broadcast skipped: {}", e);
                            }
                        }
                    }
                    Some(NetworkCommand::BroadcastBlock(block)) => {
                        if let Ok(data) = serde_json::to_vec(&block) {
                            match swarm.behaviour_mut().gossipsub.publish(
                                block_topic.clone(), data
                            ) {
                                Ok(_) => info!("📤 Broadcast block #{}", block.index),
                                Err(e) => debug!("P2P block broadcast skipped: {}", e),
                            }
                        }
                    }
                    Some(NetworkCommand::BroadcastChainSyncRequest(req)) => {
                        if let Ok(data) = serde_json::to_vec(&req) {
                            match swarm.behaviour_mut().gossipsub.publish(
                                chain_sync_topic.clone(), data
                            ) {
                                Ok(_) => info!("📤 Chain sync request from height {}", req.from_height),
                                Err(e) => warn!("Chain sync request failed: {}", e),
                            }
                        }
                    }
                    Some(NetworkCommand::BroadcastChainSyncResponse(resp)) => {
                        if let Ok(data) = serde_json::to_vec(&resp) {
                            match swarm.behaviour_mut().gossipsub.publish(
                                chain_sync_topic.clone(), data
                            ) {
                                Ok(_) => info!("📤 Chain sync response ({} blocks)", resp.blocks.len()),
                                Err(e) => warn!("Chain sync response failed: {}", e),
                            }
                        }
                    }
                    Some(NetworkCommand::DialPeer(addr)) => {
                        match addr.parse::<Multiaddr>() {
                            Ok(multiaddr) => {
                                info!("📞 Dialing peer at {}", multiaddr);
                                if let Err(e) = swarm.dial(multiaddr) {
                                    error!("❌ Failed to dial peer: {}", e);
                                }
                            }
                            Err(e) => error!("❌ Invalid multiaddr '{}': {}", addr, e),
                        }
                    }
                    None => {
                        info!("Network command channel closed, shutting down P2P");
                        break;
                    }
                }
            }
        }
    }
}

// Peer info is updated inline in the event loop (ConnectionEstablished/Closed events)
