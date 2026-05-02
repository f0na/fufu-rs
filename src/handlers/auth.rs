use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worker::{console_log, Env};

use crate::auth::{hash, jwt, totp};
use crate::db::{get_db, Db};
use crate::error::{AppError, AppResult};
use crate::kv::KvCache;
use crate::models::admin::{Admin, AdminInfo, AdminRow};
use crate::time;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    temp_token: String,
    require_2fa: bool,
    require_email_verify: bool,
}

#[derive(Deserialize)]
pub struct Login2faRequest {
    temp_token: String,
    code: String,
}

#[derive(Deserialize)]
pub struct LoginVerifyRequest {
    temp_token: String,
    code: String,
}


#[derive(Serialize)]
pub struct Setup2faResponse {
    secret: String,
    uri: String,
}

#[derive(Deserialize)]
pub struct Verify2faRequest {
    code: String,
}

#[derive(Deserialize)]
pub struct Disable2faRequest {
    password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    refresh_token: String,
}

#[derive(Deserialize)]
pub struct RegisterInput {
    username: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    refresh_token: String,
}

// ─── Helper ──────────────────────────────────────────────────

fn get_secret(env: &Arc<Env>) -> AppResult<String> {
    env.var("JWT_SECRET")
        .map(|v| v.to_string())
        .map_err(|_| AppError::Internal("JWT_SECRET 未配置".into()))
}

/// 从 D1 查询管理员（按邮箱，未被逻辑删除）
async fn find_admin_by_email(env: &Arc<Env>, email: &str) -> AppResult<Option<Admin>> {
    let db = get_db(env, Db::Auth)?;
    let result = db
        .prepare("SELECT * FROM admins WHERE email = ? AND deleted_at IS NULL")
        .bind(&[email.into()])
        .map_err(|e| AppError::Internal(format!("数据库查询失败: {}", e)))?
        .all()
        .await?;

    let rows: Vec<AdminRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next().map(Admin::from))
}

/// 从 D1 查询管理员（按 ID，未被逻辑删除）
async fn find_admin_by_id(env: &Arc<Env>, id: &str) -> AppResult<Option<Admin>> {
    let db = get_db(env, Db::Auth)?;
    let result = db
        .prepare("SELECT * FROM admins WHERE id = ? AND deleted_at IS NULL")
        .bind(&[id.into()])
        .map_err(|e| AppError::Internal(format!("数据库查询失败: {}", e)))?
        .all()
        .await?;

    let rows: Vec<AdminRow> = serde_json::from_value(serde_json::Value::Array(result.results()?))?;
    Ok(rows.into_iter().next().map(Admin::from))
}

/// 记录登录日志
async fn record_login_log(
    env: &Arc<Env>,
    admin_id: &str,
    ip: Option<String>,
    user_agent: Option<String>,
    status: &str,
) {
    let db = match get_db(env, Db::Auth) {
        Ok(db) => db,
        Err(_) => return,
    };
    let id = uuid::Uuid::now_v7().to_string();
    let now = time::now_str();
    let result = db
        .prepare(
            "INSERT INTO login_logs (id, admin_id, ip, user_agent, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&[
            id.into(),
            admin_id.into(),
            ip.unwrap_or_default().into(),
            user_agent.unwrap_or_default().into(),
            status.into(),
            now.into(),
        ]);
    if let Ok(stmt) = result {
        if let Err(e) = stmt.run().await {
            console_log!("run error: {}", e);
        }
    }
}

/// 生成 6 位邮箱验证码
fn generate_email_code() -> String {
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf).ok();
    let num = u32::from_ne_bytes(buf) % 1_000_000;
    format!("{:06}", num)
}

/// 存储验证码到 D1
async fn save_verification_code(env: &Arc<Env>, admin_id: &str, code: &str) -> AppResult<()> {
    let db = get_db(env, Db::Auth)?;
    let id = uuid::Uuid::now_v7().to_string();
    let now = time::now_str();
    let expires = time::now_str_add(chrono::Duration::minutes(10));
    db.prepare(
        "INSERT INTO verification_codes (id, admin_id, code, type, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        id.into(),
        admin_id.into(),
        code.into(),
        "email_verification".into(),
        expires.into(),
        now.into(),
    ])
    .map_err(|e| AppError::Internal(format!("验证码存储失败: {}", e)))?
    .run()
    .await?;
    Ok(())
}

