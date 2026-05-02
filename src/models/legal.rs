use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseVersion {
    pub id: String,
    pub version: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivacyVersion {
    pub id: String,
    pub version: String,
    pub date: String,
    pub content: String,
    pub created_at: String,
}
