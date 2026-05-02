use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BangumiRecord {
    pub id: String,
    pub subject_id: i64,
    pub title: String,
    pub status: String,
    pub progress: String,
    pub cover_url: String,
    pub fansub: String,
    pub added_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}
