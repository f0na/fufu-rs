use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::kv::KvCache;
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct TargetPath {
    target_type: String,
    target_id: String,
}

#[derive(Serialize)]
pub struct LikeResponse {
    pub count: i64,
    pub liked: bool,
}

// ─── 工具 ─────────────────────────────────────────────────────

fn like_kv_key(target_type: &str, target_id: &str, visitor: &str) -> String {
    format!("like:{}:{}:{}", target_type, target_id, visitor)
}

const LIKE_TTL: u64 = 30 * 24 * 60 * 60; // 30 天

/// 当前访问者标识（简化版，实际可用 CF-Connecting-IP）
fn visitor_id() -> String {
    "default_visitor".into()
}

async fn query_count(db: &worker::D1Database, target_type: &str, target_id: &str) -> AppResult<i64> {
    let result = db
        .prepare("SELECT count FROM like_counts WHERE target_type = ? AND target_id = ?")
        .bind(&[target_type.into(), target_id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    Ok(rows
        .first()
        .and_then(|r| r.get("count"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/likes/:target_type/:target_id
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn toggle_like(
    State(env): State<Arc<Env>>,
    Path(path): Path<TargetPath>,
) -> AppResult<Json<LikeResponse>> {
    let db = get_db(&env, Db::Likes)?;
    let kv = KvCache::new(&env)?;
    let visitor = visitor_id();
    let kv_key = like_kv_key(&path.target_type, &path.target_id, &visitor);

    let already_liked = kv.get_str(&kv_key).await?.is_some();

    if already_liked {
        // 取消点赞
        kv.delete(&kv_key).await?;
        db.prepare(
            "UPDATE like_counts SET count = MAX(0, count - 1), updated_at = ? \
             WHERE target_type = ? AND target_id = ?",
        )
        .bind(&[
            time::now_str().into(),
            path.target_type.as_str().into(),
            path.target_id.as_str().into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;

        let count = query_count(&db, &path.target_type, &path.target_id).await?;
        Ok(Json(LikeResponse { count, liked: false }))
    } else {
        // 点赞 — UPSERT
        kv.put_str(&kv_key, "1", LIKE_TTL).await?;
        let now = time::now_str();
        let id = uuid::Uuid::now_v7().to_string();
        db.prepare(
            "INSERT INTO like_counts (id, target_type, target_id, count, created_at, updated_at) \
             VALUES (?, ?, ?, 1, ?, ?) \
             ON CONFLICT(target_type, target_id) DO UPDATE SET \
             count = count + 1, updated_at = excluded.updated_at",
        )
        .bind(&[
            id.into(),
            path.target_type.as_str().into(),
            path.target_id.as_str().into(),
            now.clone().into(),
            now.into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;

        let count = query_count(&db, &path.target_type, &path.target_id).await?;
        Ok(Json(LikeResponse { count, liked: true }))
    }
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/likes/:target_type/:target_id
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn get_likes(
    State(env): State<Arc<Env>>,
    Path(path): Path<TargetPath>,
) -> AppResult<Json<LikeResponse>> {
    let db = get_db(&env, Db::Likes)?;
    let count = query_count(&db, &path.target_type, &path.target_id).await?;

    let kv = KvCache::new(&env)?;
    let visitor = visitor_id();
    let kv_key = like_kv_key(&path.target_type, &path.target_id, &visitor);
    let liked = kv.get_str(&kv_key).await?.is_some();

    Ok(Json(LikeResponse { count, liked }))
}
