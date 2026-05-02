use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::Claims;
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::legal::{LicenseVersion, PrivacyVersion};
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateLicenseInput {
    version: String,
    content: String,
}

#[derive(Deserialize)]
pub struct CreatePrivacyInput {
    version: String,
    date: String,
    content: String,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

// ═══════════════════════════════════════════════════════════════
//  许可证
// ═══════════════════════════════════════════════════════════════

//  GET /api/license — 获取最新版本

#[worker::send]
pub async fn get_license(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<LicenseVersion>> {
    let db = get_db(&env, Db::Legal)?;
    let result = db
        .prepare("SELECT * FROM license_versions ORDER BY created_at DESC LIMIT 1")
        .bind(&[])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<LicenseVersion> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let license = rows
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound("暂无可用的许可证".into()))?;
    Ok(Json(license))
}

//  GET /api/license/versions — 版本历史

#[worker::send]
pub async fn list_license_versions(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<Vec<LicenseVersion>>> {
    let db = get_db(&env, Db::Legal)?;
    let result = db
        .prepare("SELECT * FROM license_versions ORDER BY created_at DESC")
        .bind(&[])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let versions: Vec<LicenseVersion> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(Json(versions))
}

//  POST /api/license — 创建新版本

#[worker::send]
pub async fn create_license(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreateLicenseInput>,
) -> AppResult<Json<LicenseVersion>> {
    let db = get_db(&env, Db::Legal)?;
    let id = new_id();
    let now = time::now_str();

    db.prepare(
        "INSERT INTO license_versions (id, version, content, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.version.into(),
        body.content.into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM license_versions WHERE id = ?")
        .bind(&[id.as_str().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<LicenseVersion> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let license = rows
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal("创建许可证版本失败".into()))?;
    Ok(Json(license))
}

// ═══════════════════════════════════════════════════════════════
//  隐私政策
// ═══════════════════════════════════════════════════════════════

//  GET /api/privacy — 获取最新版本

#[worker::send]
pub async fn get_privacy(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<PrivacyVersion>> {
    let db = get_db(&env, Db::Legal)?;
    let result = db
        .prepare("SELECT * FROM privacy_versions ORDER BY created_at DESC LIMIT 1")
        .bind(&[])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<PrivacyVersion> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let privacy = rows
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound("暂无可用的隐私政策".into()))?;
    Ok(Json(privacy))
}

//  GET /api/privacy/versions — 版本历史

#[worker::send]
pub async fn list_privacy_versions(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<Vec<PrivacyVersion>>> {
    let db = get_db(&env, Db::Legal)?;
    let result = db
        .prepare("SELECT * FROM privacy_versions ORDER BY created_at DESC")
        .bind(&[])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let versions: Vec<PrivacyVersion> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(Json(versions))
}

//  POST /api/privacy — 创建新版本

#[worker::send]
pub async fn create_privacy(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreatePrivacyInput>,
) -> AppResult<Json<PrivacyVersion>> {
    let db = get_db(&env, Db::Legal)?;
    let id = new_id();
    let now = time::now_str();

    db.prepare(
        "INSERT INTO privacy_versions (id, version, date, content, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.version.into(),
        body.date.into(),
        body.content.into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM privacy_versions WHERE id = ?")
        .bind(&[id.as_str().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<PrivacyVersion> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let privacy = rows
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal("创建隐私政策版本失败".into()))?;
    Ok(Json(privacy))
}
