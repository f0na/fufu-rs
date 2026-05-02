-- 相册表 & 照片表（fufu_media）
-- tags 以 JSON 文本数组格式存储
-- photos 无 updated_at（不可编辑，只能添加/删除）

CREATE TABLE IF NOT EXISTS galleries (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    cover_path      TEXT NOT NULL DEFAULT '',
    tags            TEXT NOT NULL DEFAULT '[]',       -- JSON 数组，如 ["旅行","美食"]
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE TABLE IF NOT EXISTS photos (
    id              TEXT PRIMARY KEY,
    gallery_id      TEXT NOT NULL,                    -- FK → galleries.id
    path            TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_photos_gallery ON photos(gallery_id);
