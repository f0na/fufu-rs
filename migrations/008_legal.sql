-- 许可证 & 隐私政策版本表（fufu_legal）
-- 不可变记录：只追加新版本，不更新不删除

CREATE TABLE IF NOT EXISTS license_versions (
    id              TEXT PRIMARY KEY,
    version         TEXT NOT NULL,
    content         TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS privacy_versions (
    id              TEXT PRIMARY KEY,
    version         TEXT NOT NULL,
    date            TEXT NOT NULL,               -- 生效日期
    content         TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
