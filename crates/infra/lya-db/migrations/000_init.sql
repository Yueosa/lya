-- lya 全量 schema（唯一初始化脚本）。
--
-- 新用户：打开软件 → migrate 一次 → 库结构完整，没有 001/002 要追。
-- 已有 ~/.lya/lya.db 的老用户：不要指望自动 ALTER，请手动跑仓库里的
--   scripts/upgrade-existing-lya-db.sql（或按其中注释逐条执行）。
--
-- 以后要改表结构：**直接改本文件**，并同步更新 upgrade-existing-lya-db.sql；
-- 不要往 SCHEMA 里追加 version 1、2… 的增量迁移。
--
-- 语句写成「不存在才建」纯属稳妥：台账保证跑过就不再跑，手工照着建测试库也不会炸。

PRAGMA foreign_keys = ON;

-- ── 词表注册（lya-token）──────────────────────────────────────

CREATE TABLE IF NOT EXISTS tokenizers (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    -- bundled 资源路径（相对 lya-token/assets/）；NULL = 无文件，走启发式
    asset_path  TEXT,
    updated_at  TEXT NOT NULL
);

-- DeepSeek V4：Flash / Pro 共用同一份 HF tokenizer.json
INSERT OR IGNORE INTO tokenizers (id, label, asset_path, updated_at)
VALUES ('deepseek_v4', 'DeepSeek V4', 'deepseek_v4/tokenizer.json', datetime('now'));

-- ── 模型参数模板（上下文管理器 UI + PATCH 校验）────────────────

CREATE TABLE IF NOT EXISTS model_templates (
    model_id             TEXT PRIMARY KEY,
    tokenizer_id         TEXT NOT NULL REFERENCES tokenizers(id),
    -- 可选 context_limit，如 [300000, 1000000]
    context_options_json TEXT NOT NULL DEFAULT '[300000, 1048576]',
    -- 字段定义：type / enum / min / max / api_modes / scope / default
    schema_json          TEXT NOT NULL,
    -- 各 api_mode 默认 params（与 models.toml modes.*.params 对齐，可被会话 override）
    defaults_json        TEXT NOT NULL DEFAULT '{}',
    updated_at           TEXT NOT NULL
);

-- ── 会话与消息树 ──────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS sessions (
    id                  TEXT PRIMARY KEY,
    title               TEXT NOT NULL DEFAULT '',
    status              TEXT NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'archived')),
    active_leaf_id      INTEGER,
    work_mode           TEXT NOT NULL DEFAULT 'agent'
                            CHECK (work_mode IN ('ask', 'edit', 'agent')),
    -- 创建时从默认人设抄一份正文进来；之后只改本会话，不跟 persona.toml 联动
    persona             TEXT NOT NULL DEFAULT '',
    -- NULL = 启用全部工具；JSON 数组 = 只启用列出的（空数组即全部禁用）
    enabled_tools_json  TEXT,
    model_id            TEXT,
    api_mode            TEXT NOT NULL DEFAULT 'completions',
    -- 上下文管理：context_limit、auto_compress_pct、params override 等
    context_config_json TEXT,
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

-- ── 长期记忆 ──────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS memories (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    title               TEXT NOT NULL UNIQUE,
    summary             TEXT NOT NULL DEFAULT '',
    body                TEXT NOT NULL DEFAULT '',
    source_session_id   TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memories_updated
    ON memories(updated_at DESC);

CREATE TABLE IF NOT EXISTS memory_tags (
    memory_id   INTEGER NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    ord         INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (memory_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_memory_tags_tag
    ON memory_tags(tag);
