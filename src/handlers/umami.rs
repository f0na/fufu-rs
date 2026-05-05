use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::Mutex;
use worker::Env;

use crate::error::{AppError, AppResult};

const USER_AGENT: &str = "fufu-rs/1.0";

// ─── Umami API 响应类型 ────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct UmamiStatsValue {
    pub value: i64,
    pub prev: i64,
}

#[derive(Deserialize, Debug)]
pub struct UmamiStatsResponse {
    pub pageviews: UmamiStatsValue,
    pub visitors: UmamiStatsValue,
    pub visits: UmamiStatsValue,
    pub bounces: UmamiStatsValue,
    pub totaltime: UmamiStatsValue,
}

#[derive(Deserialize, Debug)]
pub struct UmamiActiveVisitors {
    pub x: i64,
}

#[derive(Deserialize, Debug)]
pub struct UmamiTimeSeriesPoint {
    pub x: String,
    pub y: i64,
}

#[derive(Deserialize, Debug)]
pub struct UmamiPageviewsResponse {
    pub pageviews: Vec<UmamiTimeSeriesPoint>,
    pub sessions: Vec<UmamiTimeSeriesPoint>,
}

#[derive(Deserialize, Debug)]
pub struct UmamiMetricItem {
    pub x: String,
    pub y: i64,
}

// ─── 登录响应 ──────────────────────────────────────────────────

#[derive(Serialize)]
struct LoginPayload<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
}

// ─── 全局 Token 缓存 ──────────────────────────────────────────

static TOKEN_CACHE: std::sync::OnceLock<Mutex<TokenCache>> = std::sync::OnceLock::new();

struct TokenCache {
    token: String,
}

fn get_or_init_cache() -> &'static Mutex<TokenCache> {
    TOKEN_CACHE.get_or_init(|| Mutex::new(TokenCache { token: String::new() }))
}

// ─── Umami 客户端 ──────────────────────────────────────────────

pub struct UmamiClient {
    base_url: String,
    username: String,
    password: String,
    website_id: String,
}

impl UmamiClient {
    pub async fn new(env: &Arc<Env>) -> AppResult<Self> {
        let base_url = env
            .var("UMAMI_API_URL")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "https://analytics.fufu.moe".to_string());

        let username = env
            .var("UMAMI_USERNAME")
            .map_err(|_| AppError::Internal("UMAMI_USERNAME 未配置".into()))?
            .to_string();

        let password = env
            .var("UMAMI_PASSWORD")
            .map_err(|_| AppError::Internal("UMAMI_PASSWORD 未配置".into()))?
            .to_string();

        let website_id = env
            .var("UMAMI_WEBSITE_ID")
            .map_err(|_| AppError::Internal("UMAMI_WEBSITE_ID 未配置".into()))?
            .to_string();

        Ok(Self {
            base_url,
            username,
            password,
            website_id,
        })
    }

    /// 获取有效 token（缓存中有则直接返回，否则登录）
    async fn ensure_token(&self) -> AppResult<String> {
        {
            let cache = get_or_init_cache().lock().unwrap();
            if !cache.token.is_empty() {
                return Ok(cache.token.clone());
            }
        }
        self.login_and_cache().await
    }

    async fn login_and_cache(&self) -> AppResult<String> {
        let url = format!("{}/api/auth/login", self.base_url);
        let payload = LoginPayload {
            username: &self.username,
            password: &self.password,
        };

        let resp = reqwest::Client::new()
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiFailure(format!("Umami 登录失败: {}", e)))?;

        let body: LoginResponse = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalApiFailure(format!("Umami 登录响应解析失败: {}", e)))?;

        let mut cache = get_or_init_cache().lock().unwrap();
        cache.token = body.token.clone();
        Ok(body.token)
    }

    /// token 失效时清除缓存并重试一次
    async fn authed_get(&self, path: &str) -> AppResult<reqwest::Response> {
        let token = self.ensure_token().await?;
        let url = format!("{}{}", self.base_url, path);

        let resp = reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;

        if resp.status().as_u16() == 401 {
            // token 过期，清除缓存重试一次
            get_or_init_cache().lock().unwrap().token.clear();
            let token = self.login_and_cache().await?;
            let url = format!("{}{}", self.base_url, path);
            return reqwest::Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("User-Agent", USER_AGENT)
                .send()
                .await
                .map_err(|e| AppError::ExternalApiFailure(e.to_string()));
        }

        Ok(resp)
    }

    /// 当前在线访客数
    pub async fn get_active_visitors(&self) -> AppResult<i64> {
        let resp = self
            .authed_get(&format!("/api/websites/{}/active", self.website_id))
            .await?;
        let data: UmamiActiveVisitors = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
        Ok(data.x)
    }

    /// 时间段汇总统计
    pub async fn get_stats(&self, start_at: u64, end_at: u64) -> AppResult<UmamiStatsResponse> {
        let resp = self
            .authed_get(&format!(
                "/api/websites/{}/stats?startAt={}&endAt={}",
                self.website_id, start_at, end_at
            ))
            .await?;
        let data: UmamiStatsResponse = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
        Ok(data)
    }

    /// 时序数据（pageviews / sessions）
    pub async fn get_pageviews(
        &self,
        start_at: u64,
        end_at: u64,
        unit: &str,
    ) -> AppResult<UmamiPageviewsResponse> {
        let resp = self
            .authed_get(&format!(
                "/api/websites/{}/pageviews?startAt={}&endAt={}&unit={}",
                self.website_id, start_at, end_at, unit
            ))
            .await?;
        let data: UmamiPageviewsResponse = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
        Ok(data)
    }

    /// 指标分布
    pub async fn get_metrics(
        &self,
        metric_type: &str,
        start_at: u64,
        end_at: u64,
        limit: u32,
    ) -> AppResult<Vec<UmamiMetricItem>> {
        let resp = self
            .authed_get(&format!(
                "/api/websites/{}/metrics?type={}&startAt={}&endAt={}&limit={}",
                self.website_id, metric_type, start_at, end_at, limit
            ))
            .await?;
        let data: Vec<UmamiMetricItem> = resp
            .json()
            .await
            .map_err(|e| AppError::ExternalApiFailure(e.to_string()))?;
        Ok(data)
    }
}
