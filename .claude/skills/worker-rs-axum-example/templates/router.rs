// src/router.rs - Axum Router Definition

use axum::{
    routing::{get, post, delete, put},
    Router,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use worker::Env;

use crate::handlers::{auth, resources};

pub fn api_router(env: Env) -> Router {
    // Wrap env in Arc for shared state
    let state = Arc::new(env);

    Router::new()
        // ============================================
        // Public Routes (no authentication required)
        // ============================================

        // Health check endpoint
        .route("/api/health", get(|| async {
            Json(json!({"status": "ok"}))
        }))

        // Authentication routes
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))

        // ============================================
        // Protected Routes (require authentication)
        // ============================================

        // Current user info (Claims extractor handles auth)
        .route("/api/auth/me", get(auth::me))

        // Resource CRUD operations
        .route("/api/resources",
            get(resources::list)      // List all resources
            .post(resources::create)  // Create new resource
        )
        .route("/api/resources/{id}",
            get(resources::get)       // Get single resource
            .put(resources::update)   // Update resource
            .delete(resources::delete) // Delete resource
        )

        // Attach shared state
        .with_state(state)
}