use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::Claims;
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::bangumi::BangumiRecord;
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct IdParam {
    pub id: String,
}

#[derive(Deserialize)]
pub struct RecordListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
    subject_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreateRecordInput {
    subject_id: i64,
    title: String,
    status: Option<String>,
    progress: Option<String>,
    cover_url: Option<String>,
    fansub: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateRecordInput {
    title: Option<String>,
    status: Option<String>,
    progress: Option<String>,
    cover_url: Option<String>,
    fansub: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedRecords {
    pub data: Vec<BangumiRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

async fn find_by_id(env: &Arc<Env>, id: &str) -> AppResult<Option<BangumiRecord>> {
    let db = get_db(env, Db::Bangumi)?;
    let result = db
        .prepare("SELECT * FROM bangumi_records WHERE id = ? AND deleted_at IS NULL")
        .bind(&[id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<BangumiRecord> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next())
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/bangumi/records — 列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn list_records(
    State(env): State<Arc<Env>>,
    Query(query): Query<RecordListQuery>,
) -> AppResult<Json<PaginatedRecords>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let db = get_db(&env, Db::Bangumi)?;

    let mut conditions = vec!["deleted_at IS NULL".to_string()];
    let mut params: Vec<wasm_bindgen::JsValue> = Vec::new();

    if let Some(ref status) = query.status {
        conditions.push("status = ?".into());
        params.push(status.into());
    }

    if let Some(sid) = query.subject_id {
        conditions.push("subject_id = ?".into());
        params.push(wasm_bindgen::JsValue::from(sid as f64));
    }

    let where_clause = conditions.join(" AND ");

    // 总数
    let total: i64 = {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM bangumi_records WHERE {}",
            where_clause
        );
        let result = db
            .prepare(&sql)
            .bind(&params)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .all()
            .await?;
        let rows: Vec<serde_json::Value> = result.results()?;
        rows.first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };

    // 数据
    let mut data_params = params.clone();
    data_params.push(wasm_bindgen::JsValue::from(page_size as f64));
    data_params.push(wasm_bindgen::JsValue::from(offset as f64));

    let sql = format!(
        "SELECT * FROM bangumi_records WHERE {} ORDER BY updated_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let result = db
        .prepare(&sql)
        .bind(&data_params)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let data: Vec<BangumiRecord> =
        serde_json::from_value(serde_json::Value::Array(result.results()?))?;

    let total_pages = if total == 0 {
        0
    } else {
        (total - 1) / page_size + 1
    };

    Ok(Json(PaginatedRecords {
        data,
        total,
        page,
        page_size,
        total_pages,
    }))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/bangumi/records — 添加
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn create_record(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreateRecordInput>,
) -> AppResult<Json<BangumiRecord>> {
    let db = get_db(&env, Db::Bangumi)?;
    let id = new_id();
    let now = time::now_str();
    let status = body.status.unwrap_or_else(|| "want_to_watch".to_string());

    db.prepare(
        "INSERT INTO bangumi_records (id, subject_id, title, status, progress, cover_url, fansub, added_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        wasm_bindgen::JsValue::from(body.subject_id as f64),
        body.title.into(),
        status.into(),
        body.progress.unwrap_or_default().into(),
        body.cover_url.unwrap_or_default().into(),
        body.fansub.unwrap_or_default().into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let record = find_by_id(&env, &id)
        .await?
        .ok_or_else(|| AppError::Internal("添加记录失败".into()))?;
    Ok(Json(record))
}

// ═══════════════════════════════════════════════════════════════
//  PUT /api/bangumi/records/:id — 更新
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn update_record(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<UpdateRecordInput>,
) -> AppResult<Json<BangumiRecord>> {
    let existing = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("记录不存在".into()))?;

    let db = get_db(&env, Db::Bangumi)?;
    let now = time::now_str();

    db.prepare(
        "UPDATE bangumi_records SET title = ?, status = ?, progress = ?, cover_url = ?, fansub = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&[
        body.title.unwrap_or(existing.title).into(),
        body.status.unwrap_or(existing.status).into(),
        body.progress.unwrap_or(existing.progress).into(),
        body.cover_url.unwrap_or(existing.cover_url).into(),
        body.fansub.unwrap_or(existing.fansub).into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let record = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::Internal("更新记录失败".into()))?;
    Ok(Json(record))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/bangumi/records/:id — 逻辑删除
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn delete_record(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Bangumi)?;
    let now = time::now_str();
    db.prepare("UPDATE bangumi_records SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}
