use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::{Claims, OptionalClaims};
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::link::{Link, LinkRow, TagCount};
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct IdParam {
    pub id: String,
}

#[derive(Deserialize)]
pub struct LinkListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    tag: Option<String>,
    favorite: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreateLinkInput {
    title: String,
    url: String,
    description: Option<String>,
    favicon_url: Option<String>,
    tags: Option<Vec<String>>,
    favorite: Option<i32>,
    sort_order: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateLinkInput {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
    favicon_url: Option<String>,
    tags: Option<Vec<String>>,
    favorite: Option<i32>,
    sort_order: Option<i32>,
}

#[derive(Serialize)]
pub struct PaginatedLinks {
    pub data: Vec<Link>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Serialize)]
pub struct TagMeta {
    pub tags: Vec<TagCount>,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

async fn find_by_id(env: &Arc<Env>, id: &str) -> AppResult<Option<Link>> {
    let db = get_db(env, Db::Social)?;
    let result = db
        .prepare("SELECT * FROM links WHERE id = ? AND deleted_at IS NULL")
        .bind(&[id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<LinkRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next().map(Link::from))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/links — 列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn list_links(
    auth: OptionalClaims,
    State(env): State<Arc<Env>>,
    Query(query): Query<LinkListQuery>,
) -> AppResult<Json<PaginatedLinks>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let db = get_db(&env, Db::Social)?;

    let mut conditions = vec!["deleted_at IS NULL".to_string()];
    let mut params: Vec<wasm_bindgen::JsValue> = Vec::new();

    if let Some(ref tag) = query.tag {
        conditions.push("tags LIKE ?".into());
        params.push(format!("%\"{}\"%", tag).into());
    }

    if let Some(fav) = query.favorite {
        conditions.push("favorite = ?".into());
        params.push(wasm_bindgen::JsValue::from(fav as f64));
    }

    // 未登录只看非收藏
    if auth.0.is_none() {
        conditions.push("favorite = 0".into());
    }

    let where_clause = conditions.join(" AND ");

    // 总数
    let total: i64 = {
        let sql = format!("SELECT COUNT(*) as cnt FROM links WHERE {}", where_clause);
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
        "SELECT * FROM links WHERE {} ORDER BY sort_order, created_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let result = db
        .prepare(&sql)
        .bind(&data_params)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<LinkRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let data: Vec<Link> = rows.into_iter().map(Link::from).collect();

    let total_pages = if total == 0 {
        0
    } else {
        (total - 1) / page_size + 1
    };

    Ok(Json(PaginatedLinks {
        data,
        total,
        page,
        page_size,
        total_pages,
    }))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/links/meta — 标签元数据
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn get_link_meta(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<TagMeta>> {
    let db = get_db(&env, Db::Social)?;
    let result = db
        .prepare("SELECT tags FROM links WHERE deleted_at IS NULL")
        .bind(&[])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;

    let mut tag_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in &rows {
        if let Some(tags_str) = row.get("tags").and_then(|v| v.as_str()) {
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(tags_str) {
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
        }
    }

    let mut tags: Vec<TagCount> = tag_counts
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));

    Ok(Json(TagMeta { tags }))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/links/:id — 详情
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn get_link(
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<Link>> {
    let link = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("链接不存在".into()))?;
    Ok(Json(link))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/links — 添加
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn create_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreateLinkInput>,
) -> AppResult<Json<Link>> {
    let db = get_db(&env, Db::Social)?;
    let id = new_id();
    let now = time::now_str();
    let tags = body
        .tags
        .map(|t| serde_json::to_string(&t).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string());

    db.prepare(
        "INSERT INTO links (id, title, url, description, favicon_url, tags, favorite, sort_order, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.title.into(),
        body.url.into(),
        body.description.unwrap_or_default().into(),
        body.favicon_url.unwrap_or_default().into(),
        tags.into(),
        wasm_bindgen::JsValue::from(body.favorite.unwrap_or(0) as f64),
        body.sort_order.unwrap_or(0).into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let link = find_by_id(&env, &id)
        .await?
        .ok_or_else(|| AppError::Internal("创建链接失败".into()))?;
    Ok(Json(link))
}

// ═══════════════════════════════════════════════════════════════
//  PUT /api/links/:id — 更新
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn update_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<UpdateLinkInput>,
) -> AppResult<Json<Link>> {
    let existing = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::NotFound("链接不存在".into()))?;

    let db = get_db(&env, Db::Social)?;
    let now = time::now_str();
    let tags = match body.tags {
        Some(t) => serde_json::to_string(&t).unwrap_or_else(|_| "[]".to_string()),
        None => serde_json::to_string(&existing.tags).unwrap_or_else(|_| "[]".to_string()),
    };

    db.prepare(
        "UPDATE links SET title = ?, url = ?, description = ?, favicon_url = ?, tags = ?, \
         favorite = ?, sort_order = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&[
        body.title.unwrap_or(existing.title).into(),
        body.url.unwrap_or(existing.url).into(),
        body.description.unwrap_or(existing.description).into(),
        body.favicon_url.unwrap_or(existing.favicon_url).into(),
        tags.into(),
        body.favorite.unwrap_or(existing.favorite).into(),
        body.sort_order.unwrap_or(existing.sort_order).into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let link = find_by_id(&env, &param.id)
        .await?
        .ok_or_else(|| AppError::Internal("更新链接失败".into()))?;
    Ok(Json(link))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/links/:id — 逻辑删除
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn delete_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Social)?;
    let now = time::now_str();
    db.prepare("UPDATE links SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}
