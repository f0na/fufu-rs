use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Friend {
    pub id: String,
    pub name: String,
    pub url: String,
    pub avatar_url: String,
    pub description: String,
    pub email: String,
    pub status: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}
