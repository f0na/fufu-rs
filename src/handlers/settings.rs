use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::Env;

use crate::auth::jwt::Claims;
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::settings::{
    Announcement, AnnouncementRow, FooterLink, SiteFooter, SiteProfile, SocialLink,
};
use crate::time;

// ─── 路径参数 ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct IdParam {
    pub id: String,
}

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Serialize)]
pub struct SettingResponse<T: Serialize> {
    pub data: T,
}

#[derive(Deserialize)]
pub struct ProfileInput {
    pub site_name: Option<String>,
    pub subtitle: Option<String>,
    pub logo_url: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub icp_beian: Option<String>,
}

#[derive(Deserialize)]
pub struct FooterInput {
    pub content: Option<String>,
    pub copyright_text: Option<String>,
}

#[derive(Deserialize)]
pub struct FooterLinkInput {
    pub name: String,
    pub url: String,
    pub sort_order: Option<i32>,
}

#[derive(Deserialize)]
pub struct SocialLinkInput {
    pub platform: String,
    pub label: Option<String>,
    pub url: String,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Deserialize)]
pub struct AnnouncementInput {
    pub content: String,
    pub active: Option<bool>,
    pub sort_order: Option<i32>,
}

// ─── 工具函数 ─────────────────────────────────────────────────

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

async fn get_singleton<T>(env: &Arc<Env>, table: &str) -> AppResult<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    let db = get_db(env, Db::Core)?;
    let result = db
        .prepare(&format!("SELECT * FROM {} LIMIT 1", table))
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    if let Some(val) = rows.into_iter().next() {
        Ok(Some(serde_json::from_value(val)?))
    } else {
        Ok(None)
    }
}

// ═══════════════════════════════════════════════════════════════
//  站点信息 Profile
// ═══════════════════════════════════════════════════════════════

