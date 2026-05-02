use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::Claims;
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::kv::KvCache;
use crate::time;

// ─── 响应类型 ─────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DashboardData {
    // Cloudflare Analytics
    today: TodayStats,
    this_month: MonthStats,
    total: TotalStats,
    status_codes: StatusCodeStats,
    // 站点运行状态
    health: HealthSummary,
    stats: ModuleStats,
}

#[derive(Serialize)]
pub struct HealthSummary {
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
pub struct ModuleStats {
    pub posts: u64,
    pub friends: u64,
    pub links: u64,
    pub galleries: u64,
    pub bangumi_records: u64,
}

#[derive(Serialize)]
struct TodayStats {
    requests: i64,
    bandwidth: String,
    avg_duration_ms: i64,
}

#[derive(Serialize)]
struct MonthStats {
    requests: i64,
    bandwidth: String,
}

#[derive(Serialize)]
struct TotalStats {
    requests: i64,
    bandwidth: String,
}

#[derive(Serialize)]
struct StatusCodeStats {
    #[serde(rename = "2xx")]
    xx2: i64,
    #[serde(rename = "4xx")]
    xx4: i64,
    #[serde(rename = "5xx")]
    xx5: i64,
}

// ─── 常量 ─────────────────────────────────────────────────────

const CF_API: &str = "https://api.cloudflare.com/client/v4/graphql";
const USER_AGENT: &str = "fufu-rs/1.0";

fn format_bytes(bytes: f64) -> String {
    if bytes >= 1_073_741_824.0 {
        format!("{:.1} GB", bytes / 1_073_741_824.0)
    } else if bytes >= 1_048_576.0 {
        format!("{:.1} MB", bytes / 1_048_576.0)
    } else if bytes >= 1024.0 {
        format!("{:.1} KB", bytes / 1024.0)
    } else {
        format!("{} B", bytes as i64)
    }
}

async fn query_cf_graphql(
    api_token: &str,
    query: &str,
    variables: &serde_json::Value,
) -> AppResult<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(CF_API)
        .header("Authorization", format!("Bearer {}", api_token))
        .header("User-Agent", USER_AGENT)
        .json(&serde_json::json!({
            "query": query,
            "variables": variables,
        }))
        .send()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;

    // 检查 GraphQL 错误
    if json.get("errors").and_then(|e| e.as_array()).map_or(false, |e| !e.is_empty()) {
        return Err(AppError::ExternalApiFailure("Cloudflare Analytics API 返回错误".into()));
    }

    Ok(json)
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/auth/dashboard — 仪表盘
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn dashboard(
    _claims: Claims,
    State(env): State<Arc<Env>>,
) -> AppResult<Json<DashboardData>> {
    let api_token = env
        .var("CF_API_TOKEN")
        .map_err(|_| AppError::Internal("CF_API_TOKEN 未配置".into()))?
        .to_string();
    let zone_id = env
        .var("CF_ZONE_ID")
        .map_err(|_| AppError::Internal("CF_ZONE_ID 未配置".into()))?
        .to_string();

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let month_start = chrono::Utc::now()
        .format("%Y-%m-01")
        .to_string();

    // 1) 今日数据（按小时聚合）
    let today_query = r#"
        query ($zoneTag: String!, $startOfDay: String!) {
            viewer {
                zones(filter: {zoneTag: $zoneTag}) {
                    httpRequests1hGroups(
                        limit: 24
                        filter: {datetime_geq: $startOfDay}
                        orderBy: [datetime_DESC]
                    ) {
                        sum {
                            requests
                            bytes
                            edgeDurationMs
                        }
                    }
                }
            }
        }
    "#;

    let today_result = query_cf_graphql(
        &api_token,
        today_query,
        &serde_json::json!({
            "zoneTag": zone_id,
            "startOfDay": format!("{}T00:00:00Z", today),
        }),
    )
    .await?;

    let today_zone = &today_result["data"]["viewer"]["zones"][0];

    let today_requests: i64 = today_zone
        .get("httpRequests1hGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["requests"].as_f64())
                .sum::<f64>() as i64
        })
        .unwrap_or(0);

