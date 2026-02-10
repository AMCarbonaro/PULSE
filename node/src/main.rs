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
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use pulse_node::{
    api::{self, AppState},
    consensus::{ConsensusConfig, ProofOfLife},
    crypto::Keypair,
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
    
    // Create consensus engine
    let consensus_config = ConsensusConfig {
        n_threshold: config.n_threshold,
        block_interval_ms: config.block_interval_ms,
        reward_per_block: config.reward_per_block,
        ..Default::default()
    };
    
    let pol = ProofOfLife::new(consensus_config.clone());
    let state: AppState = Arc::new(RwLock::new(pol));
    
    // Spawn block production loop
    let block_state = state.clone();
    let block_interval = config.block_interval_ms;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(block_interval));
        loop {
            interval.tick().await;
            
            let mut pol = block_state.write().await;
            if let Ok(Some(_block)) = pol.try_create_block() {
                // Block created and committed
                // In production, would broadcast to peers here
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
    
    // Start API server
    let addr = format!("0.0.0.0:{}", config.api_port);
    api::start_server(state, &addr).await?;
    
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