/// GET /api/settings/profile（公开）
#[worker::send]
pub async fn get_profile(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<SettingResponse<SiteProfile>>> {
    let profile = get_singleton::<SiteProfile>(&env, "site_profile")
        .await?
        .unwrap_or(SiteProfile {
            id: String::new(),
            site_name: String::new(),
            subtitle: String::new(),
            logo_url: String::new(),
            description: String::new(),
            keywords: String::new(),
            icp_beian: String::new(),
            created_at: time::now_str(),
            updated_at: time::now_str(),
        });
    Ok(Json(SettingResponse { data: profile }))
}

/// PUT /api/settings/profile
#[worker::send]
#[worker::send]
pub async fn update_profile(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<ProfileInput>,
) -> AppResult<Json<SettingResponse<SiteProfile>>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    let existing = db
        .prepare("SELECT id FROM site_profile LIMIT 1")
        .all()
        .await?;
    let existing_rows: Vec<serde_json::Value> = existing.results()?;

    if let Some(row) = existing_rows.first() {
        let id = row["id"].as_str().unwrap_or("");
        db.prepare(
            "UPDATE site_profile SET site_name = ?, subtitle = ?, logo_url = ?, \
             description = ?, keywords = ?, icp_beian = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&[
            body.site_name.unwrap_or_default().into(),
            body.subtitle.unwrap_or_default().into(),
            body.logo_url.unwrap_or_default().into(),
            body.description.unwrap_or_default().into(),
            body.keywords.unwrap_or_default().into(),
            body.icp_beian.unwrap_or_default().into(),
            now.into(),
            id.into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    } else {
        let id = new_id();
        db.prepare(
            "INSERT INTO site_profile (id, site_name, subtitle, logo_url, description, keywords, icp_beian, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&[
            id.as_str().into(),
            body.site_name.unwrap_or_default().into(),
            body.subtitle.unwrap_or_default().into(),
            body.logo_url.unwrap_or_default().into(),
            body.description.unwrap_or_default().into(),
            body.keywords.unwrap_or_default().into(),
            body.icp_beian.unwrap_or_default().into(),
            now.clone().into(),
            now.into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    }

    let profile = get_singleton::<SiteProfile>(&env, "site_profile")
        .await?
        .ok_or_else(|| AppError::Internal("读取站点信息失败".into()))?;
    Ok(Json(SettingResponse { data: profile }))
}

// ═══════════════════════════════════════════════════════════════
//  页脚 Footer
// ═══════════════════════════════════════════════════════════════

/// GET /api/settings/footer（公开）
#[worker::send]
pub async fn get_footer(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<SettingResponse<SiteFooter>>> {
    let footer = get_singleton::<SiteFooter>(&env, "site_footer")
        .await?
        .unwrap_or(SiteFooter {
            id: String::new(),
            content: String::new(),
            copyright_text: String::new(),
            created_at: time::now_str(),
            updated_at: time::now_str(),
        });
    Ok(Json(SettingResponse { data: footer }))
}

/// PUT /api/settings/footer
#[worker::send]
pub async fn update_footer(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<FooterInput>,
) -> AppResult<Json<SettingResponse<SiteFooter>>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    let existing = db
        .prepare("SELECT id FROM site_footer LIMIT 1")
        .all()
        .await?;
    let existing_rows: Vec<serde_json::Value> = existing.results()?;

    if let Some(row) = existing_rows.first() {
        let id = row["id"].as_str().unwrap_or("");
        db.prepare(
            "UPDATE site_footer SET content = ?, copyright_text = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&[
            body.content.unwrap_or_default().into(),
            body.copyright_text.unwrap_or_default().into(),
            now.into(),
            id.into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    } else {
        let id = new_id();
        db.prepare(
            "INSERT INTO site_footer (id, content, copyright_text, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&[
            id.as_str().into(),
            body.content.unwrap_or_default().into(),
            body.copyright_text.unwrap_or_default().into(),
            now.clone().into(),
            now.into(),
        ])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    }

    let footer = get_singleton::<SiteFooter>(&env, "site_footer")
        .await?
        .ok_or_else(|| AppError::Internal("读取页脚信息失败".into()))?;
    Ok(Json(SettingResponse { data: footer }))
}

// ═══════════════════════════════════════════════════════════════
//  页脚链接 Footer Links
// ═══════════════════════════════════════════════════════════════

/// GET /api/settings/footer-links（公开）
#[worker::send]
pub async fn list_footer_links(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<Vec<FooterLink>>> {
    let db = get_db(&env, Db::Core)?;
    let result = db
        .prepare("SELECT * FROM footer_links WHERE deleted_at IS NULL ORDER BY sort_order")
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    let links: Vec<FooterLink> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    Ok(Json(links))
}

/// POST /api/settings/footer-links
#[worker::send]
pub async fn create_footer_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<FooterLinkInput>,
) -> AppResult<Json<FooterLink>> {
    let db = get_db(&env, Db::Core)?;
    let id = new_id();
    let now = time::now_str();
    let sort = body.sort_order.unwrap_or(0);
    db.prepare(
        "INSERT INTO footer_links (id, name, url, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.name.into(),
        body.url.into(),
        sort.into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM footer_links WHERE id = ?")
        .bind(&[id.clone().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    rows.into_iter()
        .next()
        .map(|v| serde_json::from_value(v))
        .transpose()?
        .ok_or_else(|| AppError::Internal("创建页脚链接失败".into()))
        .map(Json)
}

/// PUT /api/settings/footer-links/:id
#[worker::send]
pub async fn update_footer_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<FooterLinkInput>,
) -> AppResult<Json<FooterLink>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    let sort = body.sort_order.unwrap_or(0);
    db.prepare(
        "UPDATE footer_links SET name = ?, url = ?, sort_order = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&[
        body.name.into(),
        body.url.into(),
        sort.into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM footer_links WHERE id = ?")
        .bind(&[param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    rows.into_iter()
        .next()
        .map(|v| serde_json::from_value(v))
        .transpose()?
        .ok_or_else(|| AppError::NotFound("页脚链接不存在".into()))
        .map(Json)
}

/// DELETE /api/settings/footer-links/:id
#[worker::send]
pub async fn delete_footer_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    db.prepare("UPDATE footer_links SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}

// ═══════════════════════════════════════════════════════════════
//  社交链接 Social Links
// ═══════════════════════════════════════════════════════════════

/// GET /api/settings/social-links（公开）
#[worker::send]
pub async fn list_social_links(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<Vec<SocialLink>>> {
    let db = get_db(&env, Db::Core)?;
    let result = db
        .prepare("SELECT * FROM social_links WHERE deleted_at IS NULL ORDER BY sort_order")
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    let links: Vec<SocialLink> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    Ok(Json(links))
}

/// POST /api/settings/social-links
#[worker::send]
pub async fn create_social_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<SocialLinkInput>,
) -> AppResult<Json<SocialLink>> {
    let db = get_db(&env, Db::Core)?;
    let id = new_id();
    let now = time::now_str();
    let sort = body.sort_order.unwrap_or(0);
    db.prepare(
        "INSERT INTO social_links (id, platform, label, url, icon, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.platform.into(),
        body.label.unwrap_or_default().into(),
        body.url.into(),
        body.icon.unwrap_or_default().into(),
        sort.into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM social_links WHERE id = ?")
        .bind(&[id.clone().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    rows.into_iter()
        .next()
        .map(|v| serde_json::from_value(v))
        .transpose()?
        .ok_or_else(|| AppError::Internal("创建社交链接失败".into()))
        .map(Json)
}

/// PUT /api/settings/social-links/:id
#[worker::send]
pub async fn update_social_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<SocialLinkInput>,
) -> AppResult<Json<SocialLink>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    let sort = body.sort_order.unwrap_or(0);
    db.prepare(
        "UPDATE social_links SET platform = ?, label = ?, url = ?, icon = ?, sort_order = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&[
        body.platform.into(),
        body.label.unwrap_or_default().into(),
        body.url.into(),
        body.icon.unwrap_or_default().into(),
        sort.into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM social_links WHERE id = ?")
        .bind(&[param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    rows.into_iter()
        .next()
        .map(|v| serde_json::from_value(v))
        .transpose()?
        .ok_or_else(|| AppError::NotFound("社交链接不存在".into()))
        .map(Json)
}

/// DELETE /api/settings/social-links/:id
#[worker::send]
pub async fn delete_social_link(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    db.prepare("UPDATE social_links SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}

// ═══════════════════════════════════════════════════════════════
//  公告 Announcements
// ═══════════════════════════════════════════════════════════════

/// GET /api/settings/announcements（公开）
#[worker::send]
pub async fn list_announcements(
    State(env): State<Arc<Env>>,
) -> AppResult<Json<Vec<Announcement>>> {
    let db = get_db(&env, Db::Core)?;
    let result = db
        .prepare("SELECT * FROM announcements WHERE deleted_at IS NULL ORDER BY sort_order")
        .all()
        .await?;
    let rows: Vec<AnnouncementRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let list: Vec<Announcement> = rows.into_iter().map(Announcement::from).collect();
    Ok(Json(list))
}

/// POST /api/settings/announcements（管理）
#[worker::send]
pub async fn create_announcement(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<AnnouncementInput>,
) -> AppResult<Json<Announcement>> {
    let db = get_db(&env, Db::Core)?;
    let id = new_id();
    let now = time::now_str();
    let sort = body.sort_order.unwrap_or(0);
    let active = if body.active.unwrap_or(false) { 1i32 } else { 0i32 };
    db.prepare(
        "INSERT INTO announcements (id, content, active, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        body.content.into(),
        active.into(),
        sort.into(),
        now.clone().into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM announcements WHERE id = ?")
        .bind(&[id.clone().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<AnnouncementRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    rows.into_iter()
        .next()
        .map(Announcement::from)
        .map(Json)
        .ok_or_else(|| AppError::Internal("创建公告失败".into()))
}

/// PUT /api/settings/announcements/:id
#[worker::send]
pub async fn update_announcement(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
    Json(body): Json<AnnouncementInput>,
) -> AppResult<Json<Announcement>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    let sort = body.sort_order.unwrap_or(0);
    let active = if body.active.unwrap_or(false) { 1i32 } else { 0i32 };
    db.prepare(
        "UPDATE announcements SET content = ?, active = ?, sort_order = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&[
        body.content.into(),
        active.into(),
        sort.into(),
        now.into(),
        param.id.as_str().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let result = db
        .prepare("SELECT * FROM announcements WHERE id = ?")
        .bind(&[param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<AnnouncementRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    rows.into_iter()
        .next()
        .map(Announcement::from)
        .map(Json)
        .ok_or_else(|| AppError::NotFound("公告不存在".into()))
}

/// DELETE /api/settings/announcements/:id
#[worker::send]
pub async fn delete_announcement(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<IdParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Core)?;
    let now = time::now_str();
    db.prepare("UPDATE announcements SET deleted_at = ? WHERE id = ?")
        .bind(&[now.into(), param.id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}
