//! HTTP API for the Pulse Node.
//! Endpoints for devices to submit heartbeats and query network state.

pub mod rate_limit;
pub mod websocket;

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
use crate::types::{Account, Heartbeat, Transaction};
use rate_limit::{RateLimiter, RateLimitConfig};
pub use websocket::WsBroadcaster;

/// Shared application state
pub type AppState = Arc<RwLock<ProofOfLife>>;

/// Combined app state with rate limiter and WebSocket broadcaster
#[derive(Clone)]
pub struct ApiState {
    pub consensus: AppState,
    pub pulse_limiter: RateLimiter,   // Strict: heartbeat submissions
    pub query_limiter: RateLimiter,   // Lenient: read queries
    pub ws_broadcaster: Arc<WsBroadcaster>,
}

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

/// Create the API router, returning both the Router and the WsBroadcaster
/// so the block production loop can broadcast events.
pub fn create_router(state: AppState) -> (Router, Arc<WsBroadcaster>) {
    let ws_broadcaster = Arc::new(WsBroadcaster::new(256));
    
    let api_state = ApiState {
        consensus: state,
        pulse_limiter: RateLimiter::new(RateLimitConfig {
            max_requests: 30,                    // 30 heartbeats per minute per IP
            window: Duration::from_secs(60),
        }),
        query_limiter: RateLimiter::new(RateLimitConfig {
            max_requests: 120,                   // 120 queries per minute per IP
            window: Duration::from_secs(60),
        }),
        ws_broadcaster: ws_broadcaster.clone(),
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
        // Health check
        .route("/health", get(health_check))
        // Heartbeat submission
        .route("/pulse", post(submit_heartbeat))
        // Transaction submission
        .route("/tx", post(submit_transaction))
        // Query endpoints
        .route("/stats", get(get_stats))
        .route("/balance/{pubkey}", get(get_balance))
        .route("/accounts", get(get_accounts))
        .route("/block/latest", get(get_latest_block))
        .route("/blocks", get(get_blocks))
        .route("/block/:index", get(get_block_by_index))
        .route("/chain", get(get_chain_info))
        // WebSocket for live updates
        .route("/ws", get(websocket::ws_handler).with_state(ws_broadcaster.clone()))
        // Add CORS for device access
        .layer(CorsLayer::permissive())
        .with_state(api_state);

    (router, ws_broadcaster)
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
    // Rate limit by IP
    let ip = addr.ip().to_string();
    if !state.pulse_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded. Max 30 heartbeats per minute."
        })));
    }

    // Input validation
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
    // Rate limit
    let ip = addr.ip().to_string();
    if !state.pulse_limiter.check(&ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({
            "success": false,
            "error": "Rate limit exceeded"
        })));
    }

    // Input validation
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

    // Validate pubkey format
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

/// Get all accounts (network view)
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

/// Get blocks with pagination (default: latest 50)
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
    
    // Default: latest 50 blocks
    let limit = params.limit.unwrap_or(50).min(200); // cap at 200
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

/// Start the API server, returning the WsBroadcaster for the block loop to use
pub async fn start_server(state: AppState, addr: &str) -> anyhow::Result<Arc<WsBroadcaster>> {
    let (router, broadcaster) = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    info!("🌐 API server listening on {}", addr);
    info!("🔌 WebSocket endpoint: ws://{}/ws", addr);
    
    let bc = broadcaster.clone();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    
    Ok(bc)
}
