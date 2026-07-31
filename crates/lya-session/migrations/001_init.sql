-- lya-session schema（由 lya-db 迁移执行；语句需幂等）

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

CREATE TABLE IF NOT EXISTS session_events (
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq             INTEGER NOT NULL CHECK (seq >= 1),
    envelope_json   TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    PRIMARY KEY(session_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_session_events_replay
    ON session_events(session_id, seq);
