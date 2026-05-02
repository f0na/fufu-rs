use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use std::sync::OnceLock;
use worker::Env;

use crate::db::{get_db, Db};
use crate::error::AppResult;
use crate::kv::KvCache;

static START_TIME: OnceLock<u64> = OnceLock::new();

#[derive(Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub uptime: u64,
    pub checks: HealthChecks,
}

#[derive(Serialize)]
pub struct HealthChecks {
    pub d1: CheckResult,
    pub kv: CheckResult,
    pub bangumi_api: CheckResult,
    pub anime_garden_api: CheckResult,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl CheckResult {
    fn ok(latency_ms: u64) -> Self {
        Self {
            status: "ok".into(),
            latency_ms: Some(latency_ms),
        }
    }

    fn skipped() -> Self {
        Self {
            status: "skipped".into(),
            latency_ms: None,
        }
    }

    fn degraded(msg: &str) -> Self {
        Self {
            status: format!("degraded: {}", msg),
            latency_ms: None,
        }
    }
}

#[worker::send]
pub async fn health_check(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<HealthCheckResponse>> {
    let start = chrono::Utc::now().timestamp() as u64;
    let uptime = start - START_TIME.get_or_init(|| start);

    let (d1_result, kv_result) = futures::join!(check_d1(&env), check_kv(&env),);

    let mut all_ok = true;

    let d1 = match d1_result {
        Ok(ms) => {
            all_ok &= true;
            CheckResult::ok(ms)
        }
        Err(e) => {
            all_ok = false;
            CheckResult::degraded(&e.to_string())
        }
    };

    let kv = match kv_result {
        Ok(ms) => {
            all_ok &= true;
            CheckResult::ok(ms)
        }
        Err(e) => {
            all_ok = false;
            CheckResult::degraded(&e.to_string())
        }
    };

    Ok(Json(HealthCheckResponse {
        status: if all_ok {
            "ok".into()
        } else {
            "degraded".into()
        },
        uptime,
        checks: HealthChecks {
            d1,
            kv,
            bangumi_api: CheckResult::skipped(),
            anime_garden_api: CheckResult::skipped(),
        },
    }))
}

async fn check_d1(env: &Arc<Env>) -> AppResult<u64> {
    let start = chrono::Utc::now().timestamp_millis();
    let db = get_db(env, Db::Core)?;
    db.prepare("SELECT 1").run().await?;
    let elapsed = (chrono::Utc::now().timestamp_millis() - start) as u64;
    Ok(elapsed)
}

async fn check_kv(env: &Arc<Env>) -> AppResult<u64> {
    let start = chrono::Utc::now().timestamp_millis();
    let cache = KvCache::new(env)?;
    cache.get_str("__health_check").await?;
    let elapsed = (chrono::Utc::now().timestamp_millis() - start) as u64;
    Ok(elapsed)
}
