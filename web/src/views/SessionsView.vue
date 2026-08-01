<!--
  会话列表。

  同样只有一份实现——三套外壳决定它摆在哪、周围长什么样，列表本身不重复写。
  排版差异靠 token 就能拉开：MTF 的卡片带偏移阴影、方块世界是直角厚边。
-->

<script setup lang="ts">
import { onMounted } from 'vue'

import type { SessionMeta } from '../api/wire'
import { createSession, currentId, loading, openSession, refreshSessions, rename, sessions } from '../app/useChat'
import { openContextMenu } from '../ui/useContextMenu'
import { confirm, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'

const emit = defineEmits<{ opened: [] }>()

onMounted(refreshSessions)

async function open(id: string): Promise<void> {
  await openSession(id)
  emit('opened')
}

async function start(): Promise<void> {
  await createSession()
  emit('opened')
}

function onContextMenu(event: MouseEvent, session: SessionMeta): void {
  openContextMenu(event, [
    {
      label: '重命名',
      icon: '✎',
      onSelect: async () => {
        const title = await prompt({ title: '重命名会话', initial: session.title })
        if (title === null || !title.trim()) return
        try {
          await rename(session.id, title.trim())
        } catch {
          toast('重命名失败', 'error')
        }
      },
    },
    { separator: true },
    {
      label: '归档',
      icon: '📦',
      danger: true,
      onSelect: async () => {
        const ok = await confirm({
          title: '归档这个会话？',
          message: '归档之后它不再出现在列表里。',
          danger: true,
        })
        // 归档接口还没接上，先老实说清楚，别假装成功
        if (ok) toast('归档还没做', 'info')
      },
    },
  ])
}

function when(session: SessionMeta): string {
  return new Date(session.updated_at).toLocaleString('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}
</script>

<template>
  <div class="sessions">
    <header class="sessions__head">
      <h2 class="sessions__title">会话</h2>
      <button class="btn btn--primary" @click="start">开始新对话</button>
    </header>

    <p v-if="loading && sessions.length === 0" class="sessions__hint">正在读取…</p>
    <p v-else-if="sessions.length === 0" class="sessions__hint">
      还没有会话，点「开始新对话」聊第一句。
    </p>

    <ul class="sessions__list">
      <li v-for="session in sessions" :key="session.id">
        <button
          class="panel sessions__item"
          :class="{ 'sessions__item--on': session.id === currentId }"
          @click="open(session.id)"
          @contextmenu="onContextMenu($event, session)"
        >
          <span class="sessions__name">{{ session.title || '未命名会话' }}</span>
          <span class="sessions__meta">{{ session.work_mode }} · {{ when(session) }}</span>
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.sessions {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.sessions__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.sessions__title {
  margin: 0;
  font-size: var(--text-lg);
}

.sessions__hint {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.sessions__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sessions__item {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  padding: 12px 14px;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: var(--transition);
}

.sessions__item:hover {
  background: var(--surface-hover);
}

.sessions__item--on {
  border-color: var(--accent);
}

.sessions__name {
  font-size: var(--text-md);
}

.sessions__meta {
  color: var(--text-muted);
  font-size: var(--text-xs);
}
</style>
