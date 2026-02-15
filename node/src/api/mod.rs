//! HTTP API for the Pulse Node.
//! Endpoints for devices to submit heartbeats and query network state.

pub mod rate_limit;
pub mod websocket;
pub mod events;

use axum::{
    extract::{ConnectInfo, Path, Query, State, Json},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::consensus::ProofOfLife;
use crate::network::NetworkHandle;
use crate::types::{Account, Heartbeat, Transaction};
use rate_limit::{RateLimiter, RateLimitConfig};
pub use websocket::WsBroadcaster;
pub use events::EventLog;

/// Shared application state
pub type AppState = Arc<RwLock<ProofOfLife>>;

/// Combined app state with rate limiter and WebSocket broadcaster
#[derive(Clone)]
pub struct ApiState {
    pub consensus: AppState,
    pub pulse_limiter: RateLimiter,
    pub query_limiter: RateLimiter,
    pub ws_broadcaster: Arc<WsBroadcaster>,
    pub event_log: EventLog,
    pub network: NetworkHandle,
}

/// Node version info
pub const NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Pagination query parameters
#[derive(Deserialize)]
pub struct PaginationParams {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

/// API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
}

impl ApiResponse<()> {
    pub fn err(msg: impl Into<String>) -> Self {
        Self { success: false, data: None, error: Some(msg.into()) }
    }
}

/// Create the API router
pub fn create_router(state: AppState, network: NetworkHandle) -> (Router, Arc<WsBroadcaster>, EventLog) {
    let ws_broadcaster = Arc::new(WsBroadcaster::new(256));
    let event_log = EventLog::new();
    
    let api_state = ApiState {
        consensus: state,
        pulse_limiter: RateLimiter::new(RateLimitConfig {
            max_requests: 30,
            window: Duration::from_secs(60),
        }),
        query_limiter: RateLimiter::new(RateLimitConfig {
            max_requests: 120,
            window: Duration::from_secs(60),
        }),
        ws_broadcaster: ws_broadcaster.clone(),
        event_log: event_log.clone(),
        network,
    };

    // Spawn rate limiter cleanup task
    let cleanup_state = api_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            cleanup_state.pulse_limiter.cleanup().await;
            cleanup_state.query_limiter.cleanup().await;
        }
    });

    let router = Router::new()
        .route("/health", get(health_check))
        .route("/pulse", post(submit_heartbeat))
        .route("/tx", post(submit_transaction))
        .route("/stats", get(get_stats))
        .route("/balance/{pubkey}", get(get_balance))
        .route("/accounts", get(get_accounts))
        .route("/block/latest", get(get_latest_block))
        .route("/blocks", get(get_blocks))
        .route("/block/:index", get(get_block_by_index))
        .route("/chain", get(get_chain_info))
        .route("/info", get(get_node_info))
        .route("/events", get(get_events))
        .route("/peers", get(get_peers))
        .route("/ws", get(websocket::ws_handler).with_state(ws_broadcaster.clone()))
        .layer(CorsLayer::permissive())
        .with_state(api_state);

    (router, ws_broadcaster, event_log)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::ok("Pulse node is alive"))
}

/// Submit a heartbeat
async fn submit_heartbeat(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    Json(heartbeat): Json<Heartbeat>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.pulse_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded. Max 30 heartbeats per minute."
        })));
    }

    if heartbeat.device_pubkey.len() < 32 || heartbeat.device_pubkey.len() > 256 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Invalid public key length"
        })));
    }

    if heartbeat.signature.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Signature is required"
        })));
    }

    if heartbeat.heart_rate == 0 || heartbeat.heart_rate > 300 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Heart rate out of range (1-300)"
        })));
    }

    if heartbeat.temperature < 25.0 || heartbeat.temperature > 45.0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Temperature out of range (25-45°C)"
        })));
    }

    // Forward to P2P network
    let hb_for_p2p = heartbeat.clone();
    let net = state.network.clone();
    tokio::spawn(async move {
        net.broadcast_heartbeat(&hb_for_p2p).await;
    });

    let mut pol = state.consensus.write().await;
    
    match pol.receive_heartbeat(heartbeat) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "message": "Heartbeat accepted"
        }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}

/// Submit a transaction
async fn submit_transaction(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    Json(tx): Json<Transaction>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.pulse_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        })));
    }

    if tx.amount <= 0.0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Amount must be positive"
        })));
    }

    if tx.sender_pubkey == tx.recipient_pubkey {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Cannot send to yourself"
        })));
    }

    if tx.signature.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Signature is required"
        })));
    }

    let mut pol = state.consensus.write().await;
    
    match pol.receive_transaction(tx) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "message": "Transaction queued"
        }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}

/// Get network statistics
async fn get_stats(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    let pol = state.consensus.read().await;
    Json(ApiResponse::ok(pol.get_stats())).into_response()
}

