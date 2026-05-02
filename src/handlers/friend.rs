use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::{Claims, OptionalClaims};
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::friend::Friend;
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct IdParam {
    pub id: String,
}

#[derive(Deserialize)]
pub struct FriendListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateFriendInput {
    name: String,
    url: String,
    avatar_url: Option<String>,
    description: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateFriendInput {
    name: Option<String>,
    url: Option<String>,
    avatar_url: Option<String>,
    description: Option<String>,
    email: Option<String>,
    sort_order: Option<i32>,
}

#[derive(Deserialize)]
pub struct StatusInput {
    status: String, // approved / rejected
}

#[derive(Serialize)]
pub struct PaginatedFriends {
    pub data: Vec<Friend>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

async fn find_by_id(env: &Arc<Env>, id: &str) -> AppResult<Option<Friend>> {
    let db = get_db(env, Db::Social)?;
    let result = db
        .prepare("SELECT * FROM friends WHERE id = ? AND deleted_at IS NULL")
        .bind(&[id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<Friend> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next())
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/friends — 列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn list_friends(
    auth: OptionalClaims,
    State(env): State<Arc<Env>>,
    Query(query): Query<FriendListQuery>,
) -> AppResult<Json<PaginatedFriends>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let db = get_db(&env, Db::Social)?;

    let mut conditions = vec!["deleted_at IS NULL".to_string()];
    let mut params: Vec<wasm_bindgen::JsValue> = Vec::new();

    // 未登录只能看 approved
    let status = if auth.0.is_some() {
        query.status.as_deref().unwrap_or("approved")
    } else {
        "approved"
    };
    conditions.push("status = ?".into());
    params.push(status.into());

    let where_clause = conditions.join(" AND ");

    // 总数
    let total: i64 = {
        let sql = format!("SELECT COUNT(*) as cnt FROM friends WHERE {}", where_clause);
        let result = db.prepare(&sql)
            .bind(&params)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .all().await?;
        let rows: Vec<serde_json::Value> = result.results()?;
        rows.first().and_then(|r| r.get("cnt")).and_then(|v| v.as_i64()).unwrap_or(0)
    };

    // 数据
    let mut data_params = params.clone();
    data_params.push(wasm_bindgen::JsValue::from(page_size as f64));
    data_params.push(wasm_bindgen::JsValue::from(offset as f64));

    let sql = format!(
        "SELECT * FROM friends WHERE {} ORDER BY sort_order, created_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let result = db.prepare(&sql)
        .bind(&data_params)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all().await?;
    let data: Vec<Friend> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;

    let total_pages = if total == 0 { 0 } else { (total - 1) / page_size + 1 };

    Ok(Json(PaginatedFriends { data, total, page, page_size, total_pages }))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/friends/:id — 详情
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn get_friend(
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<Friend>> {
    let friend = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("友链不存在".into()))?;
    Ok(Json(friend))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/friends — 添加（公开）
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn create_friend(
    _claims: Claims,  // 改为可公开提交？当前先要求认证
    State(env): State<Arc<Env>>,
    Json(body): Json<CreateFriendInput>,
) -> AppResult<Json<Friend>> {
    let db = get_db(&env, Db::Social)?;
    let id = new_id();
    let now = time::now_str();
    db.prepare(
        "INSERT INTO friends (id, name, url, avatar_url, description, email, status, sort_order, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, 'pending', 0, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.name.into(),
        body.url.into(),
        body.avatar_url.unwrap_or_default().into(),
        body.description.unwrap_or_default().into(),
        body.email.unwrap_or_default().into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let friend = find_by_id(&env, &id).await?
        .ok_or_else(|| AppError::Internal("创建友链失败".into()))?;
    Ok(Json(friend))
}

// ═══════════════════════════════════════════════════════════════
//  PUT /api/friends/:id — 更新
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn update_friend(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<UpdateFriendInput>,
) -> AppResult<Json<Friend>> {
    let existing = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("友链不存在".into()))?;

    let db = get_db(&env, Db::Social)?;
    let now = time::now_str();
    db.prepare(
        "UPDATE friends SET name = ?, url = ?, avatar_url = ?, description = ?, email = ?, \
         sort_order = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&[
        body.name.unwrap_or(existing.name).into(),
        body.url.unwrap_or(existing.url).into(),
        body.avatar_url.unwrap_or(existing.avatar_url).into(),
        body.description.unwrap_or(existing.description).into(),
        body.email.unwrap_or(existing.email).into(),
        body.sort_order.unwrap_or(existing.sort_order).into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let friend = find_by_id(&env, &param.id).await?
        .ok_or_else(|| AppError::Internal("更新友链失败".into()))?;
    Ok(Json(friend))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/friends/:id — 逻辑删除
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn delete_friend(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Social)?;
    let now = time::now_str();
    db.prepare("UPDATE friends SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}

// ═══════════════════════════════════════════════════════════════
//  PATCH /api/friends/:id/status — 审核
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn review_friend(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<StatusInput>,
) -> AppResult<Json<Friend>> {
    if body.status != "approved" && body.status != "rejected" {
        return Err(AppError::BadRequest("状态值必须是 approved 或 rejected".into()));
    }

    let db = get_db(&env, Db::Social)?;
    let now = time::now_str();
    db.prepare("UPDATE friends SET status = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
        .bind(&[body.status.into(), now.into(), param.id.as_str().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;

    let friend = find_by_id(&env, &param.id).await?
        .ok_or_else(|| AppError::NotFound("友链不存在".into()))?;
    Ok(Json(friend))
}
