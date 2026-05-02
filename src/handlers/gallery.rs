use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::{Claims, OptionalClaims};
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::gallery::{Gallery, GalleryRow, Photo};
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct IdParam {
    pub id: String,
}

#[derive(Deserialize)]
pub struct GalleryListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    tag: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateGalleryInput {
    title: String,
    cover_path: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateGalleryInput {
    title: Option<String>,
    cover_path: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct AddPhotosInput {
    paths: Vec<String>,
}

#[derive(Serialize)]
pub struct PaginatedGalleries {
    pub data: Vec<Gallery>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Serialize)]
pub struct GalleryDetail {
    pub id: String,
    pub title: String,
    pub cover_path: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub photos: Vec<Photo>,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

async fn find_gallery_by_id(env: &Arc<Env>, id: &str) -> AppResult<Option<Gallery>> {
    let db = get_db(env, Db::Media)?;
    let result = db
        .prepare("SELECT * FROM galleries WHERE id = ? AND deleted_at IS NULL")
        .bind(&[id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<GalleryRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next().map(Gallery::from))
}

async fn find_photos_by_gallery(env: &Arc<Env>, gallery_id: &str) -> AppResult<Vec<Photo>> {
    let db = get_db(env, Db::Media)?;
    let result = db
        .prepare("SELECT * FROM photos WHERE gallery_id = ? AND deleted_at IS NULL ORDER BY created_at ASC")
        .bind(&[gallery_id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let photos: Vec<Photo> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(photos)
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/galleries — 列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn list_galleries(
    _auth: OptionalClaims,
    State(env): State<Arc<Env>>,
    Query(query): Query<GalleryListQuery>,
) -> AppResult<Json<PaginatedGalleries>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let db = get_db(&env, Db::Media)?;

    let mut conditions = vec!["deleted_at IS NULL".to_string()];
    let mut params: Vec<wasm_bindgen::JsValue> = Vec::new();

    if let Some(ref tag) = query.tag {
        conditions.push("tags LIKE ?".into());
        params.push(format!("%\"{}\"%", tag).into());
    }

    // 未登录只看非收藏（无收藏字段，直接全部公开）
    // 相册默认全部可见，不需额外过滤

    let where_clause = conditions.join(" AND ");

    // 总数
    let total: i64 = {
        let sql = format!("SELECT COUNT(*) as cnt FROM galleries WHERE {}", where_clause);
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
        "SELECT * FROM galleries WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let result = db
        .prepare(&sql)
        .bind(&data_params)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<GalleryRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let data: Vec<Gallery> = rows.into_iter().map(Gallery::from).collect();

    let total_pages = if total == 0 {
        0
    } else {
        (total - 1) / page_size + 1
    };

    Ok(Json(PaginatedGalleries {
        data,
        total,
        page,
        page_size,
        total_pages,
    }))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/galleries/:id — 详情（含照片列表）
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn get_gallery(
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<GalleryDetail>> {
    let gallery = find_gallery_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("相册不存在".into()))?;

    let photos = find_photos_by_gallery(&env, &param.id).await?;

    Ok(Json(GalleryDetail {
        id: gallery.id,
        title: gallery.title,
        cover_path: gallery.cover_path,
        tags: gallery.tags,
        created_at: gallery.created_at,
        updated_at: gallery.updated_at,
        photos,
    }))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/galleries — 创建
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn create_gallery(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreateGalleryInput>,
) -> AppResult<Json<Gallery>> {
    let db = get_db(&env, Db::Media)?;
    let id = new_id();
    let now = time::now_str();
    let tags = body
        .tags
        .map(|t| serde_json::to_string(&t).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string());

    db.prepare(
        "INSERT INTO galleries (id, title, cover_path, tags, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.title.into(),
        body.cover_path.unwrap_or_default().into(),
        tags.into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let gallery = find_gallery_by_id(&env, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建相册失败".into()))?;
    Ok(Json(gallery))
}

// ═══════════════════════════════════════════════════════════════
//  PUT /api/galleries/:id — 更新
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn update_gallery(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<UpdateGalleryInput>,
) -> AppResult<Json<Gallery>> {
    let existing = find_gallery_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("相册不存在".into()))?;

    let db = get_db(&env, Db::Media)?;
    let now = time::now_str();
    let tags = match body.tags {
        Some(t) => serde_json::to_string(&t).unwrap_or_else(|_| "[]".to_string()),
        None => serde_json::to_string(&existing.tags).unwrap_or_else(|_| "[]".to_string()),
    };

    db.prepare(
        "UPDATE galleries SET title = ?, cover_path = ?, tags = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&[
        body.title.unwrap_or(existing.title).into(),
        body.cover_path.unwrap_or(existing.cover_path).into(),
        tags.into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let gallery = find_gallery_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::Internal("更新相册失败".into()))?;
    Ok(Json(gallery))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/galleries/:id — 逻辑删除
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn delete_gallery(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Media)?;
    let now = time::now_str();
    db.prepare("UPDATE galleries SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/galleries/:id/photos — 添加照片
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn add_photos(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<AddPhotosInput>,
) -> AppResult<Json<Vec<Photo>>> {
    // 先确认相册存在
    find_gallery_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("相册不存在".into()))?;

    let db = get_db(&env, Db::Media)?;
    let now = time::now_str();

    let mut photos = Vec::new();
    for path in &body.paths {
        let id = new_id();
        db.prepare(
            "INSERT INTO photos (id, gallery_id, path, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&[
            id.as_str().into(),
            param.id.as_str().into(),
            path.into(),
            now.clone().into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
        photos.push(Photo {
            id,
            gallery_id: param.id.clone(),
            path: path.clone(),
            created_at: now.clone(),
            deleted_at: None,
        });
    }

    Ok(Json(photos))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/photos/:id — 删除照片
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn delete_photo(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Media)?;
    let now = time::now_str();
    db.prepare("UPDATE photos SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}