/// Get account balance
async fn get_balance(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    axum::extract::Path(pubkey): axum::extract::Path<String>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    if pubkey.len() < 32 || pubkey.len() > 256 || !pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::err("Invalid public key format"))).into_response();
    }

    let pol = state.consensus.read().await;
    let balance = pol.get_balance(&pubkey);
    
    #[derive(Serialize)]
    struct BalanceResponse {
        pubkey: String,
        balance: f64,
    }
    
    Json(ApiResponse::ok(BalanceResponse { pubkey, balance })).into_response()
}

/// Get all accounts
async fn get_accounts(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    let pol = state.consensus.read().await;
    let accounts: Vec<Account> = pol.get_accounts().values().cloned().collect();
    Json(ApiResponse::ok(accounts)).into_response()
}

/// Get the latest block
async fn get_latest_block(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        })).into_response();
    }

    let pol = state.consensus.read().await;
    
    match pol.latest_block() {
        Some(block) => Json(serde_json::json!({
            "success": true,
            "data": block
        })).into_response(),
        None => Json(serde_json::json!({
            "success": false,
            "error": "No blocks yet"
        })).into_response(),
    }
}

/// Get blocks with pagination
async fn get_blocks(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    let pol = state.consensus.read().await;
    let all_blocks = pol.get_blocks();
    let total = all_blocks.len() as u64;
    
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(total.saturating_sub(limit));
    
    let blocks: Vec<_> = all_blocks.into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    #[derive(Serialize)]
    struct PaginatedBlocks {
        blocks: Vec<crate::types::PulseBlock>,
        total: u64,
        offset: u64,
        limit: u64,
    }

    Json(ApiResponse::ok(PaginatedBlocks {
        blocks,
        total,
        offset,
        limit,
    })).into_response()
}

/// Get block by index
async fn get_block_by_index(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    Path(index): Path<u64>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    let pol = state.consensus.read().await;
    match pol.get_block_by_index(index) {
        Some(block) => (StatusCode::OK, Json(ApiResponse::ok(block))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ApiResponse::<()>::err("Block not found"))).into_response(),
    }
}

/// Get chain info
async fn get_chain_info(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    let pol = state.consensus.read().await;
    
    #[derive(Serialize)]
    struct ChainInfo {
        height: u64,
        latest_hash: String,
        heartbeat_pool_size: usize,
    }
    
    let info = ChainInfo {
        height: pol.chain_height(),
        latest_hash: pol.latest_block()
            .map(|b| b.block_hash.clone())
            .unwrap_or_default(),
        heartbeat_pool_size: pol.heartbeat_pool_size(),
    };
    
    Json(ApiResponse::ok(info)).into_response()
}

/// Get node info
async fn get_node_info(
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let pol = state.consensus.read().await;
    
    #[derive(Serialize)]
    struct NodeInfo {
        version: String,
        chain_height: u64,
        active_accounts: usize,
        heartbeat_pool_size: usize,
        ws_clients: usize,
        peer_id: String,
        peer_count: usize,
    }
    
    Json(ApiResponse::ok(NodeInfo {
        version: NODE_VERSION.to_string(),
        chain_height: pol.chain_height(),
        active_accounts: pol.get_accounts().len(),
        heartbeat_pool_size: pol.heartbeat_pool_size(),
        ws_clients: state.ws_broadcaster.subscriber_count(),
        peer_id: state.network.info.peer_id.clone(),
        peer_count: state.network.info.peer_count(),
    })).into_response()
}

/// Get connected P2P peers (lock-free!)
async fn get_peers(
    State(state): State<ApiState>,
) -> impl IntoResponse {
    #[derive(Serialize)]
    struct PeerInfo {
        peer_id: String,
        peer_count: usize,
        connected_peers: Vec<String>,
    }
    
    let peers = state.network.info.connected_peers().await;
    
    Json(ApiResponse::ok(PeerInfo {
        peer_id: state.network.info.peer_id.clone(),
        peer_count: peers.len(),
        connected_peers: peers,
    })).into_response()
}

/// Query parameters for events endpoint
#[derive(Deserialize)]
pub struct EventParams {
    pub limit: Option<usize>,
    pub since: Option<u64>,
}

/// Get recent events
async fn get_events(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ApiState>,
    Query(params): Query<EventParams>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.query_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        }))).into_response();
    }

    let events = if let Some(since) = params.since {
        state.event_log.since(since).await
    } else {
        state.event_log.latest(params.limit.unwrap_or(50).min(200)).await
    };

    Json(ApiResponse::ok(events)).into_response()
}

/// Return type for start_server
pub struct ServerHandles {
    pub broadcaster: Arc<WsBroadcaster>,
    pub event_log: EventLog,
}

/// Start the API server
pub async fn start_server(
    state: AppState,
    addr: &str,
    network: NetworkHandle,
) -> anyhow::Result<ServerHandles> {
    let (router, broadcaster, event_log) = create_router(state, network);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    info!("🌐 API server listening on {}", addr);
    info!("🔌 WebSocket endpoint: ws://{}/ws", addr);
    
    let bc = broadcaster.clone();
    let el = event_log.clone();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    
    Ok(ServerHandles { broadcaster: bc, event_log: el })
}
