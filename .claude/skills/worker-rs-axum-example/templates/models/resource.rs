// src/models/resource.rs - Resource Models and DTOs

use serde::{Deserialize, Serialize};

/// Database resource entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub created_at: String,
}

/// Create resource request payload
#[derive(Debug, Deserialize)]
pub struct CreateResourceRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

/// Update resource request payload
#[derive(Debug, Deserialize)]
pub struct UpdateResourceRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

/// Resource response with owner info
#[derive(Debug, Serialize)]
pub struct ResourceResponse {
    pub id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub created_at: String,
    pub username: Option<String>,
}