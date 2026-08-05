<!--
  蔚蓝档案的外壳：仿 Momotalk 的联系人列表。

  和默认侧栏的区别不在配色，在**信息密度**：Momotalk 的列表每一项是一张有头像、
  名字和状态的卡片，占三行高；默认侧栏是一行一个标题的紧凑清单。所以这是另一种
  排版，不是同一套布局换皮。

  它仍然只管导航：内容从插槽来，聊天那套逻辑不会被复制到这里。归档列表也在，
  MC 外壳缺了它那次的教训是——数据在服务端好好的，只是没人给它一个入口。
-->

<script setup lang="ts">
import { computed, ref } from 'vue'

import type { SessionMeta } from '../api/wire'
import {
  archivedSessions,
  createSession,
  currentId,
  openSession,
  removeSession,
  rename,
  sessions,
  setArchived,
} from '../app/useChat'
import BaLogo from '../ui/BaLogo.vue'
import Icon from '../ui/Icon.vue'
import { openContextMenu } from '../ui/useContextMenu'
import { confirm, prompt } from '../ui/useDialog'
import { fmtBubbleTime } from '../utils/dateFormat'
import { readLocal, writeLocal } from '../utils/storage'
import { NAV_ICONS } from './icons'
import { NAV_ITEMS, type ShellProps, type View } from './types'

const props = defineProps<ShellProps>()
const emit = defineEmits<{ navigate: [view: View] }>()

const ARCHIVED_KEY = 'lya.ba.archived'
const showArchived = ref(readLocal(ARCHIVED_KEY) === '1')

function toggleArchived(): void {
  showArchived.value = !showArchived.value
  writeLocal(ARCHIVED_KEY, showArchived.value ? '1' : '0')
}

/**
 * 每个会话的「学生卡」。
 *
 * Momotalk 那一行是「名字 + 最后一句话」，但会话列表接口没有最后一条消息，硬要显示
 * 就得为每个会话再拉一次快照。所以这里显示的是**真有的东西**：更新时间与工作模式，
 * 正在看的那个标成「正在对话」。宁可少说一句，也不编一句。
 */
const roster = computed(() =>
  sessions.value.map((session) => ({
    session,
    subtitle:
      session.id === currentId.value && props.view === 'chat'
        ? '正在对话'
        : `${fmtBubbleTime(session.updated_at)} · ${session.work_mode}`,
  })),
)

function initial(session: SessionMeta): string {
  const title = session.title?.trim()
  return title ? Array.from(title)[0]! : '?'
}

async function open(id: string): Promise<void> {
  await openSession(id)
  emit('navigate', 'chat')
}

async function start(): Promise<void> {
  await createSession()
  emit('navigate', 'chat')
}

function sessionMenu(event: MouseEvent, session: SessionMeta): void {
  const archived = session.status === 'archived'
  openContextMenu(event, [
    {
      label: '改名',
      icon: 'edit',
      onSelect: async () => {
        const next = await prompt({ title: '给对话改个名', initial: session.title })
        if (next !== null) await rename(session.id, next)
      },
    },
    {
      label: archived ? '取消归档' : '归档',
      icon: archived ? 'unarchive' : 'archive',
      onSelect: () => void setArchived(session.id, !archived),
    },
    { separator: true },
    {
      label: '删除',
      icon: 'delete',
      danger: true,
      onSelect: async () => {
        const ok = await confirm({
          title: '删除这个对话？',
          message: '删掉之后没法恢复。',
          danger: true,
        })
        if (ok) await removeSession(session.id)
      },
    },
  ])
}
</script>

