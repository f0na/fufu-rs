-- 阶段三：博客模块
-- 数据库：fufu_posts

CREATE TABLE IF NOT EXISTS posts (
    id                      TEXT PRIMARY KEY,          -- uuidv7
    title                   TEXT NOT NULL,
    slug                    TEXT NOT NULL UNIQUE,
    content                 TEXT NOT NULL DEFAULT '',
    excerpt                 TEXT NOT NULL DEFAULT '',
    tags                    TEXT NOT NULL DEFAULT '[]', -- JSON 数组
    status                  TEXT NOT NULL DEFAULT 'draft', -- draft / published / archived
    view_count              INTEGER NOT NULL DEFAULT 0,
    github_discussion_number INTEGER,                   -- GitHub Discussion 编号
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    published_at            TEXT,                       -- ISO 8601
    deleted_at              TEXT
);

CREATE INDEX IF NOT EXISTS idx_posts_slug ON posts(slug);
CREATE INDEX IF NOT EXISTS idx_posts_status ON posts(status);
CREATE INDEX IF NOT EXISTS idx_posts_published_at ON posts(published_at);
