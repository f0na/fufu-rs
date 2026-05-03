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
    // 部署信息
    deploy_info: DeployInfo,
    // 外部 API 接口状况
    external_apis: Vec<ExternalApiStatus>,
    // Worker 运行指标
    worker_metrics: WorkerMetrics,
    // 数据库状况
    databases: Vec<DatabaseCheck>,
}

#[derive(Serialize)]
pub struct DeployInfo {
    pub deployed_at: String,
    pub deployed_at_epoch: u64,
    pub uptime_seconds: u64,
    pub uptime_human: String,
}

#[derive(Serialize)]
pub struct ExternalApiStatus {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct WorkerMetrics {
    pub total_requests: i64,
    pub error_count: i64,
    pub error_rate_pct: f64,
    pub avg_cpu_time_ms: f64,
    pub errors_by_path: Vec<PathError>,
}

#[derive(Serialize)]
pub struct PathError {
    pub path: String,
    pub status_code: i64,
    pub count: i64,
}

#[derive(Serialize)]
pub struct DatabaseCheck {
    pub name: &'static str,
    pub binding: &'static str,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct HealthSummary {
    pub status: String,
    pub uptime: u64,
    pub version: &'static str,
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

const TODAY_QUERY: &str = r#"
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

const MONTH_QUERY: &str = r#"
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

const TOTAL_QUERY: &str = r#"
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
                        edgeDurationMs
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

const ERROR_BREAKDOWN_QUERY: &str = r#"
    query ($zoneTag: String!, $startDate: String!) {
        viewer {
            zones(filter: {zoneTag: $zoneTag}) {
                httpRequestsAdaptiveGroups(
                    limit: 100
                    filter: {date_geq: $startDate, edgeResponseStatus_gt: 399}
                    orderBy: [count_DESC]
                ) {
                    dimensions {
                        clientRequestPath
                        edgeResponseStatus
                    }
                    count
                }
            }
        }
    }
"#;

// ─── 工具函数 ─────────────────────────────────────────────────

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

fn epoch_to_cst_str(epoch: u64) -> String {
    use chrono::{DateTime, FixedOffset};
    match DateTime::from_timestamp(epoch as i64, 0) {
        Some(dt) => {
            let cst = FixedOffset::east_opt(8 * 3600).expect("CST offset");
            dt.with_timezone(&cst).format("%Y-%m-%d %H:%M:%S").to_string()
        }
        None => "unknown".into(),
    }
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}天", days));
    }
    if hours > 0 {
        parts.push(format!("{}小时", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}分", minutes));
    }
    parts.push(format!("{}秒", secs));
    parts.concat()
}

// ─── Cloudflare GraphQL ──────────────────────────────────────

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

async fn query_error_breakdown(api_token: &str, zone_id: &str, start_30d: &str) -> serde_json::Value {
    query_cf_graphql(
        api_token,
        ERROR_BREAKDOWN_QUERY,
        &serde_json::json!({
            "zoneTag": zone_id,
            "startDate": start_30d,
        }),
    )
    .await
    .unwrap_or(serde_json::Value::Null)
}

fn parse_error_breakdown(data: &serde_json::Value) -> Vec<PathError> {
    let mut errors = Vec::new();
    if let Some(groups) = data["data"]["viewer"]["zones"][0]
        .get("httpRequestsAdaptiveGroups")
        .and_then(|v| v.as_array())
    {
        for group in groups {
            let path = group["dimensions"]["clientRequestPath"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let status_code = group["dimensions"]["edgeResponseStatus"]
                .as_i64()
                .unwrap_or(0);
            let count = group["count"].as_i64().unwrap_or(0);
            errors.push(PathError { path, status_code, count });
        }
    }
    errors
}

// ─── 外部 API 健康检测 ────────────────────────────────────────

async fn check_external_api(_env: &Arc<Env>, name: &str, url: &str) -> ExternalApiStatus {
    let client = reqwest::Client::new();
    let start = time::now_epoch();

    match client.get(url).header("User-Agent", USER_AGENT).send().await {
        Ok(resp) => {
            let latency = time::now_epoch().saturating_sub(start).max(1);
            let status = if resp.status().is_success() || resp.status().is_redirection() {
                "ok"
            } else {
                "degraded"
            };
            ExternalApiStatus {
                name: name.to_string(),
                status: status.into(),
                latency_ms: Some(latency),
            }
        }
        Err(_) => ExternalApiStatus {
            name: name.to_string(),
            status: "error".into(),
            latency_ms: None,
        },
    }
}

async fn check_external_apis(env: &Arc<Env>) -> Vec<ExternalApiStatus> {
    let targets = [
        ("Bangumi", "https://api.bgm.tv/ping"),
        ("Anime Garden", "https://anime-garden.app"),
        ("Baidu Translate", "https://fanyi-api.baidu.com"),
    ];

    let futures: Vec<_> = targets
        .iter()
        .map(|(name, url)| check_external_api(env, name, url))
        .collect();

    futures::future::join_all(futures).await
}

// ─── 数据库健康检测 ───────────────────────────────────────────

fn db_display_name(db: Db) -> &'static str {
    match db {
        Db::Core => "Core",
        Db::Posts => "Posts",
        Db::Media => "Media",
        Db::Bangumi => "Bangumi",
        Db::Social => "Social",
        Db::Likes => "Likes",
        Db::Legal => "Legal",
        Db::Auth => "Auth",
    }
}

async fn check_single_database(env: &Arc<Env>, db_variant: Db) -> DatabaseCheck {
    let binding = db_variant.binding_name();
    let name = db_display_name(db_variant);

    let db = match get_db(env, db_variant) {
        Ok(d) => d,
        Err(_) => {
            return DatabaseCheck {
                name,
                binding,
                status: "error".into(),
                latency_ms: None,
            }
        }
    };

    let start = time::now_epoch();
    let stmt = db.prepare("SELECT 1 as ok");
    let stmt = match stmt.bind(&[]) {
        Ok(s) => s,
        Err(_) => {
            return DatabaseCheck {
                name,
                binding,
                status: "error".into(),
                latency_ms: None,
            }
        }
    };

    match stmt.all().await {
        Ok(_) => DatabaseCheck {
            name,
            binding,
            status: "ok".into(),
            latency_ms: Some(time::now_epoch().saturating_sub(start).max(1)),
        },
        Err(_) => DatabaseCheck {
            name,
            binding,
            status: "error".into(),
            latency_ms: None,
        },
    }
}

async fn check_all_databases(env: &Arc<Env>) -> Vec<DatabaseCheck> {
    let futures: Vec<_> = Db::all()
        .iter()
        .map(|db| check_single_database(env, *db))
        .collect();
    futures::future::join_all(futures).await
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
    let start_30d = (chrono::Utc::now() - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();

    // ── 并行执行所有独立请求 ──────────────────────────────────
    let tv_today = serde_json::json!({
        "zoneTag": zone_id,
        "startOfDay": format!("{}T00:00:00Z", today),
    });
    let tv_month = serde_json::json!({
        "zoneTag": zone_id,
        "startOfMonth": month_start,
    });
    let tv_total = serde_json::json!({
        "zoneTag": zone_id,
        "startDate": start_30d,
    });

    let (
        today_res,
        month_res,
        total_res,
        err_res,
        health,
        stats,
        deploy_info,
        databases,
        external_apis,
    ) = futures::join!(
        query_cf_graphql(&api_token, TODAY_QUERY, &tv_today),
        query_cf_graphql(&api_token, MONTH_QUERY, &tv_month),
        query_cf_graphql(&api_token, TOTAL_QUERY, &tv_total),
        query_error_breakdown(&api_token, &zone_id, &start_30d),
        gather_health(&env),
        gather_stats(&env),
        gather_deploy_info(&env),
        check_all_databases(&env),
        check_external_apis(&env),
    );

    // ── 解析今日数据 ──────────────────────────────────────────
    let today_zone = &today_res?["data"]["viewer"]["zones"][0];

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

    // ── 解析本月数据 ──────────────────────────────────────────
    let month_zone = &month_res?["data"]["viewer"]["zones"][0];

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

    // ── 解析总数据 + 状态码 ──────────────────────────────────
    let total_data = total_res?;
    let total_zone = &total_data["data"]["viewer"]["zones"][0];

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

    // ── Worker 运行指标 ──────────────────────────────────────
    let worker_metrics = gather_worker_metrics(&total_data, &err_res, total_requests);

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
        stats,
        health,
        deploy_info,
        external_apis,
        worker_metrics,
        databases,
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

    // KV 检测（D1 检测移至 databases 字段）
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
        kv: CheckResult { status: kv_status, latency_ms: kv_latency },
    }
}

async fn gather_deploy_info(env: &Arc<Env>) -> DeployInfo {
    let started_at = get_started_at(env).await;
    let now = time::now_epoch();
    let uptime_secs = now.saturating_sub(started_at);

    DeployInfo {
        deployed_at: epoch_to_cst_str(started_at),
        deployed_at_epoch: started_at,
        uptime_seconds: uptime_secs,
        uptime_human: format_uptime(uptime_secs),
    }
}

fn gather_worker_metrics(
    total_data: &serde_json::Value,
    error_data: &serde_json::Value,
    total_requests: i64,
) -> WorkerMetrics {
    // 从 total_data 解析 edgeDurationMs（30 天总计 CPU 时间）
    let total_cpu_ms: f64 = total_data["data"]["viewer"]["zones"][0]
        .get("httpRequests1dGroups")
        .and_then(|v| v.as_array())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g["sum"]["edgeDurationMs"].as_f64())
                .sum::<f64>()
        })
        .unwrap_or(0.0);

    let avg_cpu_time_ms = if total_requests > 0 {
        ((total_cpu_ms / total_requests as f64) * 100.0).round() / 100.0
    } else {
        0.0
    };

    // 解析错误分布
    let errors_by_path = parse_error_breakdown(error_data);
    let error_count: i64 = errors_by_path.iter().map(|e| e.count).sum();

    let error_rate_pct = if total_requests > 0 {
        ((error_count as f64 / total_requests as f64) * 100.0 * 100.0).round() / 100.0
    } else {
        0.0
    };

    WorkerMetrics {
        total_requests,
        error_count,
        error_rate_pct,
        avg_cpu_time_ms,
        errors_by_path,
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
