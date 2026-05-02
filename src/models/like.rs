use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LikeCount {
    pub target_type: String,
    pub target_id: String,
    pub count: i64,
}
