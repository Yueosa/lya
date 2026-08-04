<!--
  会话列表 — MC 多人游戏选服界面风格。

  单击选中，底部按钮操作；DefaultShell 侧栏仍自带列表，这里主要给 Minecraft 外壳用。
-->

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { SessionMeta } from '../api/wire'
import {
  archivedSessions,
  createSession,
  currentId,
  models,
  openSession,
  refreshSessions,
  removeSession,
  rename,
  sessions,
  sessionsLoading,
  setArchived,
} from '../app/useChat'
import { confirm, confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'

const emit = defineEmits<{ opened: [] }>()

const selectedId = ref<string | null>(null)

const allSessions = computed(() =>
  [...sessions.value, ...archivedSessions.value].sort(
    (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
  ),
)

const selected = computed(
  () => allSessions.value.find((session) => session.id === selectedId.value) ?? null,
)

watch(
  [allSessions, currentId],
  () => {
    if (selectedId.value && allSessions.value.some((s) => s.id === selectedId.value)) return
    if (currentId.value && allSessions.value.some((s) => s.id === currentId.value)) {
      selectedId.value = currentId.value
      return
    }
    selectedId.value = allSessions.value[0]?.id ?? null
  },
  { immediate: true },
)

onMounted(refreshSessions)

function select(session: SessionMeta): void {
  selectedId.value = session.id
}

function modelLabel(session: SessionMeta): string {
  if (!session.model_id) return '默认模型'
  return models.value.find((item) => item.id === session.model_id)?.name ?? session.model_id
}

function when(session: SessionMeta): string {
  return new Date(session.updated_at).toLocaleString('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function subtitle(session: SessionMeta): string {
  const stack = session.api_mode === 'responses' ? 'Responses' : 'Completions'
  const bits = [stack, session.work_mode, modelLabel(session), when(session)]
  if (session.status === 'archived') bits.push('已归档')
  return bits.join(' · ')
}

async function enter(): Promise<void> {
  if (!selected.value) return
  await openSession(selected.value.id)
  emit('opened')
}

async function startNew(): Promise<void> {
  await createSession()
  emit('opened')
}

async function renameSelected(): Promise<void> {
  const session = selected.value
  if (!session) return
  const title = await prompt({ title: '重命名会话', initial: session.title })
  if (title === null || !title.trim()) return
  try {
    await rename(session.id, title.trim())
    toast('已重命名', 'success')
  } catch {
    toast('重命名失败', 'error')
  }
}

async function archiveSelected(): Promise<void> {
  const session = selected.value
  if (!session || session.status !== 'active') return
  const ok = await confirm({
    title: '归档这个会话？',
    message: '归档之后它从活跃列表里收起来，仍然可以回看，但不能再发消息。',
  })
  if (!ok) return
  try {
    await setArchived(session.id, true)
    toast('已归档', 'success')
  } catch {
    toast('归档失败', 'error')
  }
}

async function unarchiveSelected(): Promise<void> {
  const session = selected.value
  if (!session || session.status !== 'archived') return
  try {
    await setArchived(session.id, false)
    toast('已取消归档', 'success')
  } catch {
    toast('取消归档失败', 'error')
  }
}

async function deleteSelected(): Promise<void> {
  const session = selected.value
  if (!session) return
  await confirmAsync({
    title: `删除「${session.title || '未命名会话'}」？`,
    message: '连同全部消息一起从库里去掉，不可恢复。只想收起来的话用「归档」。',
    confirmText: '删除',
    danger: true,
    run: () => removeSession(session.id),
  })
}
</script>

<template>
  <div class="sessions">
    <h2 class="sessions__title">选择会话</h2>

    <div class="sessions__panel">
      <p v-if="sessionsLoading && allSessions.length === 0" class="sessions__hint">正在读取…</p>
      <p v-else-if="allSessions.length === 0" class="sessions__hint">
        还没有会话，点下方「新建」开始第一句。
      </p>

      <ul v-else class="sessions__list">
        <li v-for="session in allSessions" :key="session.id">
          <button
            type="button"
            class="sessions__row"
            :class="{ 'sessions__row--on': session.id === selectedId }"
            @click="select(session)"
            @dblclick="enter()"
          >
            <img class="sessions__icon" src="/icon.png" alt="" />
            <span class="sessions__info">
              <span class="sessions__name">{{ session.title || '未命名会话' }}</span>
              <span class="sessions__meta">{{ subtitle(session) }}</span>
            </span>
            <span v-if="session.id === currentId" class="sessions__mark">当前</span>
          </button>
        </li>
      </ul>
    </div>

    <div class="sessions__actions">
      <div class="sessions__actions-row">
        <button class="btn btn--lg sessions__action" :disabled="!selected" @click="enter">
          进入
        </button>
        <button class="btn btn--lg sessions__action" :disabled="!selected" @click="renameSelected">
          重命名
        </button>
        <button class="btn btn--lg sessions__action" @click="startNew">新建</button>
      </div>
      <div class="sessions__actions-row">
        <button
          class="btn btn--lg sessions__action"
          :disabled="!selected || selected.status !== 'active'"
          @click="archiveSelected"
        >
          归档
        </button>
        <button
          class="btn btn--lg sessions__action"
          :disabled="!selected || selected.status !== 'archived'"
          @click="unarchiveSelected"
        >
          取消归档
        </button>
        <button
          class="btn btn--lg sessions__action btn--danger"
          :disabled="!selected"
          @click="deleteSelected"
        >
          删除
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sessions {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 20px 20px;
}

.sessions__title {
  margin: 0;
  text-align: center;
  font-size: var(--text-lg);
  text-shadow: var(--text-shadow);
}

.sessions__panel {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 8px;
  background: color-mix(in srgb, var(--bg-sunken) 82%, transparent);
  border: var(--border-width) solid var(--border);
}

.sessions__hint {
  margin: 24px 12px;
  text-align: center;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.sessions__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.sessions__row {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: var(--border-width) solid transparent;
  background: color-mix(in srgb, var(--surface) 55%, transparent);
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  box-shadow: var(--shadow-card);
}

.sessions__row:hover {
  background: var(--surface-hover);
}

.sessions__row--on {
  border-color: var(--border-strong);
  background: color-mix(in srgb, var(--accent-soft) 45%, var(--surface));
}

.sessions__icon {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  object-fit: cover;
  border: var(--border-width) solid var(--border);
}

.sessions__info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sessions__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-md);
}

.sessions__meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-muted);
  font-size: var(--text-xs);
}

.sessions__mark {
  flex-shrink: 0;
  color: var(--info);
  font-size: var(--text-xs);
}

.sessions__actions {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sessions__actions-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.sessions__action {
  width: 100%;
  min-height: var(--ctl-h-lg);
}

.sessions__action:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
</style>
