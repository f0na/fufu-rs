use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::sync::Arc;
use std::sync::OnceLock;
use worker::Env;

use chrono::Datelike;

use crate::auth::jwt::Claims;
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::handlers::umami::{UmamiClient, UmamiMetricItem, UmamiPageviewsResponse, UmamiStatsResponse, UmamiStatsValue};
use crate::kv::KvCache;
use crate::time;

/// 站点级启动时间戳，持久化在 Core 数据库 site_config 表中，跨 Worker 实例共享
/// 记录了站点首次部署的时间，Worker 重启不影响此值
static SITE_STARTED_AT: OnceLock<u64> = OnceLock::new();
/// 当前 Worker 实例启动时间戳，重启后重置
static INSTANCE_STARTED_AT: OnceLock<u64> = OnceLock::new();

// ═══════════════════════════════════════════════════════════════
//  公共统计（无需认证）
// ═══════════════════════════════════════════════════════════════

#[derive(Serialize)]
pub struct PublicStats {
    pub health: HealthOverview,
    pub active_visitors: i64,
    pub today: PeriodStats,
    pub last_30_days: PeriodStats,
    pub pageviews_timeline: Vec<TimeSeriesPoint>,
    pub deploy_info: DeployInfo,
}

#[derive(Serialize)]
pub struct PeriodStats {
    pub pageviews: i64,
    pub visitors: i64,
    pub visits: i64,
}

#[derive(Serialize)]
pub struct TimeSeriesPoint {
    pub date: String,
    pub pageviews: i64,
    pub sessions: i64,
}

