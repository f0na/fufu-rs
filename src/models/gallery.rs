use serde::{Deserialize, Serialize};

/// 数据库行记录
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GalleryRow {
    pub id: String,
    pub title: String,
    pub cover_path: String,
    pub tags: String, // JSON array string from DB
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// 业务模型
#[derive(Debug, Serialize, Clone)]
pub struct Gallery {
    pub id: String,
    pub title: String,
    pub cover_path: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl From<GalleryRow> for Gallery {
    fn from(row: GalleryRow) -> Self {
        let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();
        Self {
            id: row.id,
            title: row.title,
            cover_path: row.cover_path,
            tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Photo {
    pub id: String,
    pub gallery_id: String,
    pub path: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
}