    let today_bytes: f64 = today_zone
        .get("httpRequests1hGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["bytes"].as_f64())
                .sum::<f64>()
        })
        .unwrap_or(0.0);

    let today_duration: f64 = today_zone
        .get("httpRequests1hGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            let total_duration: f64 = groups
                .iter()
                .filter_map(|g| g["sum"]["edgeDurationMs"].as_f64())
                .sum::<f64>();
            let count = groups
                .iter()
                .filter(|g| g["sum"]["edgeDurationMs"].as_f64().unwrap_or(0.0) > 0.0)
                .count() as f64;
            if count > 0.0 {
                total_duration / count
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    // 2) 本月数据（按天聚合）
    let month_query = r#"
        query ($zoneTag: String!, $startOfMonth: String!) {
            viewer {
                zones(filter: {zoneTag: $zoneTag}) {
                    httpRequests1dGroups(
                        limit: 31
                        filter: {date_geq: $startOfMonth}
                        orderBy: [date_DESC]
                    ) {
                        sum {
                            requests
                            bytes
                        }
                    }
                }
            }
        }
    "#;

    let month_result = query_cf_graphql(
        &api_token,
        month_query,
        &serde_json::json!({
            "zoneTag": zone_id,
            "startOfMonth": month_start,
        }),
    )
    .await?;

    let month_zone = &month_result["data"]["viewer"]["zones"][0];

    let month_requests: i64 = month_zone
        .get("httpRequests1dGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["requests"].as_f64())
                .sum::<f64>() as i64
        })
        .unwrap_or(0);

    let month_bytes: f64 = month_zone
        .get("httpRequests1dGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["bytes"].as_f64())
                .sum::<f64>()
        })
        .unwrap_or(0.0);

    // 3) 总数据 + 状态码（最近30天）
    let total_query = r#"
        query ($zoneTag: String!, $startDate: String!) {
            viewer {
                zones(filter: {zoneTag: $zoneTag}) {
                    httpRequests1dGroups(
                        limit: 30
                        filter: {date_geq: $startDate}
                        orderBy: [date_DESC]
                    ) {
                        sum {
                            requests
                            bytes
                        }
                    }
                    httpRequestsAdaptiveGroups(
                        limit: 10
                        filter: {date_geq: $startDate}
                        orderBy: [count_DESC]
                    ) {
                        dimensions {
                            statusCode
                        }
                        count
                    }
                }
            }
        }
    "#;

    let start_30d = (chrono::Utc::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();

    let total_result = query_cf_graphql(
        &api_token,
        total_query,
        &serde_json::json!({
            "zoneTag": zone_id,
            "startDate": start_30d,
        }),
    )
    .await?;

    let total_zone = &total_result["data"]["viewer"]["zones"][0];

    let total_requests: i64 = total_zone
        .get("httpRequests1dGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["requests"].as_f64())
                .sum::<f64>() as i64
        })
        .unwrap_or(0);

    let total_bytes: f64 = total_zone
        .get("httpRequests1dGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["bytes"].as_f64())
                .sum::<f64>()
        })
        .unwrap_or(0.0);

    // 聚合状态码
    let mut sc_2xx: i64 = 0;
    let mut sc_4xx: i64 = 0;
    let mut sc_5xx: i64 = 0;

    if let Some(groups) = total_zone
        .get("httpRequestsAdaptiveGroups")
        .and_then(|v| v.as_array())
    {
        for group in groups {
            let code = group["dimensions"]["statusCode"]
                .as_i64()
                .unwrap_or(0);
            let count = group["count"].as_i64().unwrap_or(0);
            match code / 100 {
                2 => sc_2xx += count,
                4 => sc_4xx += count,
                5 => sc_5xx += count,
                _ => {}
            }
        }
    }

    Ok(Json(DashboardData {
        today: TodayStats {
            requests: today_requests,
            bandwidth: format_bytes(today_bytes),
            avg_duration_ms: today_duration as i64,
        },
        this_month: MonthStats {
            requests: month_requests,
            bandwidth: format_bytes(month_bytes),
        },
        total: TotalStats {
            requests: total_requests,
            bandwidth: format_bytes(total_bytes),
        },
        status_codes: StatusCodeStats {
            xx2: sc_2xx,
            xx4: sc_4xx,
            xx5: sc_5xx,
        },
        // 站点运行状态
        health: gather_health(&env).await,
        stats: gather_stats(&env).await,
    }))
}

// ─── 站点状态检测 ─────────────────────────────────────────────
async fn get_started_at(env: &Arc<Env>) -> u64 {
    let kv = match KvCache::new(env) {
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

async fn gather_health(env: &Arc<Env>) -> HealthSummary {
    let started_at = get_started_at(env).await;
    let now = time::now_epoch();
    let uptime_secs = now.saturating_sub(started_at);

    // D1 检测
    let d1_ok = {
        let db = get_db(env, Db::Core).ok();
        match db {
            Some(d) => {
                let stmt = d.prepare("SELECT 1 as ok");
                match stmt.bind(&[]) {
                    Ok(s) => s.all().await.is_ok(),
                    Err(_) => false,
                }
            }
            None => false,
        }
    };
    let (d1_status, d1_latency) = if d1_ok {
        ("ok".into(), None)
    } else {
        ("error".into(), None)
    };

    // KV 检测
    let (kv_status, kv_latency) = match KvCache::new(env) {
        Ok(kv) => {
            let start = time::now_epoch();
            match kv.get_str("__health_check").await {
                Ok(_) => ("ok".into(), Some(time::now_epoch().saturating_sub(start).max(1))),
                Err(_) => ("error".into(), None),
            }
        }
        Err(_) => ("error".into(), None),
    };

    HealthSummary {
        status: "ok".into(),
        uptime: uptime_secs,
        version: env!("CARGO_PKG_VERSION"),
        d1: CheckResult { status: d1_status, latency_ms: d1_latency },
        kv: CheckResult { status: kv_status, latency_ms: kv_latency },
    }
}

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

async fn gather_stats(env: &Arc<Env>) -> ModuleStats {
    ModuleStats {
        posts: count_table(env, Db::Posts, "posts").await,
        friends: count_table(env, Db::Social, "friends").await,
        links: count_table(env, Db::Social, "links").await,
        galleries: count_table(env, Db::Media, "galleries").await,
        bangumi_records: count_table(env, Db::Bangumi, "bangumi_records").await,
    }
}
