// src/handlers/resources.rs - CRUD Handlers Example

use axum::{extract::{Path, State}, Json};
use serde_json::json;
use std::sync::Arc;
use worker::Env;

use crate::auth::Claims;
use crate::db::get_db;
use crate::error::AppError;
use crate::models::{CreateResourceRequest, Resource, ResourceResponse, UpdateResourceRequest};

/// List all resources (public or protected)
/// GET /api/resources
#[worker::send]
pub async fn list(
    State(env): State<Arc<Env>>,
) -> Result<Json<Vec<ResourceResponse>>, AppError> {
    let db = get_db(&env)?;

    let results = db
        .prepare(
            "SELECT r.id, r.user_id, r.title, r.content, r.created_at, u.username
             FROM resources r
             LEFT JOIN users u ON r.user_id = u.id
             ORDER BY r.created_at DESC"
        )
        .all()
        .await?;

    let resources: Vec<ResourceResponse> = results
        .results::<serde_json::Value>()?
        .into_iter()
        .map(|v| ResourceResponse {
            id: v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            user_id: v.get("user_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            title: v.get("title").and_then(|x| x.as_str()).map(|s| s.to_string()),
            content: v.get("content").and_then(|x| x.as_str()).map(|s| s.to_string()),
            created_at: v.get("created_at").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            username: v.get("username").and_then(|x| x.as_str()).map(|s| s.to_string()),
        })
        .collect();

    Ok(Json(resources))
}

/// Get single resource by ID
/// GET /api/resources/{id}
#[worker::send]
pub async fn get(
    Path(id): Path<String>,
    State(env): State<Arc<Env>>,
) -> Result<Json<ResourceResponse>, AppError> {
    let db = get_db(&env)?;

    let result = db
        .prepare(
            "SELECT r.id, r.user_id, r.title, r.content, r.created_at, u.username
             FROM resources r
             LEFT JOIN users u ON r.user_id = u.id
             WHERE r.id = ?1"
        )
        .bind(&[id.into()])?
        .first::<serde_json::Value>(None)
        .await?
        .ok_or_else(|| AppError::NotFound("Resource not found".into()))?;

    let resource = ResourceResponse {
        id: result.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        user_id: result.get("user_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        title: result.get("title").and_then(|x| x.as_str()).map(|s| s.to_string()),
        content: result.get("content").and_then(|x| x.as_str()).map(|s| s.to_string()),
        created_at: result.get("created_at").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        username: result.get("username").and_then(|x| x.as_str()).map(|s| s.to_string()),
    };

    Ok(Json(resource))
}

/// Create new resource (protected)
/// POST /api/resources
#[worker::send]
pub async fn create(
    claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreateResourceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if body.title.is_none() && body.content.is_none() {
        return Err(AppError::BadRequest("Title or content is required".into()));
    }

    let db = get_db(&env)?;
    let resource_id = uuid::Uuid::now_v7().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "INSERT INTO resources (id, user_id, title, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
    )
    .bind(&[
        resource_id.clone().into(),
        claims.sub.into(),
        body.title.clone().into(),
        body.content.clone().into(),
        now.clone().into(),
    ])?
    .run()
    .await?;

    Ok(Json(json!({
        "id": resource_id,
        "title": body.title,
        "content": body.content,
        "created_at": now
    })))
}

/// Update resource (protected, owner only)
/// PUT /api/resources/{id}
#[worker::send]
pub async fn update(
    claims: Claims,
    Path(id): Path<String>,
    State(env): State<Arc<Env>>,
    Json(body): Json<UpdateResourceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = get_db(&env)?;

    // Verify ownership
    let resource = db
        .prepare("SELECT user_id FROM resources WHERE id = ?1")
        .bind(&[id.clone().into()])?
        .first::<serde_json::Value>(None)
        .await?;

    match resource {
        Some(r) => {
            let owner_id = r.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
            if owner_id != claims.sub {
                return Err(AppError::Forbidden("Not authorized to update this resource".into()));
            }

            db.prepare(
                "UPDATE resources SET title = ?1, content = ?2 WHERE id = ?3"
            )
            .bind(&[
                body.title.clone().into(),
                body.content.clone().into(),
                id.into(),
            ])?
            .run()
            .await?;

            Ok(Json(json!({"message": "Updated successfully"})))
        }
        None => Err(AppError::NotFound("Resource not found".into())),
    }
}

/// Delete resource (protected, owner only)
/// DELETE /api/resources/{id}
#[worker::send]
pub async fn delete(
    claims: Claims,
    Path(id): Path<String>,
    State(env): State<Arc<Env>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = get_db(&env)?;

    // Verify ownership
    let resource = db
        .prepare("SELECT user_id FROM resources WHERE id = ?1")
        .bind(&[id.clone().into()])?
        .first::<serde_json::Value>(None)
        .await?;

    match resource {
        Some(r) => {
            let owner_id = r.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
            if owner_id != claims.sub {
                return Err(AppError::Forbidden("Not authorized to delete this resource".into()));
            }

            db.prepare("DELETE FROM resources WHERE id = ?1")
                .bind(&[id.into()])?
                .run()
                .await?;

            Ok(Json(json!({"message": "Deleted successfully"})))
        }
        None => Err(AppError::NotFound("Resource not found".into())),
    }
}