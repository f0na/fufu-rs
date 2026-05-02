-- 番剧记录表（fufu_bangumi）

CREATE TABLE IF NOT EXISTS bangumi_records (
    id              TEXT PRIMARY KEY,
    subject_id      INTEGER NOT NULL,
    title           TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'want_to_watch',  -- watching / want_to_watch / watched / dropped
    progress        TEXT NOT NULL DEFAULT '',
    cover_url       TEXT NOT NULL DEFAULT '',
    fansub          TEXT NOT NULL DEFAULT '',
    added_at        TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_bangumi_status ON bangumi_records(status);
CREATE INDEX IF NOT EXISTS idx_bangumi_subject ON bangumi_records(subject_id);
