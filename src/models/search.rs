use serde::Serialize;

/// 统一的搜索结果项
#[derive(Debug, Serialize)]
pub struct SearchItem {
    pub r#type: String,
    pub title: String,
    pub url: Option<String>,
    pub snippet: String,
    pub published_at: Option<String>,
}

/// 搜索分页响应
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub data: Vec<SearchItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
    pub query: String,
}
