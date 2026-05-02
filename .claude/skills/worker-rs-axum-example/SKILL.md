---
name: worker-rs-axum-example
description: Use this skill when building Cloudflare Workers with Rust (worker-rs) and axum framework. Provides project templates, best practices, and reference implementations for WASM-based serverless APIs.
version: 1.0.0
---

# Cloudflare Workers + Rust + Axum Project Template

This skill guides creation of Cloudflare Workers projects using Rust with the worker-rs crate and axum framework.

## When This Skill Applies

- Building serverless APIs on Cloudflare Workers with Rust
- Creating WASM-based backend services
- Setting up JWT authentication with D1 database
- Implementing RESTful API patterns in worker-rs

## Quick Start

### Prerequisites

1. Install Rust with wasm32 target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. Install worker-build:
   ```bash
   cargo install worker-build
   ```

3. Install wrangler (Cloudflare CLI):
   ```bash
   npm install -g wrangler
   ```

### Project Initialization

Create project structure:

```
project-name/
├── Cargo.toml
├── wrangler.toml
├── schema.sql
└── src/
    ├── lib.rs           # Entry point with #[event(fetch)]
    ├── router.rs        # Axum router definition
    ├── error.rs         # Error handling
    ├── db.rs            # Database utilities
    ├── auth/
    │   ├── mod.rs
    │   ├── jwt.rs       # JWT authentication
    │   └── hash.rs      # Password hashing
    ├── handlers/
    │   ├── mod.rs
    │   └── *.rs         # Route handlers
    └── models/
        ├── mod.rs
        └── *.rs         # Data models
```

## Key Configuration Files

### Cargo.toml

```toml
[package]
name = "project-name"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[profile.release]
lto = true
codegen-units = 1

[dependencies]
# Worker & Web APIs
worker = { version = "0.7", features = ["axum", "http", "d1"] }
worker-macros = { version = "0.7", features = ["http"] }
wasm-bindgen-futures = "0.4"
wasm-bindgen = "0.2"

# Axum and Routing
axum = { version = "0.8", default-features = false, features = ["json", "macros", "form"] }
tower-http = { version = "0.6", features = ["cors"] }
tower-service = "0.3"

# Data & Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Crypto & Encoding (for auth)
jsonwebtoken = "9.3"
hmac = "0.12"
sha2 = "0.10"
base64 = "0.22"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v7", "serde", "js"] }
getrandom = { version = "0.2", features = ["js"] }
```

### wrangler.toml

```toml
name = "project-name"
main = "build/index.js"
compatibility_date = "2026-01-01"

[build]
command = "worker-build --release"

[dev]
port = 8787
ip = "127.0.0.1"

[[d1_databases]]
binding = "db"
database_name = "project-db"
database_id = "your-database-id"

[vars]
JWT_SECRET = "your-jwt-secret-change-in-production"
```

## Core Implementation Patterns

### 1. Entry Point (lib.rs)

```rust
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
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    let mut app = router::api_router(env).layer(cors);
    Ok(app.call(req).await?)
}
```

### 2. Router Pattern (router.rs)

```rust
use axum::{
    routing::{get, post, delete},
    Router,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use worker::Env;

pub fn api_router(env: Env) -> Router {
    let state = Arc::new(env);

    Router::new()
        // Health check
        .route("/api/health", get(|| async { Json(json!({"status": "ok"})) }))
        // Your routes here
        .route("/api/resource", get(handlers::list).post(handlers::create))
        .route("/api/resource/{id}", delete(handlers::delete))
        .with_state(state)
}
```

### 3. Error Handling (error.rs)

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Worker(worker::Error),
    BadRequest(String),
    Unauthorized(String),
    NotFound(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Worker(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(json!({ "error": error_message }))).into_response()
    }
}
```

### 4. Database Access (db.rs)

```rust
use std::sync::Arc;
use worker::{D1Database, Env};
use crate::error::AppError;

pub fn get_db(env: &Arc<Env>) -> Result<D1Database, AppError> {
    env.d1("db").map_err(AppError::Worker)
}
```

### 5. Handler with Auth (handlers/*.rs)

```rust
use axum::{extract::State, Json};
use std::sync::Arc;
use worker::Env;
use crate::auth::Claims;
use crate::db::get_db;
use crate::error::AppError;

#[worker::send]
pub async fn protected_route(
    claims: Claims,  // Auto-extracted from JWT
    State(env): State<Arc<Env>>,
) -> Result<Json<YourResponse>, AppError> {
    let db = get_db(&env)?;
    // claims.sub contains user_id
    // ... handler logic
    Ok(Json(response))
}
```

### 6. JWT Claims Extractor (auth/jwt.rs)

```rust
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // user id
    pub username: String,
    pub exp: usize,
    pub iat: usize,
}

impl FromRequestParts<Arc<Env>> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Env>
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer ").map(|s| s.to_owned()))
            .ok_or_else(|| AppError::Unauthorized("Unauthorized".into()))?;

        let secret = state.var("JWT_SECRET")?;
        let key = DecodingKey::from_secret(secret.to_string().as_ref());
        let data = decode::<Claims>(&token, &key, &Validation::default())
            .map_err(|_| AppError::Unauthorized("Invalid token".into()))?;

        Ok(data.claims)
    }
}
```

## Important Notes

### Async Handlers Require `#[worker::send]`

All async handler functions must be annotated with `#[worker::send]`:

```rust
#[worker::send]
pub async fn handler() -> Result<Json<Response>, AppError> {
    // ...
}
```

### D1 Database Queries

```rust
// Query with results
let results = db
    .prepare("SELECT * FROM table WHERE id = ?1")
    .bind(&[id.into()])?
    .all()
    .await?;

let items: Vec<Item> = results.results::<serde_json::Value>()?
    .into_iter()
    .map(|v| /* parse */)
    .collect();

// Query single row
let row = db
    .prepare("SELECT * FROM table WHERE id = ?1")
    .bind(&[id.into()])?
    .first::<Item>(None)
    .await?;

// Insert/Update
db.prepare("INSERT INTO table (col) VALUES (?1)")
    .bind(&[value.into()])?
    .run()
    .await?;
```

### UUID Generation

Use UUID v7 for time-sortable IDs:

```rust
let id = uuid::Uuid::now_v7().to_string();
```

### Development Commands

```bash
# Run locally
wrangler dev

# Build for production
worker-build --release

# Deploy
wrangler deploy

# Create D1 database
wrangler d1 create project-db

# Run migrations
wrangler d1 execute project-db --file=./schema.sql
```