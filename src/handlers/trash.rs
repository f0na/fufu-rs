use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::time;

// ─── 资源配置 ─────────────────────────────────────────────────

struct ResourceInfo {
    db: Db,
    table: &'static str,
    fields: &'static [&'static str],
}

fn get_resource_info(resource: &str) -> AppResult<ResourceInfo> {
    match resource {
        "posts" => Ok(ResourceInfo {
            db: Db::Posts,
            table: "posts",
            fields: &[
                "id", "title", "slug", "excerpt", "tags", "status",
                "view_count", "created_at", "updated_at", "published_at", "deleted_at",
            ],
        }),
        "friends" => Ok(ResourceInfo {
            db: Db::Social,
            table: "friends",
            fields: &[
                "id", "name", "url", "avatar_url", "description",
                "email", "status", "sort_order", "created_at", "updated_at", "deleted_at",
            ],
        }),
        "links" => Ok(ResourceInfo {
            db: Db::Social,
            table: "links",
            fields: &[
                "id", "title", "url", "description", "favicon_url",
                "tags", "favorite", "sort_order", "created_at", "updated_at", "deleted_at",
            ],
        }),
        "galleries" => Ok(ResourceInfo {
            db: Db::Media,
            table: "galleries",
            fields: &[
                "id", "title", "cover_path", "tags",
                "created_at", "updated_at", "deleted_at",
            ],
        }),
        "bangumi" | "bangumi_records" | "bangumi-records" => Ok(ResourceInfo {
            db: Db::Bangumi,
            table: "bangumi_records",
            fields: &[
                "id", "subject_id", "title", "status", "progress",
                "cover_url", "fansub", "added_at", "updated_at", "deleted_at",
            ],
        }),
        _ => Err(AppError::BadRequest(format!(
            "不支持的资源类型: {}",
            resource
        ))),
    }
}

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct ResourceParam {
    pub resource: String,
}

#[derive(Deserialize)]
pub struct ResourceIdParam {
    pub resource: String,
    pub id: String,
}

#[derive(Deserialize)]
pub struct TrashListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Serialize)]
pub struct PaginatedTrash {
    pub data: Vec<serde_json::Value>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/trash/:resource — 垃圾桶列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn list_trash(
    State(env): State<Arc<Env>>,
    Path(param): Path<ResourceParam>,
    Query(query): Query<TrashListQuery>,
) -> AppResult<Json<PaginatedTrash>> {
    let info = get_resource_info(&param.resource)?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let db = get_db(&env, info.db)?;

    // 总数
    let total: i64 = {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE deleted_at IS NOT NULL",
            info.table
        );
        let result = db
            .prepare(&sql)
            .bind(&[])
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
    let fields = info.fields.join(", ");
    let sql = format!(
        "SELECT {} FROM {} WHERE deleted_at IS NOT NULL \
         ORDER BY deleted_at DESC LIMIT ? OFFSET ?",
        fields, info.table
    );
    let result = db
        .prepare(&sql)
        .bind(&[
            wasm_bindgen::JsValue::from(page_size as f64),
            wasm_bindgen::JsValue::from(offset as f64),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;

    let data: Vec<serde_json::Value> = result.results()?;

    let total_pages = if total == 0 {
        0
    } else {
        (total - 1) / page_size + 1
    };

    Ok(Json(PaginatedTrash {
        data,
        total,
        page,
        page_size,
        total_pages,
    }))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/trash/:resource/:id — 真删除
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn hard_delete(
    _claims: crate::auth::jwt::Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<ResourceIdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let info = get_resource_info(&param.resource)?;
    let db = get_db(&env, info.db)?;

    let sql = format!("DELETE FROM {} WHERE id = ? AND deleted_at IS NOT NULL", info.table);
    db.prepare(&sql)
        .bind(&[param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;

    Ok(Json(serde_json::json!({ "message": "已永久删除" })))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/trash/:resource/:id/restore — 恢复
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn restore(
    _claims: crate::auth::jwt::Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<ResourceIdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let info = get_resource_info(&param.resource)?;
    let db = get_db(&env, info.db)?;

    let now = time::now_str();
    let sql = format!(
        "UPDATE {} SET deleted_at = NULL, updated_at = ? WHERE id = ? AND deleted_at IS NOT NULL",
        info.table
    );
    db.prepare(&sql)
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;

    Ok(Json(serde_json::json!({ "message": "已恢复" })))
}
