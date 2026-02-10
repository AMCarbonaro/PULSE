//! HTTP API for the Pulse Node.
//! Endpoints for devices to submit heartbeats and query network state.

use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::consensus::ProofOfLife;
use crate::types::{Account, Heartbeat, Transaction};

/// Shared application state
pub type AppState = Arc<RwLock<ProofOfLife>>;

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
pub fn create_router(state: AppState) -> Router {
    Router::new()
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
        .route("/blocks", get(get_blocks))  // before /block/:index so literal path matches first
        .route("/block/:index", get(get_block_by_index))
        .route("/chain", get(get_chain_info))
        // Add CORS for device access
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::ok("Pulse node is alive"))
}

/// Submit a heartbeat
async fn submit_heartbeat(
    State(state): State<AppState>,
    Json(heartbeat): Json<Heartbeat>,
) -> impl IntoResponse {
    let mut pol = state.write().await;
    
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
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> impl IntoResponse {
    let mut pol = state.write().await;
    
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
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pol = state.read().await;
    Json(ApiResponse::ok(pol.get_stats()))
}

/// Get account balance
async fn get_balance(
    State(state): State<AppState>,
    axum::extract::Path(pubkey): axum::extract::Path<String>,
) -> impl IntoResponse {
    let pol = state.read().await;
    let balance = pol.get_balance(&pubkey);
    
    #[derive(Serialize)]
    struct BalanceResponse {
        pubkey: String,
        balance: f64,
    }
    
    Json(ApiResponse::ok(BalanceResponse { pubkey, balance }))
}

/// Get all accounts (network view)
async fn get_accounts(State(state): State<AppState>) -> impl IntoResponse {
    let pol = state.read().await;
    let accounts: Vec<Account> = pol.get_accounts().values().cloned().collect();
    Json(ApiResponse::ok(accounts))
}

/// Get the latest block
async fn get_latest_block(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pol = state.read().await;
    
    match pol.latest_block() {
        Some(block) => Json(serde_json::json!({
            "success": true,
            "data": block
        })),
        None => Json(serde_json::json!({
            "success": false,
            "error": "No blocks yet"
        })),
    }
}

/// Get full chain (genesis to tip)
async fn get_blocks(State(state): State<AppState>) -> impl IntoResponse {
    let pol = state.read().await;
    Json(ApiResponse::ok(pol.get_blocks()))
}

/// Get block by index
async fn get_block_by_index(
    State(state): State<AppState>,
    Path(index): Path<u64>,
) -> impl IntoResponse {
    let pol = state.read().await;
    match pol.get_block_by_index(index) {
        Some(block) => (StatusCode::OK, Json(ApiResponse::ok(block))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ApiResponse::<()>::err("Block not found"))).into_response(),
    }
}

/// Get chain info
async fn get_chain_info(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pol = state.read().await;
    
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
    
    Json(ApiResponse::ok(info))
}

/// Start the API server
pub async fn start_server(state: AppState, addr: &str) -> anyhow::Result<()> {
    let router = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    info!("🌐 API server listening on {}", addr);
    axum::serve(listener, router).await?;
    
    Ok(())
}
