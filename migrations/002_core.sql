-- 阶段二：站点设置
-- 数据库：fufu_core
-- 说明：站点信息、页脚、社交链接、公告

-- 1. 站点信息（单行配置）
CREATE TABLE IF NOT EXISTS site_profile (
    id              TEXT PRIMARY KEY,
    site_name       TEXT NOT NULL DEFAULT '',
    subtitle        TEXT NOT NULL DEFAULT '',
    logo_url        TEXT NOT NULL DEFAULT '',
    description     TEXT NOT NULL DEFAULT '',
    keywords        TEXT NOT NULL DEFAULT '',
    icp_beian       TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 2. 页脚配置（单行配置）
CREATE TABLE IF NOT EXISTS site_footer (
    id              TEXT PRIMARY KEY,
    content         TEXT NOT NULL DEFAULT '',
    copyright_text  TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 3. 页脚链接
CREATE TABLE IF NOT EXISTS footer_links (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL DEFAULT '',
    url             TEXT NOT NULL DEFAULT '',
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

-- 4. 社交链接
CREATE TABLE IF NOT EXISTS social_links (
    id              TEXT PRIMARY KEY,
    platform        TEXT NOT NULL DEFAULT '',
    label           TEXT NOT NULL DEFAULT '',
    url             TEXT NOT NULL DEFAULT '',
    icon            TEXT NOT NULL DEFAULT '',
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

-- 5. 公告
CREATE TABLE IF NOT EXISTS announcements (
    id              TEXT PRIMARY KEY,
    content         TEXT NOT NULL DEFAULT '',
    active          INTEGER NOT NULL DEFAULT 0,   -- 0=隐藏, 1=显示（不用 is_ 前缀）
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_footer_links_sort ON footer_links(sort_order);
CREATE INDEX IF NOT EXISTS idx_social_links_sort ON social_links(sort_order);
CREATE INDEX IF NOT EXISTS idx_announcements_active_sort ON announcements(active, sort_order);
