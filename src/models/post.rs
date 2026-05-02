use serde::{Deserialize, Serialize};

/// 数据库行记录
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostRow {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: String,
    pub tags: String, // JSON array string from DB
    pub status: String,
    pub view_count: i64,
    pub github_discussion_number: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub deleted_at: Option<String>,
}

/// 业务模型
#[derive(Debug, Serialize, Clone)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub status: String,
    pub view_count: i64,
    pub github_discussion_number: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

impl From<PostRow> for Post {
    fn from(row: PostRow) -> Self {
        let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();
        Self {
            id: row.id,
            title: row.title,
            slug: row.slug,
            content: row.content,
            excerpt: row.excerpt,
            tags,
            status: row.status,
            view_count: row.view_count,
            github_discussion_number: row.github_discussion_number,
            created_at: row.created_at,
            updated_at: row.updated_at,
            published_at: row.published_at,
        }
    }
}

/// 文章列表项（不包含正文）
#[derive(Debug, Serialize)]
pub struct PostSummary {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub status: String,
    pub view_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

impl From<Post> for PostSummary {
    fn from(p: Post) -> Self {
        Self {
            id: p.id,
            title: p.title,
            slug: p.slug,
            excerpt: p.excerpt,
            tags: p.tags,
            status: p.status,
            view_count: p.view_count,
            created_at: p.created_at,
            updated_at: p.updated_at,
            published_at: p.published_at,
        }
    }
}

/// 分页响应
#[derive(Debug, Serialize)]
pub struct PaginatedPosts {
    pub data: Vec<PostSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}
