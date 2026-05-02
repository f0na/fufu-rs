use serde::{Deserialize, Serialize};

/// 数据库行记录
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LinkRow {
    pub id: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub favicon_url: String,
    pub tags: String, // JSON array string from DB
    pub favorite: i32,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// 业务模型
#[derive(Debug, Serialize, Clone)]
pub struct Link {
    pub id: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub favicon_url: String,
    pub tags: Vec<String>,
    pub favorite: i32,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl From<LinkRow> for Link {
    fn from(row: LinkRow) -> Self {
        let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();
        Self {
            id: row.id,
            title: row.title,
            url: row.url,
            description: row.description,
            favicon_url: row.favicon_url,
            tags,
            favorite: row.favorite,
            sort_order: row.sort_order,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}
