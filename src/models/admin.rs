use serde::{Deserialize, Serialize};

// ─── 管理员 ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Admin {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

/// 数据库查询结果（SQLite 用整数表示布尔值）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdminRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub totp_enabled: i32, // SQLite 0/1
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl From<AdminRow> for Admin {
    fn from(row: AdminRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            email: row.email,
            password_hash: row.password_hash,
            totp_secret: row.totp_secret,
            totp_enabled: row.totp_enabled != 0,
            role: row.role,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

/// 管理员信息（脱敏，返回给前端）
#[derive(Debug, Serialize)]
pub struct AdminInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub totp_enabled: bool,
    pub role: String,
    pub created_at: String,
}

impl From<Admin> for AdminInfo {
    fn from(a: Admin) -> Self {
        Self {
            id: a.id,
            username: a.username,
            email: a.email,
            totp_enabled: a.totp_enabled,
            role: a.role,
            created_at: a.created_at,
        }
    }
}

// ─── 登录日志 ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginLog {
    pub id: String,
    pub admin_id: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub status: String,
    pub created_at: String,
}

// ─── 验证码 ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationCode {
    pub id: String,
    pub admin_id: String,
    pub code: String,
    #[serde(rename = "type")]
    pub code_type: String,
    pub expires_at: String,
    pub used_at: Option<String>,
    pub created_at: String,
}
