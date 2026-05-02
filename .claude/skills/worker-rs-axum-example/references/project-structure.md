# Project Structure Reference

## Complete Directory Layout

```
project-name/
├── Cargo.toml              # Rust dependencies and build config
├── Cargo.lock              # Dependency lock file (generated)
├── wrangler.toml           # Cloudflare Workers configuration
├── schema.sql              # D1 database schema
├── .gitignore
├── src/
│   ├── lib.rs              # WASM entry point with #[event(fetch)]
│   ├── router.rs           # Axum router definitions
│   ├── error.rs            # Custom error types and IntoResponse
│   ├── db.rs               # Database connection helper
│   ├── auth/
│   │   ├── mod.rs          # Re-exports auth modules
│   │   ├── jwt.rs          # JWT generation and Claims extractor
│   │   └── hash.rs         # Password hashing utilities
│   ├── handlers/
│   │   ├── mod.rs          # Re-exports handler modules
│   │   ├── auth.rs         # Authentication handlers
│   │   └── resources.rs    # CRUD handlers for resources
│   └── models/
│       ├── mod.rs          # Re-exports model modules
│       ├── user.rs         # User model and DTOs
│       └── resource.rs      # Resource model and DTOs
└── target/                  # Build artifacts (gitignored)
```

## Module Organization

### src/lib.rs - Entry Point

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

### src/router.rs - Route Definitions

```rust
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
    let state = Arc::new(env);

    Router::new()
        // Health check (no auth required)
        .route("/api/health", get(|| async {
            Json(json!({"status": "ok"}))
        }))

        // Auth routes (public)
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))  // Protected by Claims extractor

        // Resource routes (CRUD)
        .route("/api/resources", get(resources::list).post(resources::create))
        .route("/api/resources/{id}", get(resources::get)
                                        .put(resources::update)
                                        .delete(resources::delete))

        .with_state(state)
}
```

### src/error.rs - Error Handling

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
    Forbidden(String),
    Internal(String),
}

impl From<worker::Error> for AppError {
    fn from(e: worker::Error) -> Self {
        AppError::Worker(e)
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        AppError::Unauthorized(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Worker(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}
```

### src/db.rs - Database Helper

```rust
use std::sync::Arc;
use worker::{D1Database, Env};
use crate::error::AppError;

pub fn get_db(env: &Arc<Env>) -> Result<D1Database, AppError> {
    env.d1("db").map_err(AppError::Worker)
}
```

### src/auth/mod.rs - Auth Module

```rust
pub mod hash;
pub mod jwt;

pub use hash::*;
pub use jwt::*;
```

### src/handlers/mod.rs - Handlers Module

```rust
pub mod auth;
pub mod resources;
```

### src/models/mod.rs - Models Module

```rust
pub mod user;
pub mod resource;

pub use user::*;
pub use resource::*;
```

## Best Practices

1. **Module Organization**: Group related code by domain (auth, handlers, models)
2. **Error Handling**: Use custom AppError enum with IntoResponse
3. **State Management**: Use `Arc<Env>` for shared state across handlers
4. **Async Handlers**: Always annotate with `#[worker::send]`
5. **Database Access**: Create a helper function `get_db()` for consistent access
6. **Route Protection**: Use `Claims` extractor to protect routes requiring auth
7. **CORS**: Configure CORS layer at the app level in lib.rs