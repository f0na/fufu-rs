use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasm_bindgen::JsValue;
use worker::Env;

use crate::auth::jwt::{Claims, OptionalClaims};
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::kv::KvCache;
use crate::models::post::{PaginatedPosts, Post, PostRow, PostSummary};
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct PostListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    tag: Option<String>,
    year: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
pub struct CreatePostInput {
    title: String,
    slug: Option<String>,
    content: String,
    excerpt: Option<String>,
    tags: Option<Vec<String>>,
    status: Option<String>,
    github_discussion_number: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdatePostInput {
    title: Option<String>,
    slug: Option<String>,
    content: Option<String>,
    excerpt: Option<String>,
    tags: Option<Vec<String>>,
    status: Option<String>,
    published_at: Option<String>,
    github_discussion_number: Option<i64>,
}

#[derive(Deserialize)]
pub struct SlugParam {
    slug: String,
}

#[derive(Serialize)]
pub struct CommentCount {
    pub count: i64,
}

// ─── 工具函数 ─────────────────────────────────────────────────


fn new_uuid() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn tags_to_json(tags: &Option<Vec<String>>) -> String {
    serde_json::to_string(tags.as_deref().unwrap_or(&[])).unwrap_or("[]".into())
}

/// 检查 slug 是否已存在
async fn slug_exists(env: &Arc<Env>, slug: &str) -> AppResult<bool> {
    let db = get_db(env, Db::Posts)?;
    let result = db
        .prepare("SELECT 1 FROM posts WHERE slug = ?")
        .bind(&[slug.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    Ok(!rows.is_empty())
}

/// 检查其他文章是否已使用此 slug（更新时用）
async fn slug_exists_excluding(env: &Arc<Env>, slug: &str, exclude_id: &str) -> AppResult<bool> {
    let db = get_db(env, Db::Posts)?;
    let result = db
        .prepare("SELECT 1 FROM posts WHERE slug = ? AND id != ?")
        .bind(&[slug.into(), exclude_id.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    Ok(!rows.is_empty())
}

/// 在 slug 末尾追加随机后缀
fn uniquify_slug(slug: &str) -> String {
    let suffix = &uuid::Uuid::now_v7().to_string()[..6];
    format!("{}-{}", slug, suffix)
}

async fn find_by_slug(env: &Arc<Env>, slug: &str) -> AppResult<Option<Post>> {
    let db = get_db(env, Db::Posts)?;
    let result = db
        .prepare("SELECT * FROM posts WHERE slug = ? AND deleted_at IS NULL")
        .bind(&[slug.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<PostRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next().map(Post::from))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/posts — 列表
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn list_posts(
    _auth: OptionalClaims,
    State(env): State<Arc<Env>>,
    Query(query): Query<PostListQuery>,
) -> AppResult<Json<PaginatedPosts>> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(10).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let mut conditions = vec!["deleted_at IS NULL".to_string()];
    let mut params: Vec<JsValue> = Vec::new();

    // 状态
    let status = query.status.as_deref().unwrap_or("published");
    conditions.push("status = ?".into());
    params.push(status.into());

    // 标签
    if let Some(ref tag) = query.tag {
        conditions.push("tags LIKE ?".into());
        params.push(format!("%\"{}\"%", tag).into());
    }

    // 年份
    if let Some(ref year) = query.year {
        conditions.push("published_at LIKE ?".into());
        params.push(format!("{}%", year).into());
    }

    let where_clause = conditions.join(" AND ");
    let db = get_db(&env, Db::Posts)?;

    // 总数
    let total: i64 = {
        let count_sql = format!("SELECT COUNT(*) as cnt FROM posts WHERE {}", where_clause);
        let stmt = db.prepare(&count_sql);
        let result = stmt.bind(&params).map_err(|e| AppError::Internal(e.to_string()))?.all().await?;
        let rows: Vec<serde_json::Value> = result.results()?;
        rows.first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };

    // 数据
    let data_sql = format!(
        "SELECT * FROM posts WHERE {} ORDER BY created_at DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let mut data_params = params.clone();
    data_params.push(JsValue::from(page_size as f64));
    data_params.push(JsValue::from(offset as f64));

    let stmt = db.prepare(&data_sql);
    let result = stmt.bind(&data_params).map_err(|e| AppError::Internal(e.to_string()))?.all().await?;
    let rows: Vec<PostRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    let posts: Vec<PostSummary> = rows.into_iter().map(|r| PostSummary::from(Post::from(r))).collect();

    let total_pages = if total == 0 { 0 } else { (total - 1) / page_size + 1 };

    Ok(Json(PaginatedPosts {
        data: posts,
        total,
        page,
        page_size,
        total_pages,
    }))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/posts/{slug} — 详情
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn get_post(
    State(env): State<Arc<Env>>,
    Path(param): Path<SlugParam>,
) -> AppResult<Json<Post>> {
    let post = find_by_slug(&env, &param.slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".into()))?;
    Ok(Json(post))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/posts — 创建
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn create_post(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<CreatePostInput>,
) -> AppResult<Json<Post>> {
    let db = get_db(&env, Db::Posts)?;
    let id = new_uuid();
    let now = time::now_str();
    let slug = body.slug.unwrap_or_else(|| slugify(&body.title));

    // slug 冲突时自动追加随机后缀
    let slug = if slug_exists(&env, &slug).await? {
        let mut final_slug = slug.clone();
        for _ in 0..10 {
            final_slug = uniquify_slug(&slug);
            if !slug_exists(&env, &final_slug).await? {
                break;
            }
        }
        final_slug
    } else {
        slug
    };
    let excerpt = body.excerpt.unwrap_or_else(|| {
        body.content.chars().take(200).collect::<String>()
    });
    let status = body.status.unwrap_or_else(|| "draft".into());
    let published_at = if status == "published" {
        Some(now.clone())
    } else {
        None
    };
    let tags = tags_to_json(&body.tags);

    db.prepare(
        "INSERT INTO posts (id, title, slug, content, excerpt, tags, status, github_discussion_number, created_at, updated_at, published_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.into(),
        body.title.clone().into(),
        slug.as_str().into(),
        body.content.into(),
        excerpt.into(),
        tags.into(),
        status.into(),
        body.github_discussion_number.map(|n| JsValue::from(n as f64)).unwrap_or(JsValue::null()),
        now.clone().into(),
        now.into(),
        published_at.map(|p| JsValue::from_str(&p)).unwrap_or(JsValue::null()),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    // 用已确定的 slug 查询返回完整数据
    let post = find_by_slug(&env, &slug).await?
        .ok_or_else(|| AppError::Internal("创建文章失败".into()))?;
    Ok(Json(post))
}

// ═══════════════════════════════════════════════════════════════
//  PUT /api/posts/{slug} — 更新
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn update_post(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<SlugParam>,
    Json(body): Json<UpdatePostInput>,
) -> AppResult<Json<Post>> {
    let existing = find_by_slug(&env, &param.slug)
        .await?
        .ok_or_else(|| AppError::NotFound("文章不存在".into()))?;

    let db = get_db(&env, Db::Posts)?;
    let now = time::now_str();
    let title = body.title.unwrap_or(existing.title);
    let slug = if let Some(new_slug) = body.slug {
        if new_slug != existing.slug && slug_exists_excluding(&env, &new_slug, &existing.id).await? {
            let mut final_slug = new_slug.clone();
            for _ in 0..10 {
                final_slug = uniquify_slug(&new_slug);
                if !slug_exists_excluding(&env, &final_slug, &existing.id).await? {
                    break;
                }
            }
            final_slug
        } else {
            new_slug
        }
    } else {
        existing.slug
    };
    let content = body.content.unwrap_or(existing.content);
    let excerpt = body.excerpt.unwrap_or(existing.excerpt);
    let tags = tags_to_json(&body.tags);
    let status = body.status.unwrap_or(existing.status);
    let published_at = body.published_at.or(existing.published_at);
    let gh_disc = body.github_discussion_number.or(existing.github_discussion_number);

    db.prepare(
        "UPDATE posts SET title = ?, slug = ?, content = ?, excerpt = ?, tags = ?, status = ?, \
         github_discussion_number = ?, updated_at = ?, published_at = ? WHERE id = ?",
    )
    .bind(&[
        title.into(),
        slug.as_str().into(),
        content.into(),
        excerpt.into(),
        tags.into(),
        status.into(),
        gh_disc.map(|n| JsValue::from(n as f64)).unwrap_or(JsValue::null()),
        now.into(),
        published_at.as_deref().map(JsValue::from_str).unwrap_or(JsValue::null()),
        existing.id.into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    let post = find_by_slug(&env, &slug).await?
        .ok_or_else(|| AppError::Internal("更新文章失败".into()))?;
    Ok(Json(post))
}

// ═══════════════════════════════════════════════════════════════
//  DELETE /api/posts/{slug} — 逻辑删除
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn delete_post(
    _claims: Claims,
    State(env): State<Arc<Env>>,
    Path(param): Path<SlugParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Posts)?;
    let now = time::now_str();
    db.prepare("UPDATE posts SET deleted_at = ? WHERE slug = ?")
        .bind(&[now.into(), param.slug.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "已删除" })))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/posts/{slug}/views — 增加浏览量
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn increment_views(
    State(env): State<Arc<Env>>,
    Path(param): Path<SlugParam>,
) -> AppResult<Json<serde_json::Value>> {
    let db = get_db(&env, Db::Posts)?;
    db.prepare("UPDATE posts SET view_count = view_count + 1 WHERE slug = ? AND deleted_at IS NULL")
        .bind(&[param.slug.into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .run()
        .await?;
    Ok(Json(serde_json::json!({ "message": "ok" })))
}

// ═══════════════════════════════════════════════════════════════
//  GET /api/posts/{slug}/comments-count — GitHub 评论数代理
// ═══════════════════════════════════════════════════════════════

#[worker::send]
pub async fn comments_count(
    State(env): State<Arc<Env>>,
    Path(param): Path<SlugParam>,
) -> AppResult<Json<CommentCount>> {
    // 先查文章获取 discussion_number
    let post = find_by_slug(&env, &param.slug).await?;
    let discussion_number = match post {
        Some(ref p) => p.github_discussion_number,
        None => return Ok(Json(CommentCount { count: 0 })),
    };

    let number = match discussion_number {
        Some(n) => n,
        None => return Ok(Json(CommentCount { count: 0 })),
    };

    // 尝试从 KV 缓存读取
    let kv = KvCache::new(&env).ok();
    if let Some(ref kv) = kv {
        let cache_key = format!("gh_comments:{}", number);
        if let Some(cached) = kv.get_str(&cache_key).await? {
            if let Ok(count) = cached.parse::<i64>() {
                return Ok(Json(CommentCount { count }));
            }
        }
    }

    // 从 GitHub API 获取
    let count = fetch_github_comment_count(&env, number).await;

    // 写入缓存（1 小时）
    if let Some(ref kv) = kv {
        let cache_key = format!("gh_comments:{}", number);
        let _ = kv.put_str(&cache_key, &count.to_string(), 3600).await;
    }

    Ok(Json(CommentCount { count }))
}

async fn fetch_github_comment_count(env: &Arc<Env>, discussion_number: i64) -> i64 {
    let token = match env.var("GITHUB_TOKEN") {
        Ok(t) => t.to_string(),
        Err(_) => return 0,
    };
    let repo = match env.var("GITHUB_REPO") {
        Ok(r) => r.to_string(),
        Err(_) => return 0,
    };

    let query = serde_json::json!({
        "query": format!(
            "{{ repository(owner: \"{}\", name: \"{}\") {{ discussion(number: {}) {{ comments {{ totalCount }} }} }} }}",
            repo.split('/').next().unwrap_or(""),
            repo.split('/').nth(1).unwrap_or(""),
            discussion_number
        )
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.github.com/graphql")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "fufu-rs/1.0")
        .json(&query)
        .send()
        .await;

    match resp {
        Ok(r) => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                let count = json["data"]["repository"]["discussion"]["comments"]["totalCount"]
                    .as_i64()
                    .unwrap_or(0);
                return count;
            }
            0
        }
        Err(_) => 0,
    }
}
