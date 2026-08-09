-- 已有 ~/.lya/lya.db 手动升级用（新装用户不需要跑这个文件）。
--
-- 用法：
--   sqlite3 ~/.lya/lya.db < scripts/upgrade-existing-lya-db.sql
--
-- 跑之前请先备份：
--   cp ~/.lya/lya.db ~/.lya/lya.db.bak-$(date +%Y%m%d)

PRAGMA foreign_keys = ON;

-- ── 词表 + 模型模板 ───────────────────────────────────────────

CREATE TABLE IF NOT EXISTS tokenizers (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    asset_path  TEXT,
    updated_at  TEXT NOT NULL
);

INSERT OR IGNORE INTO tokenizers (id, label, asset_path, updated_at)
VALUES ('deepseek_v4', 'DeepSeek V4', 'deepseek_v4/tokenizer.json', datetime('now'));

CREATE TABLE IF NOT EXISTS model_templates (
    model_id             TEXT PRIMARY KEY,
    tokenizer_id         TEXT NOT NULL REFERENCES tokenizers(id),
    context_options_json TEXT NOT NULL DEFAULT '[300000, 1048576]',
    schema_json          TEXT NOT NULL,
    defaults_json        TEXT NOT NULL DEFAULT '{}',
    updated_at           TEXT NOT NULL
);

-- ── sessions：identity / style + 上下文配置 ─────────────────────

-- 老库只有 persona 列时，迁到 identity；style 留空由 prompt.toml 或会话设置补。
-- 下列 ALTER 在列已存在时会报错，可忽略（重复执行安全）。
-- ALTER TABLE sessions ADD COLUMN identity TEXT NOT NULL DEFAULT '';
-- ALTER TABLE sessions ADD COLUMN style TEXT NOT NULL DEFAULT '';
-- ALTER TABLE sessions ADD COLUMN context_config_json TEXT;

-- persona → identity（persona 列仍存在时执行；DROP 后此行无效果）
UPDATE sessions SET identity = persona WHERE length(identity) = 0 AND persona IS NOT NULL AND length(persona) > 0;

UPDATE sessions SET identity = '' WHERE identity IS NULL;
UPDATE sessions SET style = '' WHERE style IS NULL;

-- 废弃列清理（SQLite 3.35+；列已删时会报错，可忽略）
-- ALTER TABLE sessions DROP COLUMN persona;
