-- 阶段十：站点配置键值存储
-- 数据库：fufu_core
-- 说明：存储站点级别的键值配置，替代 KV 存储的部分功能

CREATE TABLE IF NOT EXISTS site_config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
