//! Pulse Network Node
//! 
//! A Proof-of-Life consensus node for the Pulse Network.
//! 
//! Usage:
//!   pulse-node [OPTIONS]
//! 
//! Options:
//!   --port <PORT>       API port (default: 8080)
//!   --p2p-port <PORT>   P2P port (default: 4001)
//!   --data-dir <PATH>   Data directory (default: ./pulse-data)
//!   --threshold <N>     Minimum live participants (default: 1)
//!   --interval <MS>     Block interval in ms (default: 5000)

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

use pulse_node::{
    api::{self, AppState},
    api::websocket::WsEvent,
    api::events::NodeEvent,
    consensus::{ConsensusConfig, ProofOfLife},
    crypto::Keypair,
    storage::Storage,
    types::{Heartbeat, Motion},
};

#[derive(Debug)]
struct Config {
    api_port: u16,
    p2p_port: u16,
    data_dir: String,
    n_threshold: usize,
    block_interval_ms: u64,
    reward_per_block: f64,
    simulate: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_port: 8080,
            p2p_port: 4001,
            data_dir: "./pulse-data".to_string(),
            n_threshold: 1,
            block_interval_ms: 5000,
            reward_per_block: 100.0,
            simulate: false,
        }
    }
}

fn parse_args() -> Config {
    let mut config = Config::default();
    let args: Vec<String> = std::env::args().collect();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                config.api_port = args.get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(8080);
                i += 1;
            }
            "--p2p-port" => {
                config.p2p_port = args.get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(4001);
                i += 1;
            }
            "--data-dir" => {
                config.data_dir = args.get(i + 1)
                    .cloned()
                    .unwrap_or_else(|| "./pulse-data".to_string());
                i += 1;
            }
            "--threshold" => {
                config.n_threshold = args.get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                i += 1;
            }
            "--interval" => {
                config.block_interval_ms = args.get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5000);
                i += 1;
            }
            "--simulate" => {
                config.simulate = true;
            }
            _ => {}
        }
        i += 1;
    }
    
    config
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .pretty()
        .init();
    
    let config = parse_args();
    
    println!(r#"
    ╔═══════════════════════════════════════════════════════════╗
    ║                                                           ║
    ║   🫀  PULSE NETWORK NODE                                  ║
    ║       Proof-of-Life Consensus                             ║
    ║                                                           ║
    ╚═══════════════════════════════════════════════════════════╝
    "#);
    
    info!("Starting Pulse Node...");
    info!("  API Port: {}", config.api_port);
    info!("  P2P Port: {}", config.p2p_port);
    info!("  Data Dir: {}", config.data_dir);
    info!("  Threshold: {} participants", config.n_threshold);
    info!("  Block Interval: {}ms", config.block_interval_ms);
    
    // Create consensus engine with persistent storage
    let consensus_config = ConsensusConfig {
        n_threshold: config.n_threshold,
        block_interval_ms: config.block_interval_ms,
        initial_reward_per_block: config.reward_per_block,
        ..Default::default()
    };

    // Open persistent storage
    let storage = match Storage::open(&config.data_dir) {
        Ok(s) => {
            info!("💾 Storage opened at: {}", config.data_dir);
            Arc::new(s)
        }
        Err(e) => {
            error!("❌ Failed to open storage at {}: {}", config.data_dir, e);
            error!("   Falling back to in-memory mode (data will NOT persist!)");
            // Fall back to in-memory
            let pol = ProofOfLife::new(consensus_config.clone());
            let state: AppState = Arc::new(RwLock::new(pol));
            return run_node(state, &config).await;
        }
    };

    // Create consensus engine with storage (loads existing chain if present)
    let pol = match ProofOfLife::with_storage(consensus_config.clone(), storage) {
        Ok(p) => p,
        Err(e) => {
            error!("❌ Failed to load chain from storage: {}", e);
            error!("   Starting fresh with in-memory mode");
            ProofOfLife::new(consensus_config.clone())
        }
    };

    let state: AppState = Arc::new(RwLock::new(pol));
    run_node(state, &config).await
}

