-- lya-memory schema（由 lya-db 迁移执行；语句需幂等）

PRAGMA foreign_keys = ON;

-- 跨会话的显式笔记。正文直接入库，不拆到外部文件，保证单一真相。
CREATE TABLE IF NOT EXISTS memories (
    -- 自增整数：索引要常驻 prompt，短 id 比 uuid 省 token 且模型更容易引用准
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    -- 唯一：同名即视为同一条记忆，写入天然变成更新
    title               TEXT NOT NULL UNIQUE,
    -- 一句话概括，进常驻索引
    summary             TEXT NOT NULL DEFAULT '',
    -- 正文，按需读取
    body                TEXT NOT NULL DEFAULT '',
    -- 溯源用；会话删除时**不**级联，记忆要比会话活得久
    source_session_id   TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memories_updated
    ON memories(updated_at DESC);

-- 标签拆关联表而不是塞 JSON，便于按标签直接查
CREATE TABLE IF NOT EXISTS memory_tags (
    memory_id   INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    -- 写入顺序。标签是有主次的（具体名词在前、泛类在后），排序展示会丢掉这层信息
    ord         INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (memory_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_memory_tags_tag
    ON memory_tags(tag);