/// GET /api/stats — 公共统计
#[worker::send]
pub async fn public_stats(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<PublicStats>> {
    let now = chrono::Utc::now();
    let today_start = timestamp_ms(now.date_naive().and_hms_opt(0, 0, 0).unwrap());
    let thirty_days_ago = timestamp_ms((now - chrono::Duration::days(30)).naive_utc());
    let now_ms = timestamp_ms(now.naive_utc());
    let day_unit = "day";

    let (deploy_info, health) = futures::join!(
        gather_deploy_info(&env),
        gather_health_checks(&env),
    );

    let umami = match UmamiClient::new(&env).await {
        Ok(c) => c,
        Err(_) => {
            return Ok(Json(PublicStats {
                health,
                active_visitors: 0,
                today: PeriodStats { pageviews: 0, visitors: 0, visits: 0 },
                last_30_days: PeriodStats { pageviews: 0, visitors: 0, visits: 0 },
                pageviews_timeline: vec![],
                deploy_info,
            }));
        }
    };

    let (active, today_stats, month_stats, pageviews) = futures::join!(
        umami.get_active_visitors(),
        umami.get_stats(today_start, now_ms),
        umami.get_stats(thirty_days_ago, now_ms),
        umami.get_pageviews(thirty_days_ago, now_ms, day_unit),
    );

    let active_visitors = active.unwrap_or(0);

    let today = match today_stats {
        Ok(s) => PeriodStats {
            pageviews: s.pageviews.value,
            visitors: s.visitors.value,
            visits: s.visits.value,
        },
        Err(_) => PeriodStats { pageviews: 0, visitors: 0, visits: 0 },
    };

    let last_30_days = match month_stats {
        Ok(s) => PeriodStats {
            pageviews: s.pageviews.value,
            visitors: s.visitors.value,
            visits: s.visits.value,
        },
        Err(_) => PeriodStats { pageviews: 0, visitors: 0, visits: 0 },
    };

    let pageviews_timeline = match pageviews {
        Ok(pv) => {
            let sessions_map: std::collections::HashMap<&str, i64> = pv
                .sessions
                .iter()
                .filter_map(|p| {
                    let date = p.x.split_once(' ').map(|(d, _)| d).unwrap_or(&p.x);
                    Some((date, p.y))
                })
                .collect();

            pv.pageviews
                .iter()
                .map(|p| {
                    let date = p.x.split_once(' ').map(|(d, _)| d).unwrap_or(&p.x);
                    TimeSeriesPoint {
                        date: date.to_string(),
                        pageviews: p.y,
                        sessions: *sessions_map.get(date).unwrap_or(&0),
                    }
                })
                .collect()
        }
        Err(_) => vec![],
    };

    Ok(Json(PublicStats {
        health,
        active_visitors,
        today,
        last_30_days,
        pageviews_timeline,
        deploy_info,
    }))
}

fn timestamp_ms(naive: chrono::NaiveDateTime) -> u64 {
    naive.and_utc().timestamp_millis() as u64
}

// ═══════════════════════════════════════════════════════════════
//  管理仪表盘（需登录）
// ═══════════════════════════════════════════════════════════════

#[derive(Serialize)]
pub struct DashboardData {
    // Umami 数据分析
    pub analytics: AnalyticsData,
    // 站点运行状态
    pub health: HealthSummary,
    pub stats: ModuleStats,
    // 部署信息
    pub deploy_info: DeployInfo,
    // 外部 API 接口状况
    pub external_apis: Vec<ExternalApiStatus>,
    // 数据库状况
    pub databases: Vec<DatabaseCheck>,
}

#[derive(Serialize)]
pub struct AnalyticsData {
    pub active_visitors: i64,
    pub today: PeriodStats,
    pub this_month: PeriodStats,
    pub last_30_days: PeriodStats,
    pub pageviews_timeline: Vec<TimeSeriesPoint>,
    pub top_pages: Vec<MetricItem>,
    pub top_referrers: Vec<MetricItem>,
    pub browsers: Vec<MetricItem>,
    pub os: Vec<MetricItem>,
    pub devices: Vec<MetricItem>,
    pub countries: Vec<MetricItem>,
}

#[derive(Serialize)]
pub struct MetricItem {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct DeployInfo {
    pub deployed_at_epoch: u64,
}

#[derive(Serialize)]
pub struct ExternalApiStatus {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
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
    pub version: &'static str,
    pub instance_started_at_epoch: u64,
    pub kv: CheckResult,
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

    fn degraded(msg: &str) -> Self {
        Self {
            status: format!("degraded: {}", msg),
            latency_ms: None,
        }
    }
}

#[derive(Serialize)]
pub struct HealthOverview {
    pub status: String,
    pub instance_started_at_epoch: u64,
    pub checks: HealthChecks,
}

#[derive(Serialize)]
pub struct HealthChecks {
    pub d1: CheckResult,
    pub kv: CheckResult,
}

#[derive(Serialize)]
pub struct ModuleStats {
    pub posts: u64,
    pub friends: u64,
    pub links: u64,
    pub galleries: u64,
    pub bangumi_records: u64,
}

// ─── 常量 ─────────────────────────────────────────────────────

const USER_AGENT: &str = "fufu-rs/1.0";

// ═══════════════════════════════════════════════════════════════
//  GET /api/auth/dashboard — 管理仪表盘
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn dashboard(
    _claims: Claims,
    State(env): State<Arc<Env>>,
) -> AppResult<Json<DashboardData>> {
    let now = chrono::Utc::now();
    let today_start = timestamp_ms(now.date_naive().and_hms_opt(0, 0, 0).unwrap());
    let month_start = timestamp_ms(
        now.date_naive()
            .with_day(1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
    );
    let thirty_days_ago = timestamp_ms((now - chrono::Duration::days(30)).naive_utc());
    let now_ms = timestamp_ms(now.naive_utc());
    let day_unit = "day";

    // ── Umami 并行请求 ──────────────────────────────────────
    let umami = match UmamiClient::new(&env).await {
        Ok(u) => u,
        Err(_) => return Err(AppError::Internal("Umami 未配置".into())),
    };

    let (
        active,
        today_s,
        month_s,
        last30_s,
        pv,
        pages,
        refs,
        browsers,
        os,
        devices,
        countries,
    ) = futures::join!(
        umami.get_active_visitors(),
        umami.get_stats(today_start, now_ms),
        umami.get_stats(month_start, now_ms),
        umami.get_stats(thirty_days_ago, now_ms),
        umami.get_pageviews(thirty_days_ago, now_ms, day_unit),
        umami.get_metrics("url", thirty_days_ago, now_ms, 20),
        umami.get_metrics("referrer", thirty_days_ago, now_ms, 10),
        umami.get_metrics("browser", thirty_days_ago, now_ms, 10),
        umami.get_metrics("os", thirty_days_ago, now_ms, 10),
        umami.get_metrics("device", thirty_days_ago, now_ms, 10),
        umami.get_metrics("country", thirty_days_ago, now_ms, 10),
    );

    let active_visitors = active.unwrap_or(0);

    let today = s_to_period(today_s.unwrap_or_else(|_| default_stats()));
    let this_month = s_to_period(month_s.unwrap_or_else(|_| default_stats()));
    let last_30_days = s_to_period(last30_s.unwrap_or_else(|_| default_stats()));

    let pageviews_timeline = pageviews_to_timeline(pv.unwrap_or_else(|_| default_pageviews()));

    fn metric_items(items: Result<Vec<UmamiMetricItem>, AppError>) -> Vec<MetricItem> {
        items
            .unwrap_or_default()
            .into_iter()
            .map(|m| MetricItem {
                name: m.x,
                count: m.y,
            })
            .collect()
    }

    let analytics = AnalyticsData {
        active_visitors,
        today,
        this_month,
        last_30_days,
        pageviews_timeline,
        top_pages: metric_items(pages),
        top_referrers: metric_items(refs),
        browsers: metric_items(browsers),
        os: metric_items(os),
        devices: metric_items(devices),
        countries: metric_items(countries),
    };

    // ── 其余数据并行 ────────────────────────────────────────
    let (health, stats, deploy_info, databases, external_apis) = futures::join!(
        gather_health(&env),
        gather_stats(&env),
        gather_deploy_info(&env),
        check_all_databases(&env),
        check_external_apis(&env),
    );

    Ok(Json(DashboardData {
        analytics,
        health,
        stats,
        deploy_info,
        external_apis,
        databases,
    }))
}

fn s_to_period(s: UmamiStatsResponse) -> PeriodStats {
    PeriodStats {
        pageviews: s.pageviews.value,
        visitors: s.visitors.value,
        visits: s.visits.value,
    }
}

fn default_stats() -> UmamiStatsResponse {
    UmamiStatsResponse {
        pageviews: UmamiStatsValue { value: 0, prev: 0 },
        visitors: UmamiStatsValue { value: 0, prev: 0 },
        visits: UmamiStatsValue { value: 0, prev: 0 },
        bounces: UmamiStatsValue { value: 0, prev: 0 },
        totaltime: UmamiStatsValue { value: 0, prev: 0 },
    }
}

fn default_pageviews() -> UmamiPageviewsResponse {
    UmamiPageviewsResponse {
        pageviews: vec![],
        sessions: vec![],
    }
}

fn pageviews_to_timeline(pv: UmamiPageviewsResponse) -> Vec<TimeSeriesPoint> {
    let sessions_map: std::collections::HashMap<&str, i64> = pv
        .sessions
        .iter()
        .filter_map(|p| {
            let date = p.x.split_once(' ').map(|(d, _)| d).unwrap_or(&p.x);
            Some((date, p.y))
        })
        .collect();

    pv.pageviews
        .iter()
        .map(|p| {
            let date = p.x.split_once(' ').map(|(d, _)| d).unwrap_or(&p.x);
            TimeSeriesPoint {
                date: date.to_string(),
                pageviews: p.y,
                sessions: *sessions_map.get(date).unwrap_or(&0),
            }
        })
        .collect()
}

// ─── 站点状态检测 ─────────────────────────────────────────────

async fn get_started_at(env: &Arc<Env>) -> u64 {
    if let Some(&ts) = SITE_STARTED_AT.get() {
        return ts;
    }

    let now = time::now_epoch();
    let db = match get_db(env, Db::Core) {
        Ok(d) => d,
        Err(_) => return now,
    };

    let key = "site_started_at";
    let result = match db
        .prepare("SELECT value FROM site_config WHERE key = ?")
        .bind(&[key.into()])
    {
        Ok(stmt) => stmt.all().await.ok(),
        Err(_) => None,
    };

    let ts = match result {
        Some(r) => {
            let rows: Vec<serde_json::Value> = r.results().unwrap_or_default();
            match rows.first().and_then(|r| r.get("value")).and_then(|v| v.as_str()) {
                Some(v) => v.parse::<u64>().unwrap_or(now),
                None => {
                    let _ = insert_site_started_at(&db, key, now).await;
                    now
                }
            }
        }
        None => {
            let _ = insert_site_started_at(&db, key, now).await;
            now
        }
    };

    let _ = SITE_STARTED_AT.set(ts);
    ts
}

async fn insert_site_started_at(db: &worker::D1Database, key: &str, epoch: u64) -> Result<(), ()> {
    let now_iso = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    match db.prepare("INSERT INTO site_config (key, value, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind(&[key.into(), epoch.to_string().into(), now_iso.clone().into(), now_iso.into()])
    {
        Ok(stmt) => match stmt.run().await {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        },
        Err(_) => Err(()),
    }
}

async fn gather_health(_env: &Arc<Env>) -> HealthSummary {
    let now = time::now_epoch();
    let instance_started_at = INSTANCE_STARTED_AT.get_or_init(|| now);

    HealthSummary {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION"),
        instance_started_at_epoch: *instance_started_at,
        kv: CheckResult {
            status: "ok".into(),
            latency_ms: None,
        },
    }
}

async fn gather_deploy_info(env: &Arc<Env>) -> DeployInfo {
    DeployInfo {
        deployed_at_epoch: get_started_at(env).await,
    }
}

// ─── 健康检查（合并自原 /api/health）─────────────────────────

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

async fn gather_health_checks(env: &Arc<Env>) -> HealthOverview {
    let (d1, kv) = futures::join!(
        async {
            match check_d1(env).await {
                Ok(ms) => CheckResult::ok(ms),
                Err(e) => CheckResult::degraded(&e.to_string()),
            }
        },
        async {
            match check_kv(env).await {
                Ok(ms) => CheckResult::ok(ms),
                Err(e) => CheckResult::degraded(&e.to_string()),
            }
        },
    );

    let now = time::now_epoch();
    let instance_started_at = INSTANCE_STARTED_AT.get_or_init(|| now);

    let status = if d1.status == "ok" && kv.status == "ok" {
        "ok"
    } else {
        "degraded"
    };

    HealthOverview {
        status: status.into(),
        instance_started_at_epoch: *instance_started_at,
        checks: HealthChecks { d1, kv },
    }
}

// ─── 外部 API 健康检测 ────────────────────────────────────────

async fn check_external_api(_env: &Arc<Env>, name: &str, url: &str) -> ExternalApiStatus {
    let client = reqwest::Client::new();
    let start = time::now_epoch();

    match client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
    {
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

// ─── 模块统计 ─────────────────────────────────────────────────

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
