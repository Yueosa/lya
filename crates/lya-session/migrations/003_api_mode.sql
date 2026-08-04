-- 会话创建时锁定的 LLM API 栈（completions / responses）。

ALTER TABLE sessions ADD COLUMN api_mode TEXT NOT NULL DEFAULT 'completions';
