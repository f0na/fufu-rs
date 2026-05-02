-- 阶段四：点赞系统
-- 数据库：fufu_likes

CREATE TABLE IF NOT EXISTS like_counts (
    id              TEXT PRIMARY KEY,
    target_type     TEXT NOT NULL,   -- 目标类型：post, comment 等
    target_id       TEXT NOT NULL,   -- 目标 ID：如文章 slug
    count           INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(target_type, target_id)
);

CREATE INDEX IF NOT EXISTS idx_like_counts_target ON like_counts(target_type, target_id);
