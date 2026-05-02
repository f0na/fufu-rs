-- 阶段一：身份验证系统
-- 数据库：fufu_auth
-- 说明：管理员表、登录日志、验证码

-- 1. 管理员表
CREATE TABLE IF NOT EXISTS admins (
    id              TEXT PRIMARY KEY,                          -- uuidv7
    username        TEXT NOT NULL UNIQUE,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    totp_secret     TEXT,                                      -- Base32 编码的 TOTP 密钥
    totp_enabled    INTEGER NOT NULL DEFAULT 0,                -- 0=未启用, 1=已启用
    role            TEXT NOT NULL DEFAULT 'admin',
    created_at      TEXT NOT NULL,                             -- ISO 8601
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT                                        -- 逻辑删除
);

-- 2. 登录日志表
-- admin_id 对应 admins.id（应用层关联，无外键约束）
CREATE TABLE IF NOT EXISTS login_logs (
    id              TEXT PRIMARY KEY,                          -- uuidv7
    admin_id        TEXT NOT NULL,                             -- 对应 admins.id
    ip              TEXT,
    user_agent      TEXT,
    status          TEXT NOT NULL,                             -- success / failed / totp_required
    created_at      TEXT NOT NULL
);

-- 3. 验证码表
-- admin_id 对应 admins.id（应用层关联，无外键约束）
CREATE TABLE IF NOT EXISTS verification_codes (
    id              TEXT PRIMARY KEY,                          -- uuidv7
    admin_id        TEXT NOT NULL,                             -- 对应 admins.id
    code            TEXT NOT NULL,
    type            TEXT NOT NULL,                             -- email_verification / password_reset
    expires_at      TEXT NOT NULL,                             -- ISO 8601 过期时间
    used_at         TEXT,                                      -- ISO 8601 使用时间
    created_at      TEXT NOT NULL
);

-- 索引（按 admin_id 查询优化）
CREATE INDEX IF NOT EXISTS idx_login_logs_admin_id ON login_logs(admin_id);
CREATE INDEX IF NOT EXISTS idx_verification_codes_admin_id ON verification_codes(admin_id);
CREATE INDEX IF NOT EXISTS idx_verification_codes_code_type ON verification_codes(code, type);
