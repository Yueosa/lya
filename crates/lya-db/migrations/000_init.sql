-- lya v0.1.0 全量 schema。
--
-- 这是唯一的初始化脚本：新库一步建到位，不需要经历任何中间状态。以后要改库，
-- 另加 `001_xxx.sql` 独立迁移，**不要动这个文件** —— 已经建过库的机器跳过它，
-- 改了只会让新旧两条路走到不同的终点。
--
-- 语句写成「不存在才建」纯属稳妥：台账已经保证跑过就不再跑，这里只是让手工执行
-- 这个文件（比如照着它建一个测试库）也不会炸。

PRAGMA foreign_keys = ON;

-- ── 会话与消息树 ──────────────────────────────────────────────

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
    -- 不指定则跟随配置里的默认模型
    model_id            TEXT,
    -- 建会话时锁定的 LLM API 栈，之后不再改
    api_mode            TEXT NOT NULL DEFAULT 'completions',
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    -- RESTRICT：只允许删叶节点，删中间节点会让下游整段失去父亲
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

-- 跨会话的显式笔记。正文直接入库，不拆到外部文件，保证单一真相。
CREATE TABLE IF NOT EXISTS memories (
    -- 自增整数：索引要常驻 prompt，短 id 比 uuid 省 token 且模型更容易引用准。
    -- 这个 id 就是模型看到的编号，不再另算一套展示序号——序号会随写入重排，
    -- 而历史消息里的旧序号不会跟着改，同一个号在一个上下文里能指两条记忆
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

-- 索引按 id 升序列出，但超预算时丢的是最久没更新的，所以这个索引仍有用
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
