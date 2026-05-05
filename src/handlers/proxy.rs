use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use worker::Env;

use crate::error::{AppError, AppResult};
use crate::kv::KvCache;

// ─── 常量 ─────────────────────────────────────────────────────

const USER_AGENT: &str = "fufu-rs/1.0";

// ─── 请求类型 ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchInput {
    keyword: String,
    #[allow(dead_code)]
    sort: Option<String>,
    #[allow(dead_code)]
    filter: Option<serde_json::Value>,
    #[allow(dead_code)]
    limit: Option<i32>,
    #[allow(dead_code)]
    offset: Option<i32>,
    #[allow(dead_code)]
    r#type: Option<i32>,
}

#[derive(Deserialize)]
pub struct SubjectIdParam {
    pub id: String,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn build_cache_key(prefix: &str, params: &HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = params.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let query: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    if query.is_empty() {
        prefix.to_string()
    } else {
        format!("{}:{}", prefix, query.join("&"))
    }
}

async fn fetch_json(url: &str) -> AppResult<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
    Ok(json)
}

async fn post_json(url: &str, body: &serde_json::Value) -> AppResult<serde_json::Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .header("User-Agent", USER_AGENT)
        .json(body)
        .send()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
    Ok(json)
}

async fn cached_fetch(
    kv: &KvCache,
    cache_key: &str,
    ttl: u64,
    url: &str,
) -> AppResult<serde_json::Value> {
    if let Some(cached) = kv.get_json::<serde_json::Value>(cache_key).await? {
        return Ok(cached);
    }
    let data = fetch_json(url).await?;
    kv.put_json(cache_key, &data, ttl).await?;
    Ok(data)
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/bangumi/search — Bangumi 搜索
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn bangumi_search(
    State(env): State<Arc<Env>>,
    Json(body): Json<SearchInput>,
) -> AppResult<Json<serde_json::Value>> {
    let kv = KvCache::new(&env)?;
    let cache_key = {
        let mut parts = vec![body.keyword.clone()];
        if let Some(sort) = &body.sort { parts.push(format!("s={}", sort)); }
        if let Some(filter) = &body.filter { parts.push(format!("f={}", filter)); }
        if let Some(limit) = body.limit { parts.push(format!("l={}", limit)); }
        if let Some(offset) = body.offset { parts.push(format!("o={}", offset)); }
        format!("bangumi:search:{}", parts.join(":"))
    };

    if let Some(cached) = kv.get_json::<serde_json::Value>(&cache_key).await? {
        return Ok(Json(cached));
    }

    let mut url = "https://api.bgm.tv/v0/search/subjects".to_string();
    let mut query_params: Vec<String> = Vec::new();
    if let Some(limit) = body.limit {
        query_params.push(format!("limit={}", limit));
    }
    if let Some(offset) = body.offset {
        query_params.push(format!("offset={}", offset));
    }
    if !query_params.is_empty() {
        url.push('?');
        url.push_str(&query_params.join("&"));
    }

    let mut payload = serde_json::json!({
        "keyword": body.keyword,
    });
    if let Some(sort) = body.sort {
        payload["sort"] = serde_json::json!(sort);
    }
    if let Some(filter) = &body.filter {
        payload["filter"] = filter.clone();
    }
    // 兼容旧的 type 参数（转为 filter.type）
    if let Some(type_val) = body.r#type {
        if body.filter.is_none() {
            payload["filter"] = serde_json::json!({ "type": [type_val] });
        }
    }

    let data = post_json(&url, &payload).await?;
    kv.put_json(&cache_key, &data, 7200).await?; // 2小时缓存
    Ok(Json(data))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/bangumi/subjects/:id — 条目详情
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn bangumi_subject(
    State(env): State<Arc<Env>>,
    Path(param): Path<SubjectIdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let kv = KvCache::new(&env)?;
    let cache_key = format!("bangumi:subject:{}", param.id);
    let url = format!("https://api.bgm.tv/v0/subjects/{}", param.id);

    cached_fetch(&kv, &cache_key, 86400, &url).await.map(Json)
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/bangumi/calendar — 每日放送
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn bangumi_calendar(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<serde_json::Value>> {
    let kv = KvCache::new(&env)?;
    let cache_key = "bangumi:calendar";
    let url = "https://api.bgm.tv/calendar";

    cached_fetch(&kv, cache_key, 14400, url).await.map(Json) // 4小时缓存（日更数据）
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/bangumi/browse — 浏览条目
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn bangumi_browse(
    State(env): State<Arc<Env>>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    let kv = KvCache::new(&env)?;
    let cache_key = build_cache_key("bangumi:browse", &params);

    if let Some(cached) = kv.get_json::<serde_json::Value>(&cache_key).await? {
        return Ok(Json(cached));
    }

    let query_string: String = {
        let mut pairs: Vec<_> = params.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    };
    let url = format!("https://api.bgm.tv/v0/subjects?{}", query_string);
    let data = fetch_json(&url).await?;
    kv.put_json(&cache_key, &data, 7200).await?; // 2小时缓存
    Ok(Json(data))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/anime-garden/resources — AnimeGarden 资源列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn anime_garden_resources(
    State(env): State<Arc<Env>>,
    Query(params): Query<HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    let kv = KvCache::new(&env)?;
    let cache_key = build_cache_key("anime-garden:resources", &params);

    if let Some(cached) = kv.get_json::<serde_json::Value>(&cache_key).await? {
        return Ok(Json(cached));
    }

    let query_string: String = {
        let mut pairs: Vec<_> = params.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    };
    let url = format!("https://api.animes.garden/resources?{}", query_string);
    let data = fetch_json(&url).await?;
    kv.put_json(&cache_key, &data, 7200).await?; // 2小时缓存
    Ok(Json(data))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/translate — 百度翻译（中译英）
// ═══════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct TranslateInput {
    text: String,
    #[allow(dead_code)]
    from: Option<String>,
    #[allow(dead_code)]
    to: Option<String>,
}

#[worker::send]
pub async fn translate(
    State(env): State<Arc<Env>>,
    Json(body): Json<TranslateInput>,
) -> AppResult<Json<serde_json::Value>> {
    let appid = env
        .var("BAIDU_TRANSLATE_APPID")
        .map_err(|_| AppError::Internal("BAIDU_TRANSLATE_APPID 未配置".into()))?
        .to_string();
    let secret = env
        .var("BAIDU_TRANSLATE_SECRET")
        .map_err(|_| AppError::Internal("BAIDU_TRANSLATE_SECRET 未配置".into()))?
        .to_string();

    let from = body.from.unwrap_or_else(|| "auto".to_string());
    let to = body.to.unwrap_or_else(|| "en".to_string());
    let salt = format!("{}", uuid::Uuid::now_v7().as_u128() % 100000);
    let sign = format!(
        "{:x}",
        md5::compute(format!("{}{}{}{}", appid, body.text, salt, secret))
    );

    let client = reqwest::Client::new();
    let resp = client
        .post("https://fanyi-api.baidu.com/api/trans/vip/translate")
        .header("User-Agent", USER_AGENT)
        .form(&[
            ("q", body.text.as_str()),
            ("from", from.as_str()),
            ("to", to.as_str()),
            ("appid", appid.as_str()),
            ("salt", salt.as_str()),
            ("sign", sign.as_str()),
        ])
        .send()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
    Ok(Json(json))
}
