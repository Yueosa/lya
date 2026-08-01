-- lya-session v1：初始表结构。
--
-- 这一步保持「最初的样子」不再改动——已经建过库的机器跳过它，新机器靠它
-- 起步，两边都要能走到同一个终点。后续的字段变更一律另起一个版本文件。

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sessions (
    id                  TEXT PRIMARY KEY,
    title               TEXT NOT NULL DEFAULT '',
    status              TEXT NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'archived')),
    active_leaf_id      INTEGER,
    work_mode           TEXT NOT NULL DEFAULT 'agent'
                            CHECK (work_mode IN ('ask', 'edit', 'agent')),
    persona             TEXT,
    -- NULL = 启用全部工具；JSON 数组 = 只启用列出的（空数组即全部禁用）。
    -- 与 lya-config 的 tools.enabled 和 ToolRegistry::bundle 的 names 同语义
    enabled_tools_json  TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    parent_id       INTEGER REFERENCES messages(id) ON DELETE RESTRICT,
    sort_key        INTEGER NOT NULL,
    message_json    TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    UNIQUE(session_id, sort_key)
);

CREATE INDEX IF NOT EXISTS idx_messages_session_sort
    ON messages(session_id, sort_key);

CREATE INDEX IF NOT EXISTS idx_messages_parent
    ON messages(parent_id);

CREATE TABLE IF NOT EXISTS branch_meta (
    leaf_msg_id     INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    title           TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL
);

-- 曾经为「SSE 断线续传」预留过一张 session_events 表，后来发现用不上：
-- 订阅时先发一份快照（消息树 + 当前轮的内存缓冲）再推增量，重连和首次连接
-- 走同一条路，天然幂等，不需要序号对齐也不需要事件重放。
