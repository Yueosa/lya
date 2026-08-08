<!--
  会话列表 — MC 多人游戏选服界面风格。

  单击选中，底部按钮操作；DefaultShell 侧栏仍自带列表，这里主要给 Minecraft 外壳用。
-->

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { SessionMeta } from '../api/wire'
import {
  archiveSession,
  deleteSession,
  renameSession,
  unarchiveSession,
} from '../app/sessionActions'
import {
  archivedSessions,
  createSession,
  currentId,
  models,
  openSession,
  refreshSessions,
  sessions,
  sessionsLoading,
} from '../app/useChat'
import { useArchiveDock } from '../shell/useArchiveDock'

const emit = defineEmits<{ opened: [] }>()

const selectedId = ref<string | null>(null)

/**
 * 归档单独列，不混进主列表。
 *
 * 原先两批按时间揉成一列，归档的唯一标记是副标题末尾多「已归档」三个字——夹在
 * 「Responses · chat · 模型名 · 时间」后面，扫一眼根本分不出来。现在主列表只放在用的，
 * 归档收进底部抽屉，和默认外壳侧栏是同一套做法。
 */
const byRecent = (a: SessionMeta, b: SessionMeta): number =>
  new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()

const activeSessions = computed(() => [...sessions.value].sort(byRecent))
const archive = useArchiveDock()

/** 选中项可以来自任意一批，所以查找和空态判断都看合起来的这份。 */
const allSessions = computed(() => [...activeSessions.value, ...archivedSessions.value])

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
  // 「已归档」不再写进这串：归档现在自己一栏，位置就说明了状态
  return [stack, session.work_mode, modelLabel(session), when(session)].join(' · ')
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

/*
  底部那排按钮作用在选中项上。

  每个动作本身（问什么、成功说什么、失败说什么）在 `app/sessionActions.ts`，和右键菜单
  共用一份——这四个动作原先两边各写一遍，措辞已经走偏了八处。这里只负责「作用在哪一条」。

  状态那道判断留着：按钮是 disabled 的，但键盘和辅助技术不一定认那个属性。
*/
function onSelected(
  act: (session: SessionMeta) => Promise<void>,
  when?: SessionMeta['status'],
): void {
  const session = selected.value
  if (!session || (when && session.status !== when)) return
  void act(session)
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
      <p v-else-if="activeSessions.length === 0" class="sessions__hint">
        在用的对话都归档了，展开下面那栏可以找回来。
      </p>

      <ul v-else class="sessions__list">
        <li v-for="session in activeSessions" :key="session.id">
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

      <!-- 归档钉在列表底部，收起来只占一行 -->
      <div
        v-if="archive.count.value"
        class="sessions__archive"
        :class="{ 'sessions__archive--open': archive.open.value }"
      >
        <button
          class="sessions__archive-head"
          type="button"
          :aria-expanded="archive.open.value"
          @click="archive.open.value = !archive.open.value"
        >
          <span class="sessions__archive-label">归档对话</span>
          <span class="sessions__archive-count">{{ archive.count.value }}</span>
          <span class="sessions__archive-chevron">›</span>
        </button>

        <ul v-if="archive.open.value" class="sessions__list sessions__list--archived">
          <li v-for="session in archive.items.value" :key="session.id">
            <button
              type="button"
              class="sessions__row sessions__row--archived"
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
    </div>

    <div class="sessions__actions">
      <div class="sessions__actions-row">
        <button class="btn btn--lg sessions__action" :disabled="!selected" @click="enter">
          进入
        </button>
        <button
          class="btn btn--lg sessions__action"
          :disabled="!selected"
          @click="onSelected(renameSession)"
        >
          重命名
        </button>
        <button class="btn btn--lg sessions__action" @click="startNew">新建</button>
      </div>
      <div class="sessions__actions-row">
        <button
          class="btn btn--lg sessions__action"
          :disabled="!selected || selected.status !== 'active'"
          @click="onSelected(archiveSession, 'active')"
        >
          归档
        </button>
        <button
          class="btn btn--lg sessions__action"
          :disabled="!selected || selected.status !== 'archived'"
          @click="onSelected(unarchiveSession, 'archived')"
        >
          取消归档
        </button>
        <button
          class="btn btn--lg sessions__action btn--danger"
          :disabled="!selected"
          @click="onSelected(deleteSession)"
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

/* ── 归档抽屉 ─────────────────────────────────── */

.sessions__archive {
  flex-shrink: 0;
  border-top: var(--border-width) solid var(--border);
}

.sessions__archive-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 10px 12px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-xs);
  font-weight: 700;
  text-align: left;
  cursor: pointer;
}

.sessions__archive-head:hover {
  color: var(--accent);
}

.sessions__archive-label {
  flex: 1;
}

.sessions__archive-count {
  padding: 1px 8px;
  border-radius: var(--radius-pill);
  background: var(--surface-active);
  font-weight: 700;
}

.sessions__archive-chevron {
  display: inline-block;
  font-size: 15px;
  line-height: 1;
  transition: transform var(--transition);
}

.sessions__archive--open .sessions__archive-chevron {
  transform: rotate(90deg);
}

/* 归档项压暗一档：点得开，但一眼看得出不是在用的那批 */
.sessions__row--archived .sessions__name {
  color: var(--text-muted);
}

.sessions__row--archived .sessions__icon {
  opacity: 0.62;
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

/* 圆形，和聊天气泡旁的头像同一个规矩——它们是同一个「人」 */
.sessions__icon {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  border-radius: 50%;
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
