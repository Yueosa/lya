-- lya-session v2：会话可以各自选模型。
--
-- 老库里没有这一列，而 CREATE TABLE IF NOT EXISTS 对已存在的表整段跳过，
-- 所以只能靠 ALTER 补。版本化迁移记着谁跑过，这条不会重复执行。
ALTER TABLE sessions ADD COLUMN model_id TEXT;
