-- 链接收藏表（fufu_social）
-- 标签以 JSON 文本数组格式存储

CREATE TABLE IF NOT EXISTS links (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    url             TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    favicon_url     TEXT NOT NULL DEFAULT '',
    tags            TEXT NOT NULL DEFAULT '[]',       -- JSON 数组，如 ["rust","web"]
    favorite        INTEGER NOT NULL DEFAULT 0,       -- 0/1 收藏标记
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_links_favorite ON links(favorite);
CREATE INDEX IF NOT EXISTS idx_links_sort ON links(sort_order);