async fn run_node(state: AppState, config: &Config) -> anyhow::Result<()> {
    // Start API server (returns broadcaster + event log for block loop)
    let addr = format!("0.0.0.0:{}", config.api_port);
    let handles = api::start_server(state.clone(), &addr).await?;
    let broadcaster = handles.broadcaster;
    let event_log = handles.event_log;
    
    // Log node start event
    {
        let pol = state.read().await;
        event_log.push(NodeEvent::NodeStarted {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap()
                .as_millis() as u64,
            version: api::NODE_VERSION.to_string(),
            chain_height: pol.chain_height(),
        }).await;
    }
    
    // Spawn block production loop with WebSocket broadcasting + event logging
    let block_state = state.clone();
    let block_interval = config.block_interval_ms;
    let block_broadcaster = broadcaster.clone();
    let block_event_log = event_log.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(block_interval));
        loop {
            interval.tick().await;
            
            let mut pol = block_state.write().await;
            
            // Broadcast heartbeat pool size
            let pool_size = pol.heartbeat_pool_size();
            if pool_size > 0 {
                block_broadcaster.broadcast(WsEvent::HeartbeatCount { count: pool_size });
            }
            
            if let Ok(Some(block)) = pol.try_create_block() {
                // Log block event
                block_event_log.push(NodeEvent::BlockCreated {
                    timestamp: block.timestamp,
                    index: block.index,
                    block_hash: block.block_hash.clone(),
                    n_live: block.n_live,
                    total_weight: block.total_weight,
                    security: block.security,
                    rewards_distributed: 100.0, // reward_per_block
                }).await;
                
                // Log each heartbeat in the block
                for hb in &block.heartbeats {
                    block_event_log.push(NodeEvent::HeartbeatReceived {
                        timestamp: hb.timestamp,
                        device_pubkey: hb.device_pubkey[..16].to_string() + "...",
                        heart_rate: hb.heart_rate,
                        weight: hb.weight(),
                    }).await;
                }
                
                // Broadcast new block to WebSocket clients
                block_broadcaster.broadcast(WsEvent::NewBlock { block });
                
                // Broadcast updated stats
                let stats = pol.get_stats();
                block_broadcaster.broadcast(WsEvent::Stats { stats });
            }
        }
    });
    
    // Spawn simulation if enabled
    if config.simulate {
        let sim_state = state.clone();
        tokio::spawn(async move {
            simulate_heartbeats(sim_state).await;
        });
    }
    
    // Keep main alive
    info!("🚀 Pulse node running!");
    tokio::signal::ctrl_c().await?;
    info!("👋 Shutting down...");
    
    Ok(())
}

/// Simulate heartbeats for testing (when --simulate is passed)
async fn simulate_heartbeats(state: AppState) {
    use rand::{Rng, SeedableRng};
    use rand::rngs::StdRng;
    
    info!("🎭 Starting heartbeat simulation...");
    
    // Create simulated devices
    let devices: Vec<Keypair> = (0..3)
        .map(|_| Keypair::generate())
        .collect();
    
    for (i, kp) in devices.iter().enumerate() {
        info!("  Device {}: {}...", i, &kp.public_key_hex()[..16]);
    }
    
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    let mut rng = StdRng::from_entropy();
    
    loop {
        interval.tick().await;
        
        // Each device sends a heartbeat
        for device in &devices {
            let activity: f64 = rng.gen_range(0.0..0.5);
            
            let mut hb = Heartbeat {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                heart_rate: 70 + (activity * 60.0) as u16 + rng.gen_range(0..10),
                motion: Motion {
                    x: rng.gen_range(-0.2..0.2) + activity * 0.5,
                    y: rng.gen_range(-0.2..0.2) + activity * 0.3,
                    z: rng.gen_range(-0.1..0.1) + activity * 0.2,
                },
                temperature: 36.5 + rng.gen_range(-0.5..0.5),
                device_pubkey: device.public_key_hex(),
                signature: String::new(),
            };
            
            hb.signature = device.sign(&hb.signable_bytes());
            
            let mut pol = state.write().await;
            if pol.receive_heartbeat(hb).is_ok() {
                // Heartbeat accepted
            }
        }
    }
}
