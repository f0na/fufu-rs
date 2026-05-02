use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use worker::Env;

use crate::db::{get_db, Db};
use crate::error::AppResult;
use crate::time;

// ─── 响应类型 ─────────────────────────────────────────────────

// ─── 响应类型 ─────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SiteStatus {
    /// API 服务状态
    pub api: ApiStatus,
    /// 站点基本信息
    pub site: SiteInfo,
    /// 各模块数据统计
    pub stats: SiteStats,
}

#[derive(Serialize)]
pub struct ApiStatus {
    pub status: String,
    pub uptime: u64,
    pub version: &'static str,
    pub d1: CheckResult,
    pub kv: CheckResult,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct SiteInfo {
    pub site_name: String,
    pub subtitle: String,
    pub description: String,
    pub logo_url: String,
}

#[derive(Serialize)]
pub struct SiteStats {
    pub posts: u64,
    pub friends: u64,
    pub links: u64,
    pub galleries: u64,
    pub bangumi_records: u64,
}

// ─── 工具 ─────────────────────────────────────────────────────
async fn count_table(env: &Arc<Env>, db_instance: Db, table: &str) -> u64 {
    let db = match get_db(env, db_instance) {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let sql = format!("SELECT COUNT(*) as cnt FROM {} WHERE deleted_at IS NULL", table);
    let stmt = db.prepare(&sql);
    let stmt = match stmt.bind(&[]) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let result = match stmt.all().await {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let rows: Vec<serde_json::Value> = result.results().unwrap_or_default();
    rows.first()
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as u64
}

// ─── D1 健康检测 ────────────────────────────────────────────────

async fn check_d1(env: &Arc<Env>) -> (String, Option<u64>) {
    let db = match get_db(env, Db::Core) {
        Ok(d) => d,
        Err(_) => return ("error".into(), None),
    };
    let start = time::now_epoch();
    let stmt = db.prepare("SELECT 1 as ok");
    let stmt = match stmt.bind(&[]) {
        Ok(s) => s,
        Err(_) => return ("error".into(), None),
    };
    match stmt.all().await {
        Ok(_) => ("ok".into(), Some(time::now_epoch().saturating_sub(start).max(1))),
        Err(_) => ("error".into(), None),
    }
}

// ─── KV 健康检测 ────────────────────────────────────────────────

async fn check_kv(env: &Arc<Env>) -> (String, Option<u64>) {
    let kv = match crate::kv::KvCache::new(env) {
        Ok(k) => k,
        Err(_) => return ("error".into(), None),
    };
    let start = time::now_epoch();
    match kv.get_str("__health_check").await {
        Ok(_) => ("ok".into(), Some(time::now_epoch().saturating_sub(start).max(1))),
        Err(_) => ("error".into(), None),
    }
}

// ─── 站点信息查询 ──────────────────────────────────────────────

async fn get_site_info(env: &Arc<Env>) -> SiteInfo {
    let db = match get_db(env, Db::Core) {
        Ok(d) => d,
        Err(_) => return default_site_info(),
    };
    let stmt = db.prepare("SELECT site_name, subtitle, description, logo_url FROM site_profile LIMIT 1");
    let stmt = match stmt.bind(&[]) {
        Ok(s) => s,
        Err(_) => return default_site_info(),
    };
    let result = match stmt.all().await {
        Ok(r) => r,
        Err(_) => return default_site_info(),
    };
    let rows: Vec<serde_json::Value> = result.results().unwrap_or_default();
    if let Some(row) = rows.first() {
        SiteInfo {
            site_name: row.get("site_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            subtitle: row.get("subtitle").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: row.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            logo_url: row.get("logo_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }
    } else {
        default_site_info()
    }
}

fn default_site_info() -> SiteInfo {
    SiteInfo {
        site_name: String::new(),
        subtitle: String::new(),
        description: String::new(),
        logo_url: String::new(),
    }
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/status — 公开站点状态
// ═══════════════════════════════════════════════════════════════

async fn get_started_at(env: &Arc<Env>) -> u64 {
    let kv = match crate::kv::KvCache::new(env) {
        Ok(k) => k,
        Err(_) => return time::now_epoch(),
    };
    let key = "__site_started_at";
    if let Some(val) = kv.get_str(key).await.unwrap_or(None) {
        if let Ok(ts) = val.parse::<u64>() {
            return ts;
        }
    }
    let ts = time::now_epoch();
    let _ = kv.put_str(key, &ts.to_string(), 0).await;
    ts
}

#[worker::send]
pub async fn site_status(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<SiteStatus>> {
    let started_at = get_started_at(&env).await;
    let now = time::now_epoch();
    let uptime = now.saturating_sub(started_at);

    let (d1_status, d1_latency) = check_d1(&env).await;
    let (kv_status, kv_latency) = check_kv(&env).await;

    let site = get_site_info(&env).await;

    let stats = SiteStats {
        posts: count_table(&env, Db::Posts, "posts").await,
        friends: count_table(&env, Db::Social, "friends").await,
        links: count_table(&env, Db::Social, "links").await,
        galleries: count_table(&env, Db::Media, "galleries").await,
        bangumi_records: count_table(&env, Db::Bangumi, "bangumi_records").await,
    };

    Ok(Json(SiteStatus {
        api: ApiStatus {
            status: "ok".into(),
            uptime,
            version: env!("CARGO_PKG_VERSION"),
            d1: CheckResult { status: d1_status, latency_ms: d1_latency },
            kv: CheckResult { status: kv_status, latency_ms: kv_latency },
        },
        site,
        stats,
    }))
}
