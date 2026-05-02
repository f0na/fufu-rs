use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use wasm_bindgen::JsValue;
use worker::Env;

use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::models::search::{SearchItem, SearchResponse};

// ─── 常量 ───────────────────────────────────────────────────────

const MAX_PAGE_SIZE: i64 = 100;
const DEFAULT_PAGE_SIZE: i64 = 10;
const PER_SOURCE_LIMIT: i64 = 50;
const MIN_QUERY_LENGTH: usize = 2;

// ─── 请求类型 ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

// ─── 内部辅助类型 ───────────────────────────────────────────────

struct RankedItem {
    item: SearchItem,
    relevance: i64,
}

// ─── 工具函数 ───────────────────────────────────────────────────

/// 转义 LIKE 通配符
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// ─── SQL 查询 ───────────────────────────────────────────────────

// 文章：搜索 title / content / excerpt，按权重 + 发布时间排序
const SQL_POSTS: &str = r#"SELECT *,
  CASE WHEN title LIKE ? ESCAPE '\' THEN 3
       WHEN excerpt LIKE ? ESCAPE '\' THEN 2
       WHEN content LIKE ? ESCAPE '\' THEN 1 ELSE 0 END as relevance
FROM posts
WHERE deleted_at IS NULL AND status = 'published'
  AND (title LIKE ? ESCAPE '\' OR excerpt LIKE ? ESCAPE '\' OR content LIKE ? ESCAPE '\')
ORDER BY relevance DESC, published_at DESC
LIMIT ? OFFSET ?"#;

const SQL_POSTS_COUNT: &str = r#"SELECT COUNT(*) as cnt FROM posts
WHERE deleted_at IS NULL AND status = 'published'
  AND (title LIKE ? ESCAPE '\' OR excerpt LIKE ? ESCAPE '\' OR content LIKE ? ESCAPE '\')"#;

// 收藏链接：搜索 title / description
const SQL_LINKS: &str = r#"SELECT *,
  CASE WHEN title LIKE ? ESCAPE '\' THEN 3
       WHEN description LIKE ? ESCAPE '\' THEN 2 ELSE 0 END as relevance
FROM links
WHERE deleted_at IS NULL
  AND (title LIKE ? ESCAPE '\' OR description LIKE ? ESCAPE '\')
ORDER BY relevance DESC, created_at DESC
LIMIT ? OFFSET ?"#;

const SQL_LINKS_COUNT: &str = r#"SELECT COUNT(*) as cnt FROM links
WHERE deleted_at IS NULL
  AND (title LIKE ? ESCAPE '\' OR description LIKE ? ESCAPE '\')"#;

// 相册：搜索 title
const SQL_GALLERIES: &str = r#"SELECT *,
  CASE WHEN title LIKE ? ESCAPE '\' THEN 3 ELSE 0 END as relevance
FROM galleries
WHERE deleted_at IS NULL
  AND title LIKE ? ESCAPE '\'
ORDER BY created_at DESC
LIMIT ? OFFSET ?"#;

const SQL_GALLERIES_COUNT: &str = r#"SELECT COUNT(*) as cnt FROM galleries
WHERE deleted_at IS NULL
  AND title LIKE ? ESCAPE '\'"#;

// 友人帐：搜索 name / description
const SQL_FRIENDS: &str = r#"SELECT *,
  CASE WHEN name LIKE ? ESCAPE '\' THEN 3
       WHEN description LIKE ? ESCAPE '\' THEN 2 ELSE 0 END as relevance
FROM friends
WHERE deleted_at IS NULL AND status = 'approved'
  AND (name LIKE ? ESCAPE '\' OR description LIKE ? ESCAPE '\')
ORDER BY relevance DESC, created_at DESC
LIMIT ? OFFSET ?"#;

const SQL_FRIENDS_COUNT: &str = r#"SELECT COUNT(*) as cnt FROM friends
WHERE deleted_at IS NULL AND status = 'approved'
  AND (name LIKE ? ESCAPE '\' OR description LIKE ? ESCAPE '\')"#;

// 公告：搜索 content
const SQL_ANNOUNCEMENTS: &str = r#"SELECT *,
  CASE WHEN content LIKE ? ESCAPE '\' THEN 1 ELSE 0 END as relevance
FROM announcements
WHERE deleted_at IS NULL AND active = 1
  AND content LIKE ? ESCAPE '\'
ORDER BY created_at DESC
LIMIT ? OFFSET ?"#;

const SQL_ANNOUNCEMENTS_COUNT: &str = r#"SELECT COUNT(*) as cnt FROM announcements
WHERE deleted_at IS NULL AND active = 1
  AND content LIKE ? ESCAPE '\'"#;

// ─── 各类型搜索函数 ─────────────────────────────────────────────

/// 构建 LIKE 参数列表：like_count 个 pattern + limit + offset
fn bind_like(pattern: &str, like_count: usize, limit: i64, offset: i64) -> Vec<JsValue> {
    let mut params = Vec::with_capacity(like_count + 2);
    for _ in 0..like_count {
        params.push(pattern.to_string().into());
    }
    params.push(JsValue::from(limit as f64));
    params.push(JsValue::from(offset as f64));
    params
}

/// 构建仅 COUNT 查询的 LIKE 参数
fn bind_like_count(pattern: &str, like_count: usize) -> Vec<JsValue> {
    let mut params = Vec::with_capacity(like_count);
    for _ in 0..like_count {
        params.push(pattern.to_string().into());
    }
    params
}

async fn query_items(
    env: &Arc<Env>,
    db: Db,
    sql: &str,
    pattern: &str,
    like_count: usize,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<serde_json::Value>> {
    let database = get_db(env, db)?;
    let params = bind_like(pattern, like_count, limit, offset);
    let stmt = database.prepare(sql);
    let result = stmt
        .bind(&params)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    Ok(result.results()?)
}

async fn query_count(
    env: &Arc<Env>,
    db: Db,
    sql: &str,
    pattern: &str,
    like_count: usize,
) -> AppResult<i64> {
    let database = get_db(env, db)?;
    let params = bind_like_count(pattern, like_count);
    let result = database
        .prepare(sql)
        .bind(&params)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    Ok(rows
        .first()
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0))
}

// ─── 各类型结果转换 ─────────────────────────────────────────────

fn to_post_item(row: &serde_json::Value) -> RankedItem {
    let relevance = row.get("relevance").and_then(|v| v.as_i64()).unwrap_or(0);
    let excerpt = row.get("excerpt").and_then(|v| v.as_str()).unwrap_or("");
    let snippet = if excerpt.is_empty() {
        let content = row.get("content").and_then(|v| v.as_str()).unwrap_or("");
        if content.len() > 150 {
            format!("…{}…", &content[..150])
        } else {
            content.to_string()
        }
    } else {
        excerpt.to_string()
    };
    RankedItem {
        item: SearchItem {
            r#type: "post".into(),
            title: row.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: row
                .get("slug")
                .and_then(|v| v.as_str())
                .map(|s| format!("/posts/{}", s)),
            snippet,
            published_at: row.get("published_at").and_then(|v| v.as_str()).map(String::from),
        },
        relevance,
    }
}

fn to_link_item(row: &serde_json::Value) -> RankedItem {
    let relevance = row.get("relevance").and_then(|v| v.as_i64()).unwrap_or(0);
    let desc = row.get("description").and_then(|v| v.as_str()).unwrap_or("");
    RankedItem {
        item: SearchItem {
            r#type: "link".into(),
            title: row.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: row.get("url").and_then(|v| v.as_str()).map(String::from),
            snippet: desc.to_string(),
            published_at: row.get("created_at").and_then(|v| v.as_str()).map(String::from),
        },
        relevance,
    }
}

fn to_gallery_item(row: &serde_json::Value) -> RankedItem {
    let relevance = row.get("relevance").and_then(|v| v.as_i64()).unwrap_or(0);
    let title = row.get("title").and_then(|v| v.as_str()).unwrap_or("");
    RankedItem {
        item: SearchItem {
            r#type: "gallery".into(),
            title: title.to_string(),
            url: None,
            snippet: title.to_string(),
            published_at: row.get("created_at").and_then(|v| v.as_str()).map(String::from),
        },
        relevance,
    }
}

fn to_friend_item(row: &serde_json::Value) -> RankedItem {
    let relevance = row.get("relevance").and_then(|v| v.as_i64()).unwrap_or(0);
    let desc = row.get("description").and_then(|v| v.as_str()).unwrap_or("");
    RankedItem {
        item: SearchItem {
            r#type: "friend".into(),
            title: row.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: row.get("url").and_then(|v| v.as_str()).map(String::from),
            snippet: desc.to_string(),
            published_at: row.get("created_at").and_then(|v| v.as_str()).map(String::from),
        },
        relevance,
    }
}

fn to_announcement_item(row: &serde_json::Value) -> RankedItem {
    let relevance = row.get("relevance").and_then(|v| v.as_i64()).unwrap_or(0);
    let content = row.get("content").and_then(|v| v.as_str()).unwrap_or("");
    // 公告无标题，用内容前 50 字符作为标题
    let title = if content.len() > 50 {
        format!("{}…", &content[..50])
    } else {
        content.to_string()
    };
    RankedItem {
        item: SearchItem {
            r#type: "announcement".into(),
            title,
            url: None,
            snippet: content.to_string(),
            published_at: row.get("created_at").and_then(|v| v.as_str()).map(String::from),
        },
        relevance,
    }
}

// ─── 搜索入口 ───────────────────────────────────────────────────

#[worker::send]
pub async fn search_content(
    State(env): State<Arc<Env>>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<SearchResponse>> {
    let q = query.q.ok_or_else(|| AppError::BadRequest("请输入搜索关键词".into()))?;
    if q.len() < MIN_QUERY_LENGTH {
        return Err(AppError::BadRequest("关键词至少需要 2 个字符".into()));
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    let pattern = format!("%{}%", escape_like(&q));

    // 并发搜索各内容类型
    let (posts, links, galleries, friends, announcements) = futures::join!(
        search_posts(&env, &pattern, PER_SOURCE_LIMIT, 0),
        search_links(&env, &pattern, PER_SOURCE_LIMIT, 0),
        search_galleries(&env, &pattern, PER_SOURCE_LIMIT, 0),
        search_friends(&env, &pattern, PER_SOURCE_LIMIT, 0),
        search_announcements(&env, &pattern, PER_SOURCE_LIMIT, 0),
    );

    // 并发计数
    let (total_posts, total_links, total_galleries, total_friends, total_announcements) = futures::join!(
        count_posts(&env, &pattern),
        count_links(&env, &pattern),
        count_galleries(&env, &pattern),
        count_friends(&env, &pattern),
        count_announcements(&env, &pattern),
    );

    let total = total_posts? + total_links? + total_galleries? + total_friends? + total_announcements?;

    // 合并所有结果
    let mut all: Vec<RankedItem> = Vec::new();
    if let Ok(items) = posts {
        all.extend(items);
    }
    if let Ok(items) = links {
        all.extend(items);
    }
    if let Ok(items) = galleries {
        all.extend(items);
    }
    if let Ok(items) = friends {
        all.extend(items);
    }
    if let Ok(items) = announcements {
        all.extend(items);
    }

    // 按 relevance 降序 → published_at 降序排列
    all.sort_by(|a, b| {
        b.relevance
            .cmp(&a.relevance)
            .then_with(|| b.item.published_at.cmp(&a.item.published_at))
    });

    // 分页
    let data: Vec<SearchItem> = all
        .into_iter()
        .skip(offset as usize)
        .take(page_size as usize)
        .map(|r| r.item)
        .collect();

    let total_pages = if total == 0 { 0 } else { (total - 1) / page_size + 1 };

    Ok(Json(SearchResponse {
        data,
        total,
        page,
        page_size,
        total_pages,
        query: q,
    }))
}

// ─── 各类型搜索实现 ─────────────────────────────────────────────

async fn search_posts(env: &Arc<Env>, pattern: &str, limit: i64, offset: i64) -> AppResult<Vec<RankedItem>> {
    let rows = query_items(env, Db::Posts, SQL_POSTS, pattern, 6, limit, offset).await?;
    Ok(rows.iter().map(to_post_item).collect())
}

async fn count_posts(env: &Arc<Env>, pattern: &str) -> AppResult<i64> {
    query_count(env, Db::Posts, SQL_POSTS_COUNT, pattern, 3).await
}

async fn search_links(env: &Arc<Env>, pattern: &str, limit: i64, offset: i64) -> AppResult<Vec<RankedItem>> {
    let rows = query_items(env, Db::Social, SQL_LINKS, pattern, 4, limit, offset).await?;
    Ok(rows.iter().map(to_link_item).collect())
}

async fn count_links(env: &Arc<Env>, pattern: &str) -> AppResult<i64> {
    query_count(env, Db::Social, SQL_LINKS_COUNT, pattern, 2).await
}

async fn search_galleries(env: &Arc<Env>, pattern: &str, limit: i64, offset: i64) -> AppResult<Vec<RankedItem>> {
    let rows = query_items(env, Db::Media, SQL_GALLERIES, pattern, 2, limit, offset).await?;
    Ok(rows.iter().map(to_gallery_item).collect())
}

async fn count_galleries(env: &Arc<Env>, pattern: &str) -> AppResult<i64> {
    query_count(env, Db::Media, SQL_GALLERIES_COUNT, pattern, 1).await
}

async fn search_friends(env: &Arc<Env>, pattern: &str, limit: i64, offset: i64) -> AppResult<Vec<RankedItem>> {
    let rows = query_items(env, Db::Social, SQL_FRIENDS, pattern, 4, limit, offset).await?;
    Ok(rows.iter().map(to_friend_item).collect())
}

async fn count_friends(env: &Arc<Env>, pattern: &str) -> AppResult<i64> {
    query_count(env, Db::Social, SQL_FRIENDS_COUNT, pattern, 2).await
}

async fn search_announcements(env: &Arc<Env>, pattern: &str, limit: i64, offset: i64) -> AppResult<Vec<RankedItem>> {
    let rows = query_items(env, Db::Core, SQL_ANNOUNCEMENTS, pattern, 2, limit, offset).await?;
    Ok(rows.iter().map(to_announcement_item).collect())
}

async fn count_announcements(env: &Arc<Env>, pattern: &str) -> AppResult<i64> {
    query_count(env, Db::Core, SQL_ANNOUNCEMENTS_COUNT, pattern, 1).await
}
