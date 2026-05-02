use serde::{Deserialize, Serialize};

// ─── 站点信息 ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteProfile {
    pub id: String,
    pub site_name: String,
    pub subtitle: String,
    pub logo_url: String,
    pub description: String,
    pub keywords: String,
    pub icp_beian: String,
    pub created_at: String,
    pub updated_at: String,
}

// ─── 页脚配置 ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteFooter {
    pub id: String,
    pub content: String,
    pub copyright_text: String,
    pub created_at: String,
    pub updated_at: String,
}

// ─── 页脚链接 ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FooterLink {
    pub id: String,
    pub name: String,
    pub url: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

// ─── 社交链接 ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SocialLink {
    pub id: String,
    pub platform: String,
    pub label: String,
    pub url: String,
    pub icon: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

// ─── 公告 ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Announcement {
    pub id: String,
    pub content: String,
    pub active: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnnouncementRow {
    pub id: String,
    pub content: String,
    pub active: i32, // SQLite 0/1
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl From<AnnouncementRow> for Announcement {
    fn from(row: AnnouncementRow) -> Self {
        Self {
            id: row.id,
            content: row.content,
            active: row.active != 0,
            sort_order: row.sort_order,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}
