// src/lib.rs - WASM Entry Point
// This is the main entry point for Cloudflare Workers

mod auth;
mod db;
mod error;
mod handlers;
mod models;
mod router;

use tower_http::cors::{Any, CorsLayer};
use tower_service::Service;
use worker::*;

#[event(fetch)]
pub async fn main(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    // Configure CORS for cross-origin requests
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    // Build the router with CORS layer
    let mut app = router::api_router(env).layer(cors);

    // Handle the incoming request
    Ok(app.call(req).await?)
}