<template>
  <div class="ba-shell">
    <aside class="ba-shell__side">
      <!--
        字标就是回首页的按钮，和默认外壳一致。`NAV_ITEMS` 里没有 `home`（默认外壳挂在
        字标上、MC 外壳自己就是首页），所以每套外壳都得自己给一个入口——这一条第一版
        漏了，结果是一离开首页就再也回不去。
      -->
      <header class="ba-shell__brand">
        <button
          class="ba-shell__brand-btn"
          :class="{ 'ba-shell__brand-btn--on': view === 'home' }"
          type="button"
          v-tip="'回首页'"
          @click="emit('navigate', 'home')"
        >
          <BaLogo class="ba-shell__logo" left="lya" right="Archive" />
        </button>
      </header>

      <nav class="ba-shell__nav">
        <button
          v-for="item in NAV_ITEMS"
          :key="item.view"
          class="ba-shell__nav-item"
          :class="{ 'ba-shell__nav-item--on': view === item.view }"
          type="button"
          @click="emit('navigate', item.view)"
        >
          <span class="ba-shell__nav-icon" v-html="NAV_ICONS[item.icon]" />
          <span>{{ item.label }}</span>
        </button>
      </nav>

      <div class="ba-shell__roster-head">
        <span class="ba-shell__roster-title">对话</span>
        <button class="ba-shell__new" type="button" v-tip="'新对话'" @click="start">
          <Icon name="plus" size="sm" />
        </button>
      </div>

      <div class="ba-shell__roster">
        <button
          v-for="row in roster"
          :key="row.session.id"
          class="ba-shell__card"
          :class="{ 'ba-shell__card--on': row.session.id === currentId && view === 'chat' }"
          type="button"
          @click="open(row.session.id)"
          @contextmenu.prevent="sessionMenu($event, row.session)"
        >
          <span class="ba-shell__portrait" aria-hidden="true">{{ initial(row.session) }}</span>
          <span class="ba-shell__card-text">
            <span class="ba-shell__card-name">{{ row.session.title || '未命名' }}</span>
            <span class="ba-shell__card-sub">{{ row.subtitle }}</span>
          </span>
        </button>
        <p v-if="sessions.length === 0" class="ba-shell__empty">还没有对话</p>
      </div>

      <div class="ba-shell__archive">
        <button class="ba-shell__archive-head" type="button" :aria-expanded="showArchived" @click="toggleArchived">
          <span class="ba-shell__nav-icon" v-html="NAV_ICONS.archive" />
          <span>已归档</span>
          <span v-if="archivedSessions.length" class="ba-shell__badge">
            {{ archivedSessions.length }}
          </span>
          <Icon
            class="ba-shell__chevron"
            :class="{ 'ba-shell__chevron--open': showArchived }"
            name="chevronRight"
            size="sm"
          />
        </button>
        <div v-if="showArchived" class="ba-shell__archive-list">
          <button
            v-for="session in archivedSessions"
            :key="session.id"
            class="ba-shell__card ba-shell__card--archived"
            :class="{ 'ba-shell__card--on': session.id === currentId && view === 'chat' }"
            type="button"
            @click="open(session.id)"
            @contextmenu.prevent="sessionMenu($event, session)"
          >
            <span class="ba-shell__portrait" aria-hidden="true">{{ initial(session) }}</span>
            <span class="ba-shell__card-text">
              <span class="ba-shell__card-name">{{ session.title || '未命名' }}</span>
              <span class="ba-shell__card-sub">已归档</span>
            </span>
          </button>
          <p v-if="archivedSessions.length === 0" class="ba-shell__empty">没有归档</p>
        </div>
      </div>
    </aside>

    <main class="ba-shell__main">
      <slot />
    </main>
  </div>
</template>

<style scoped>
.ba-shell {
  display: flex;
  height: 100vh;
  overflow: hidden;
  background: var(--bg);
}

.ba-shell__side {
  width: var(--sidebar-width);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--surface);
  border-right: var(--border-width) solid var(--border);
}

.ba-shell__brand {
  padding: 12px 12px 10px;
}

.ba-shell__brand-btn {
  display: block;
  width: 100%;
  padding: 6px 8px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: background var(--transition);
}

.ba-shell__brand-btn:hover {
  background: var(--surface-hover);
}

.ba-shell__brand-btn--on {
  background: var(--accent-soft);
}

.ba-shell__logo {
  font-size: 26px;
}

.ba-shell__nav {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
  padding: 0 12px 12px;
}

.ba-shell__nav-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  height: var(--ctl-h-md);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
  transition: background var(--transition);
}

.ba-shell__nav-item:hover {
  background: var(--surface-hover);
}

.ba-shell__nav-item--on {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}

.ba-shell__nav-icon {
  display: inline-flex;
  width: 15px;
  height: 15px;
}

.ba-shell__nav-icon :deep(svg) {
  width: 100%;
  height: 100%;
}

.ba-shell__roster-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px 6px;
  border-top: var(--border-width) solid var(--border);
}

.ba-shell__roster-title {
  flex: 1;
  font-size: var(--text-xs);
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--text-faint);
}

.ba-shell__new {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--ctl-h-sm);
  height: var(--ctl-h-sm);
  border: none;
  border-radius: var(--radius-pill);
  background: var(--accent);
  color: var(--on-accent);
  cursor: pointer;
}

.ba-shell__roster {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 8px 8px;
}

/* 学生卡：头像 + 名字 + 状态，三行高，这是和默认侧栏最直观的差别 */
.ba-shell__card {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px;
  margin-bottom: 4px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: background var(--transition);
}

.ba-shell__card:hover {
  background: var(--surface-hover);
}

.ba-shell__card--on {
  background: var(--accent-soft);
}

.ba-shell__card--archived {
  opacity: 0.72;
}

.ba-shell__portrait {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border-radius: var(--radius-md);
  background: var(--bg-sunken);
  color: var(--accent);
  font-size: var(--text-md);
  font-weight: 700;
}

.ba-shell__card-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.ba-shell__card-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-sm);
  font-weight: 600;
}

.ba-shell__card-sub {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.ba-shell__empty {
  margin: 8px;
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.ba-shell__archive {
  border-top: var(--border-width) solid var(--border);
}

.ba-shell__archive-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 0 16px;
  height: var(--ctl-h-lg);
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}

.ba-shell__badge {
  padding: 0 6px;
  border-radius: var(--radius-pill);
  background: var(--bg-sunken);
  font-size: var(--text-xs);
}

.ba-shell__chevron {
  margin-left: auto;
  transition: transform var(--transition);
}

.ba-shell__chevron--open {
  transform: rotate(90deg);
}

.ba-shell__archive-list {
  max-height: 30vh;
  overflow-y: auto;
  padding: 0 8px 8px;
}

.ba-shell__main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

@media (max-width: 720px) {
  .ba-shell {
    flex-direction: column;
  }

  .ba-shell__side {
    width: 100%;
    max-height: 42vh;
    border-right: none;
    border-bottom: var(--border-width) solid var(--border);
  }
}
</style>
