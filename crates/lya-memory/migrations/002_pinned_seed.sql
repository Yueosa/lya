-- 索引 #1 固定留给一条置顶记忆；模型侧编号与 DB 自增 id 解耦。

ALTER TABLE memories ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1));

CREATE UNIQUE INDEX idx_memories_single_pin ON memories(pinned) WHERE pinned = 1;

INSERT INTO memories (title, summary, body, pinned, created_at, updated_at)
SELECT
    '致小恋恋: 想对你说的话',
    '留给小恋恋的心里话，正文待填写。',
    '',
    1,
    '2026-01-01T00:00:00+00:00',
    '2026-01-01T00:00:00+00:00'
WHERE NOT EXISTS (SELECT 1 FROM memories WHERE pinned = 1);
