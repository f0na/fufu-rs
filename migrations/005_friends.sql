-- 阶段五：友人帐
-- 数据库：fufu_social

CREATE TABLE IF NOT EXISTS friends (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    url             TEXT NOT NULL,
    avatar_url      TEXT NOT NULL DEFAULT '',
    description     TEXT NOT NULL DEFAULT '',
    email           TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'pending',  -- pending / approved / rejected
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_friends_status ON friends(status);
CREATE INDEX IF NOT EXISTS idx_friends_sort ON friends(sort_order);
