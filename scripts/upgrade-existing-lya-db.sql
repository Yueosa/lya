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

-- ── sessions：人设 + 上下文配置 ─────────────────────────────────

-- persona 曾为 NULL（表示跟全局走）——一次性补成当前默认人设正文。
-- 下面占位符请改成你 persona.toml 里的 text，或跑完用 Session 面板逐条改。
UPDATE sessions
SET persona = ''
WHERE persona IS NULL;

-- SQLite 无法直接给已有列加 NOT NULL；新库在 000_init 里已是 NOT NULL DEFAULT ''。
-- 老库靠上面 UPDATE 保证不再出现 NULL 即可。

-- 上下文配置列（不存在才加——重复执行安全）
ALTER TABLE sessions ADD COLUMN context_config_json TEXT;
