use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use worker::Env;

use crate::error::AppError;
use crate::kv::KvCache;

// ─── Token 类型 ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Access,
    Refresh,
    Temp,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,       // admin_id
    pub username: String,
    pub exp: usize,
    pub iat: usize,
    pub kind: TokenKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,       // token ID，用于 refresh token 黑名单
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,  // temp token 用途: "2fa" | "verify_email"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

// ─── Token 生成 ───────────────────────────────────────────────

const ACCESS_TTL: usize = 15 * 60;           // 15 分钟
const REFRESH_TTL: usize = 7 * 24 * 60 * 60; // 7 天
const TEMP_TTL: usize = 5 * 60;              // 5 分钟

fn make_token(
    claims: &Claims,
    secret: &str,
    ttl: usize,
) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let mut c = claims.clone();
    c.iat = now;
    c.exp = now + ttl;
    encode(
        &Header::default(),
        &c,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token 生成失败: {}", e)))
}

pub fn generate_access_token(admin_id: &str, username: &str, secret: &str) -> Result<String, AppError> {
    make_token(
        &Claims {
            sub: admin_id.into(),
            username: username.into(),
            exp: 0,
            iat: 0,
            kind: TokenKind::Access,
            jti: None,
            purpose: None,
        },
        secret,
        ACCESS_TTL,
    )
}

pub fn generate_refresh_token(admin_id: &str, username: &str, secret: &str) -> Result<(String, String), AppError> {
    let jti = Uuid::now_v7().to_string();
    let token = make_token(
        &Claims {
            sub: admin_id.into(),
            username: username.into(),
            exp: 0,
            iat: 0,
            kind: TokenKind::Refresh,
            jti: Some(jti.clone()),
            purpose: None,
        },
        secret,
        REFRESH_TTL,
    )?;
    Ok((token, jti))
}

pub fn generate_temp_token(admin_id: &str, purpose: &str, secret: &str) -> Result<String, AppError> {
    make_token(
        &Claims {
            sub: admin_id.into(),
            username: String::new(),
            exp: 0,
            iat: 0,
            kind: TokenKind::Temp,
            jti: None,
            purpose: Some(purpose.into()),
        },
        secret,
        TEMP_TTL,
    )
}

pub fn generate_token_pair(admin_id: &str, username: &str, secret: &str) -> Result<TokenPair, AppError> {
    let access_token = generate_access_token(admin_id, username, secret)?;
    let (refresh_token, _jti) = generate_refresh_token(admin_id, username, secret)?;
    Ok(TokenPair {
        access_token,
        refresh_token,
    })
}

// ─── Token 验证 ───────────────────────────────────────────────

fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

pub fn verify_access_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let claims = verify_token(token, secret)?;
    if claims.kind != TokenKind::Access {
        return Err(AppError::WrongCredentials);
    }
    Ok(claims)
}

pub fn verify_refresh_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let claims = verify_token(token, secret)?;
    if claims.kind != TokenKind::Refresh {
        return Err(AppError::WrongCredentials);
    }
    Ok(claims)
}

pub fn verify_temp_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let claims = verify_token(token, secret)?;
    if claims.kind != TokenKind::Temp {
        return Err(AppError::WrongCredentials);
    }
    Ok(claims)
}

// ─── Refresh Token 黑名单（KV） ───────────────────────────────

const BLACKLIST_PREFIX: &str = "rtk_blacklist:";

pub async fn blacklist_refresh_token(kv: &KvCache, jti: &str) -> Result<(), AppError> {
    kv.put_str(&format!("{}{}", BLACKLIST_PREFIX, jti), "1", REFRESH_TTL as u64)
        .await
}

pub async fn is_refresh_token_blacklisted(kv: &KvCache, jti: &str) -> Result<bool, AppError> {
    Ok(kv
        .get_str(&format!("{}{}", BLACKLIST_PREFIX, jti))
        .await?
        .is_some())
}

// ─── Claims 提取器（axum 中间件/守卫） ───────────────────────

impl FromRequestParts<Arc<Env>> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Env>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_bearer_token(parts)?;

        let secret = get_jwt_secret(state)?;
        verify_access_token(&token, &secret)
    }
}

/// 可选认证提取器 — 不强制要求 token，有则解析
pub struct OptionalClaims(pub Option<Claims>);

impl FromRequestParts<Arc<Env>> for OptionalClaims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Env>,
    ) -> Result<Self, Self::Rejection> {
        let token = match extract_bearer_token(parts) {
            Ok(t) => t,
            Err(_) => return Ok(Self(None)),
        };
        let secret = get_jwt_secret(state)?;
        match verify_access_token(&token, &secret) {
            Ok(c) => Ok(Self(Some(c))),
            Err(_) => Ok(Self(None)),
        }
    }
}

// ─── 工具函数 ─────────────────────────────────────────────────

fn extract_bearer_token(parts: &Parts) -> Result<String, AppError> {
    parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_owned())
        .ok_or(AppError::WrongCredentials)
}

fn get_jwt_secret(state: &Arc<Env>) -> Result<String, AppError> {
    state
        .var("JWT_SECRET")
        .map(|v| v.to_string())
        .map_err(|_| AppError::Internal("JWT_SECRET 未配置".into()))
}
