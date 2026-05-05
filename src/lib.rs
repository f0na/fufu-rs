#![allow(dead_code)]

mod auth;
mod db;
mod error;
mod handlers;
mod kv;
mod middleware;
mod models;
mod router;
mod time;

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::Arc;
use axum::response::IntoResponse;
use tower_http::cors::{Any, CorsLayer};
use tower_service::Service;
use worker::*;

use crate::error::AppError;

const RATE_WINDOW: u64 = 60;
const RATE_MAX: u64 = 100;

/// 内存限流：key = "ratelimit:{ip}:{window}"，value = 请求计数
static RATE_LIMITS: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn check_rate_limit(client_ip: &str) -> bool {
    let now = chrono::Utc::now().timestamp() as u64;
    let window = now / RATE_WINDOW;
    let key = format!("ratelimit:{}:{}", client_ip, window);

    let mut map = RATE_LIMITS.lock().unwrap();

    // 定期清理过期窗口，防止内存泄漏
    if map.len() > 2000 {
        map.retain(|k, _| {
            k.rsplit(':')
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .map(|w| w + 1 >= window)
                .unwrap_or(false)
        });
    }

    let count = map.get(&key).copied().unwrap_or(0);
    if count >= RATE_MAX {
        return false;
    }

    map.insert(key, count + 1);
    true
}

#[event(fetch)]
pub async fn main(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    console_error_panic_hook::set_once();

    // 读取 CORS_ORIGINS（需要在此处读取，之后 env 会被移入 Arc）
    let cors_origins = env.var("CORS_ORIGINS").ok().map(|s| s.to_string());

    // 内存限流检查，不依赖 KV，避免消耗免费额度
    let client_ip = req
        .headers()
        .get("CF-Connecting-IP")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    if !check_rate_limit(client_ip) {
        let resp = AppError::RateLimited.into_response();
        return Ok(resp.into_response());
    }

    let state = Arc::new(env);

    // CORS: 无 CORS_ORIGINS 时允许任何来源（本地开发），否则仅允许列出的来源
    let allowed_headers = tower_http::cors::AllowHeaders::list([
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
        axum::http::header::ACCEPT,
        axum::http::header::HeaderName::from_static("x-requested-with"),
    ]);
    let cors = if let Some(val) = cors_origins {
        use axum::http::header::HeaderValue;
        let origins: Vec<HeaderValue> = val
            .split(',')
            .filter_map(|s: &str| {
                let s = s.trim();
                if s.is_empty() { None } else { HeaderValue::try_from(s).ok() }
            })
            .collect();
        CorsLayer::new()
            .allow_methods(Any)
            .allow_headers(allowed_headers)
            .allow_origin(tower_http::cors::AllowOrigin::list(origins))
    } else {
        CorsLayer::new()
            .allow_methods(Any)
            .allow_headers(allowed_headers)
            .allow_origin(Any)
    };

    let mut app = router::api_router(state).layer(cors);

    Ok(app.call(req).await?)
}