/// 发送验证码邮件（通过 Mailchannels，Cloudflare Workers 免费内建）
/// 使用前需在域名 DNS 添加 SPF 记录，详见:
/// https://developers.cloudflare.com/pages/functions/mailchannels/
async fn send_email_code(env: &Arc<Env>, email: &str, code: &str) -> AppResult<()> {
    if env.var("MAIL_DISABLED").is_ok() {
        console_log!("[本地开发] 验证码 {} -> {} (跳过邮件发送)", code, email);
        return Ok(());
    }

    let mail_from = env
        .var("MAIL_FROM")
        .map_err(|_| AppError::Internal("MAIL_FROM 未配置".into()))?
        .to_string();
    let mail_from_name = env
        .var("MAIL_FROM_NAME")
        .ok()
        .map(|v| v.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Fufu".into());

    let body = serde_json::json!({
        "personalizations": [{
            "to": [{"email": email}]
        }],
        "from": {"email": mail_from, "name": mail_from_name},
        "subject": "【Fufu】登录验证码",
        "content": [{
            "type": "text/plain",
            "value": format!(
                "您的登录验证码为：{}\n\n该验证码有效期为 10 分钟，请勿泄露给他人。\n\n如果不是您本人操作，请忽略此邮件。",
                code
            )
        }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.mailchannels.net/tx/v1/send")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("发送邮件失败: {}", e)))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        console_log!("Mailchannels error: {}", text);
        return Err(AppError::Internal("发送邮件失败".into()));
    }

    console_log!("验证码已发送到 {}: {}", email, code);
    Ok(())
}

// ─── Handlers ────────────────────────────────────────────────

/// POST /api/auth/register — 注册（仅限无管理员时首次注册）
#[worker::send]
pub async fn register(
    State(env): State<Arc<Env>>,
    Json(body): Json<RegisterInput>,
) -> AppResult<Json<AdminInfo>> {
    let db = get_db(&env, Db::Auth)?;

    // 检查是否已有管理员
    let result = db
        .prepare("SELECT COUNT(*) as cnt FROM admins WHERE deleted_at IS NULL")
        .bind(&[])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let rows: Vec<serde_json::Value> = result.results()?;
    let count = rows
        .first()
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if count > 0 {
        return Err(AppError::Conflict("管理员已存在，无法重复注册".into()));
    }

    // 检查邮箱是否已存在
    let dup = db
        .prepare("SELECT COUNT(*) as cnt FROM admins WHERE email = ? AND deleted_at IS NULL")
        .bind(&[body.email.as_str().into()])
        .map_err(|e| AppError::Internal(e.to_string()))?
        .all()
        .await?;
    let dup_rows: Vec<serde_json::Value> = dup.results()?;
    let dup_count = dup_rows
        .first()
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if dup_count > 0 {
        return Err(AppError::Conflict("该邮箱已被注册".into()));
    }

    let id = uuid::Uuid::now_v7().to_string();
    let now = time::now_str();
    let password_hash = crate::auth::hash::hash_password(&body.password)?;

    let username = body.username;
    let email = body.email;

    db.prepare(
        "INSERT INTO admins (id, username, email, password_hash, role, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'admin', ?, ?)",
    )
    .bind(&[
        id.as_str().into(),
        username.clone().into(),
        email.clone().into(),
        password_hash.into(),
        now.clone().into(),
        now.clone().into(),
    ])
    .map_err(|e| AppError::Internal(e.to_string()))?
    .run()
    .await?;

    Ok(Json(AdminInfo {
        id,
        username,
        email,
        totp_enabled: false,
        role: "admin".into(),
        created_at: now,
    }))
}

/// POST /api/auth/login — 第一步：验证邮箱密码
#[worker::send]
pub async fn login(
    State(env): State<Arc<Env>>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let admin = find_admin_by_email(&env, &body.email)
        .await?
        .ok_or_else(|| {
            AppError::WrongCredentials
        })?;

    let valid = hash::verify_password(&body.password, &admin.password_hash)?;
    if !valid {
        record_login_log(&env, &admin.id, None, None, "failed").await;
        return Err(AppError::WrongCredentials);
    }

    let secret = get_secret(&env)?;

    if admin.totp_enabled {
        // 需要 TOTP 第二步
        let temp_token = jwt::generate_temp_token(&admin.id, "2fa", &secret)?;
        record_login_log(&env, &admin.id, None, None, "totp_required").await;
        Ok(Json(LoginResponse {
            temp_token,
            require_2fa: true,
            require_email_verify: false,
        }))
    } else {
        // 需要邮箱验证码第二步
        let code = generate_email_code();
        save_verification_code(&env, &admin.id, &code).await?;
        send_email_code(&env, &admin.email, &code).await?;

        let temp_token = jwt::generate_temp_token(&admin.id, "verify_email", &secret)?;
        record_login_log(&env, &admin.id, None, None, "totp_required").await;
        Ok(Json(LoginResponse {
            temp_token,
            require_2fa: false,
            require_email_verify: true,
        }))
    }
}

/// POST /api/auth/login/2fa — 第二步：TOTP 验证
#[worker::send]
pub async fn login_2fa(
    State(env): State<Arc<Env>>,
    Json(body): Json<Login2faRequest>,
) -> AppResult<Json<jwt::TokenPair>> {
    let secret = get_secret(&env)?;
    let temp_claims = jwt::verify_temp_token(&body.temp_token, &secret)?;

    if temp_claims.purpose.as_deref() != Some("2fa") {
        return Err(AppError::TempTokenExpired);
    }

    let admin = find_admin_by_id(&env, &temp_claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    let totp_secret = admin
        .totp_secret
        .as_deref()
        .ok_or(AppError::TotpInvalid)?;

    if !totp::verify_totp(totp_secret, &body.code)? {
        return Err(AppError::TotpInvalid);
    }

    let tokens = jwt::generate_token_pair(&admin.id, &admin.username, &secret)?;
    record_login_log(&env, &admin.id, None, None, "success").await;
    Ok(Json(tokens))
}

/// POST /api/auth/login/verify — 第二步：邮箱验证码验证
#[worker::send]
pub async fn login_verify(
    State(env): State<Arc<Env>>,
    Json(body): Json<LoginVerifyRequest>,
) -> AppResult<Json<jwt::TokenPair>> {
    let secret = get_secret(&env)?;
    let temp_claims = jwt::verify_temp_token(&body.temp_token, &secret)?;

    if temp_claims.purpose.as_deref() != Some("verify_email") {
        return Err(AppError::TempTokenExpired);
    }

    let admin = find_admin_by_id(&env, &temp_claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    // 验证邮箱验证码
    let db = get_db(&env, Db::Auth)?;
    let now = time::now_str();
    let result = db
        .prepare(
            "SELECT id FROM verification_codes \
             WHERE admin_id = ? AND code = ? AND type = 'email_verification' \
             AND expires_at > ? AND used_at IS NULL \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&[admin.id.clone().into(), body.code.into(), now.into()])
        .map_err(|e| AppError::Internal(format!("查询验证码失败: {}", e)))?
        .all()
        .await?;

    let rows: Vec<serde_json::Value> = result.results()?;
    let code_id = rows
        .first()
        .and_then(|r| r.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or(AppError::EmailCodeInvalid)?;

    // 标记验证码已使用
    let used_at = time::now_str();
    db.prepare("UPDATE verification_codes SET used_at = ? WHERE id = ?")
        .bind(&[used_at.into(), code_id.into()])
        .map_err(|e| AppError::Internal(format!("更新验证码状态失败: {}", e)))?
        .run()
        .await?;

    let tokens = jwt::generate_token_pair(&admin.id, &admin.username, &secret)?;
    record_login_log(&env, &admin.id, None, None, "success").await;
    Ok(Json(tokens))
}

/// POST /api/auth/2fa/setup — 生成 TOTP 密钥
#[worker::send]
pub async fn setup_2fa(
    claims: jwt::Claims,
    State(env): State<Arc<Env>>,
) -> AppResult<Json<Setup2faResponse>> {
    let admin = find_admin_by_id(&env, &claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    if admin.totp_enabled {
        return Err(AppError::Conflict("2FA 已经开启".into()));
    }

    let secret = totp::generate_secret()?;
    let uri = totp::provisioning_uri(&secret, &admin.email);

    // 暂存到环境变量中？不，需要持久化。但先不开启 totp_enabled
    // 将密钥保存到 admin 记录中但 totp_enabled 维持 false
    // 等到 verify 确认后才真正启用
    let now = time::now_str();
    let db = get_db(&env, Db::Auth)?;
    db.prepare("UPDATE admins SET totp_secret = ?, updated_at = ? WHERE id = ?")
        .bind(&[secret.clone().into(), now.into(), admin.id.into()])
        .map_err(|e| AppError::Internal(format!("保存 TOTP 密钥失败: {}", e)))?
        .run()
        .await?;

    Ok(Json(Setup2faResponse { secret, uri }))
}

/// POST /api/auth/2fa/verify — 确认开启 2FA
#[worker::send]
pub async fn verify_2fa(
    claims: jwt::Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<Verify2faRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let admin = find_admin_by_id(&env, &claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    let totp_secret = admin
        .totp_secret
        .as_deref()
        .ok_or(AppError::BadRequest("请先执行 2FA 设置".into()))?;

    if admin.totp_enabled {
        return Err(AppError::Conflict("2FA 已经开启".into()));
    }

    if !totp::verify_totp(totp_secret, &body.code)? {
        return Err(AppError::TotpInvalid);
    }

    let now = time::now_str();
    let db = get_db(&env, Db::Auth)?;
    db.prepare("UPDATE admins SET totp_enabled = 1, updated_at = ? WHERE id = ?")
        .bind(&[now.into(), admin.id.into()])
        .map_err(|e| AppError::Internal(format!("开启 2FA 失败: {}", e)))?
        .run()
        .await?;

    Ok(Json(serde_json::json!({ "message": "2FA 已开启" })))
}

/// POST /api/auth/2fa/disable — 关闭 2FA
#[worker::send]
pub async fn disable_2fa(
    claims: jwt::Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<Disable2faRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let admin = find_admin_by_id(&env, &claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    // 需要验证密码
    let valid = hash::verify_password(&body.password, &admin.password_hash)?;
    if !valid {
        return Err(AppError::WrongCredentials);
    }

    if !admin.totp_enabled {
        return Err(AppError::BadRequest("2FA 未开启".into()));
    }

    let now = time::now_str();
    let db = get_db(&env, Db::Auth)?;
    db.prepare("UPDATE admins SET totp_enabled = 0, totp_secret = NULL, updated_at = ? WHERE id = ?")
        .bind(&[now.into(), admin.id.into()])
        .map_err(|e| AppError::Internal(format!("关闭 2FA 失败: {}", e)))?
        .run()
        .await?;

    Ok(Json(serde_json::json!({ "message": "2FA 已关闭" })))
}

/// GET /api/auth/me — 获取当前管理员信息
#[worker::send]
pub async fn me(
    claims: jwt::Claims,
    State(env): State<Arc<Env>>,
) -> AppResult<Json<AdminInfo>> {
    let admin = find_admin_by_id(&env, &claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    Ok(Json(AdminInfo::from(admin)))
}

/// POST /api/auth/logout — 登出
#[worker::send]
pub async fn logout(
    claims: jwt::Claims,
    State(env): State<Arc<Env>>,
    Json(body): Json<LogoutRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let secret = get_secret(&env)?;
    let refresh_claims = jwt::verify_refresh_token(&body.refresh_token, &secret)?;

    // 确保 refresh token 属于当前用户
    if refresh_claims.sub != claims.sub {
        return Err(AppError::WrongCredentials);
    }

    // 将 refresh token 加入黑名单
    if let Some(jti) = &refresh_claims.jti {
        let kv = KvCache::new(&env)?;
        jwt::blacklist_refresh_token(&kv, jti).await?;
    }

    Ok(Json(serde_json::json!({ "message": "已登出" })))
}

/// POST /api/auth/refresh — 刷新 access token
#[worker::send]
pub async fn refresh(
    State(env): State<Arc<Env>>,
    Json(body): Json<RefreshRequest>,
) -> AppResult<Json<jwt::TokenPair>> {
    let secret = get_secret(&env)?;
    let refresh_claims = jwt::verify_refresh_token(&body.refresh_token, &secret)?;

    // 检查是否在黑名单中
    if let Some(jti) = &refresh_claims.jti {
        let kv = KvCache::new(&env)?;
        if jwt::is_refresh_token_blacklisted(&kv, jti).await? {
            return Err(AppError::WrongCredentials);
        }
    }

    let admin = find_admin_by_id(&env, &refresh_claims.sub)
        .await?
        .ok_or(AppError::WrongCredentials)?;

    let tokens = jwt::generate_token_pair(&admin.id, &admin.username, &secret)?;
    Ok(Json(tokens))
}
