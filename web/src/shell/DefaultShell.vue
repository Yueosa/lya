<!--
  默认外壳：左侧栏 + 内容区（lianclaw 布局）。
-->

<script setup lang="ts">
import { computed, ref, watch } from 'vue'

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
import { setSidebarCollapsed, sidebarCollapsed } from '../app/useShell'
import { readLocal, writeLocal } from '../utils/storage'
import { openContextMenu } from '../ui/useContextMenu'
import { confirm, confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'
import Icon from '../ui/Icon.vue'
import { NAV_ITEMS, type ShellProps, type View } from './types'
import { NAV_ICONS } from './icons'

const props = defineProps<ShellProps>()
const emit = defineEmits<{ navigate: [view: View] }>()

const ARCHIVED_KEY = 'lya.sidebar.archived'

const collapsed = sidebarCollapsed
const showArchived = ref(readLocal(ARCHIVED_KEY) === '1')

const archiveCount = computed(() => archivedSessions.value.length)
const activeCount = computed(() => sessions.value.length)
const viewingArchived = computed(() =>
  archivedSessions.value.some((session) => session.id === currentId.value),
)
const viewingActive = computed(
  () => currentId.value !== null && !viewingArchived.value,
)

watch(showArchived, (open) => {
  writeLocal(ARCHIVED_KEY, open ? '1' : '0')
})

watch(
  viewingArchived,
  (on) => {
    if (on) showArchived.value = true
  },
  { immediate: true },
)

/** 首页聚焦大 logo，进入时自动收起侧栏（含首次访问）。 */
watch(
  () => props.view,
  (v) => {
    if (v === 'home') setSidebarCollapsed(true)
  },
  { immediate: true },
)

function toggleCollapse(): void {
  setSidebarCollapsed(!collapsed.value)
}

async function start(): Promise<void> {
  await createSession()
  emit('navigate', 'chat')
}

async function open(id: string): Promise<void> {
  await openSession(id)
  emit('navigate', 'chat')
}

function sessionMenu(event: MouseEvent, session: SessionMeta): void {
  const archived = session.status === 'archived'
  openContextMenu(event, [
    {
      label: '重命名',
      icon: 'edit',
      onSelect: async () => {
        const title = await prompt({ title: '重命名', initial: session.title })
        if (title === null || !title.trim()) return
        try {
          await rename(session.id, title.trim())
        } catch {
          toast('重命名失败', 'error')
        }
      },
    },
    archived
      ? {
          label: '取消归档',
          icon: 'unarchive',
          onSelect: async () => {
            try {
              await setArchived(session.id, false)
              toast('已恢复', 'success')
            } catch {
              toast('操作失败', 'error')
            }
          },
        }
      : {
          label: '归档',
          icon: 'archive',
          onSelect: async () => {
            const ok = await confirm({ title: '归档此会话？', message: '归档后只读，可随时恢复。' })
            if (!ok) return
            try {
              await setArchived(session.id, true)
              toast('已归档', 'success')
            } catch {
              toast('归档失败', 'error')
            }
          },
        },
    { separator: true },
    {
      label: '删除',
      icon: 'delete',
      danger: true,
      onSelect: async () => {
        await confirmAsync({
          title: `删除「${session.title || '未命名'}」？`,
          message: '不可恢复。',
          confirmText: '删除',
          danger: true,
          run: () => removeSession(session.id),
        })
      },
    },
  ])
}
</script>

<template>
  <div class="shell" :class="{ 'shell--collapsed': collapsed }">
    <aside class="shell__side">
      <header class="shell__head">
        <button class="shell__toggle" v-tip="'收起侧栏'" @click="toggleCollapse">
          <Icon name="menu" size="sm" />
        </button>
        <button
          class="shell__brand"
          :class="{ 'shell__brand--on': view === 'home' }"
          v-tip="'首页'"
          @click="emit('navigate', 'home')"
        >
          lya
        </button>
        <button class="shell__new btn-icon" v-tip="'新对话'" @click="start">
          <Icon name="plus" size="sm" />
        </button>
      </header>

      <nav class="shell__nav">
        <button
          v-for="item in NAV_ITEMS"
          :key="item.view"
          class="shell__nav-item"
          :class="{ 'shell__nav-item--on': view === item.view }"
          @click="$emit('navigate', item.view)"
        >
          <span class="shell__nav-icon" v-html="NAV_ICONS[item.icon]" />
          <span>{{ item.label }}</span>
        </button>
      </nav>

      <div class="shell__divider" />

      <div class="shell__sessions">
        <div
          class="shell__sessions-head"
          :class="{ 'shell__sessions-head--active': view === 'chat' && viewingActive }"
        >
          <span class="shell__section-icon" v-html="NAV_ICONS.chat" />
          <span class="shell__section-label">活跃对话</span>
          <span v-if="activeCount" class="shell__section-badge">{{ activeCount }}</span>
        </div>
        <div class="shell__list">
          <button
            v-for="session in sessions"
            :key="session.id"
            class="shell__item"
            :class="{ 'shell__item--on': session.id === currentId && view === 'chat' }"
            :title="session.title || '未命名'"
            @click="open(session.id)"
            @contextmenu.prevent="sessionMenu($event, session)"
          >
            {{ session.title || '未命名' }}
          </button>
          <p v-if="sessions.length === 0" class="shell__empty">暂无会话</p>
        </div>
      </div>

      <div
        class="shell__archive-dock"
        :class="{
          'shell__archive-dock--open': showArchived,
          'shell__archive-dock--active': viewingArchived,
          'shell__archive-dock--has': archiveCount > 0,
        }"
      >
        <button
          class="shell__archive-toggle"
          type="button"
          :aria-expanded="showArchived"
          @click="showArchived = !showArchived"
        >
          <span class="shell__section-icon" v-html="NAV_ICONS.archive" />
          <span class="shell__section-label">归档对话</span>
          <span v-if="archiveCount" class="shell__section-badge">{{ archiveCount }}</span>
          <span class="shell__chevron" :class="{ 'shell__chevron--open': showArchived }">
            <Icon name="chevronRight" size="sm" />
          </span>
        </button>
        <div v-if="showArchived" class="shell__archive-list">
          <button
            v-for="session in archivedSessions"
            :key="session.id"
            class="shell__item shell__item--archived"
            :class="{ 'shell__item--on': session.id === currentId && view === 'chat' }"
            :title="session.title || '未命名'"
            @click="open(session.id)"
            @contextmenu.prevent="sessionMenu($event, session)"
          >
            {{ session.title || '未命名' }}
          </button>
          <p v-if="archivedSessions.length === 0" class="shell__empty">暂无归档</p>
        </div>
      </div>
    </aside>

    <main class="shell__main">
      <slot />
    </main>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.shell__side {
  width: var(--sidebar-width);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-sunken);
  border-right: 2px solid var(--accent);
  transition:
    margin-left 0.25s ease,
    opacity 0.25s ease;
}

.shell--collapsed .shell__side {
  margin-left: calc(-1 * var(--sidebar-width));
  opacity: 0;
  pointer-events: none;
}

.shell__head {
  display: grid;
  grid-template-columns: 32px 1fr 32px;
  align-items: center;
  padding: 14px 16px;
  flex-shrink: 0;
}

.shell__toggle {
  grid-column: 1;
  justify-self: center;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border: none;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-sm);
  transition: var(--transition);
}

.shell__toggle:hover {
  background: var(--surface-hover);
}

.btn-icon {
  grid-column: 3;
  justify-self: end;
  width: 32px;
  height: 32px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: var(--transition);
  flex-shrink: 0;
}

.btn-icon:hover {
  background: var(--surface-hover);
  transform: translateY(-1px);
}

.shell__nav-icon {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.shell__nav-icon :deep(svg) {
  width: 16px;
  height: 16px;
  display: block;
}

.shell__brand {
  grid-column: 2;
  justify-self: center;
  margin: 0;
  padding: 2px 8px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  font: inherit;
  font-size: 26px;
  font-weight: 800;
  letter-spacing: 0.04em;
  color: var(--accent);
  cursor: pointer;
  transition: background var(--transition), opacity var(--transition);
}

.shell__brand:hover {
  background: var(--surface-hover);
}

.shell__brand--on {
  opacity: 1;
}

.shell__nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px;
  flex-shrink: 0;
}

.shell__nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 12px;
  border: none;
  border-left: var(--border-accent-width) solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
  transition: var(--transition);
}

.shell__nav-item:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.shell__nav-item--on {
  background: var(--surface-active);
  color: var(--accent);
  font-weight: 600;
  border-left-color: var(--accent);
}

.shell__item {
  display: block;
  width: 100%;
  padding: 8px 12px;
  margin-bottom: 2px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell__item--on {
  background: color-mix(in srgb, var(--info) 15%, var(--surface));
  font-weight: 600;
}

.shell__divider {
  height: 2px;
  margin: 0 12px;
  background: var(--accent);
  opacity: 0.5;
  flex-shrink: 0;
}

.shell__section-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  color: var(--accent);
  display: flex;
  align-items: center;
  justify-content: center;
}

.shell__section-icon :deep(svg) {
  width: 16px;
  height: 16px;
  display: block;
}

.shell__section-label {
  flex: 1;
  min-width: 0;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text);
}

.shell__section-badge {
  min-width: 20px;
  padding: 1px 7px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--info) 18%, var(--surface));
  color: var(--info);
  font-size: var(--text-xs);
  font-weight: 700;
  line-height: 1.35;
  text-align: center;
}

.shell__sessions {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: color-mix(in srgb, var(--surface) 35%, var(--bg-sunken));
}

.shell__sessions-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding: 11px 14px 9px;
  border-bottom: 2px solid color-mix(in srgb, var(--info) 28%, transparent);
}

.shell__sessions-head--active {
  border-bottom-color: color-mix(in srgb, var(--info) 55%, transparent);
  background: color-mix(in srgb, var(--info) 10%, var(--bg-sunken));
}

.shell__sessions-head--active .shell__section-badge {
  background: color-mix(in srgb, var(--info) 24%, var(--surface));
}

.shell__chevron {
  display: inline-block;
  flex-shrink: 0;
  color: var(--text-faint);
  transition: transform 0.15s ease;
}

.shell__chevron--open {
  transform: rotate(90deg);
}

.shell__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 10px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.shell__archive-dock {
  flex-shrink: 0;
  border-top: 2px solid color-mix(in srgb, var(--accent) 28%, transparent);
  background: color-mix(in srgb, var(--surface) 55%, var(--bg-sunken));
}

.shell__archive-dock--has {
  border-top-color: color-mix(in srgb, var(--accent) 45%, transparent);
}

.shell__archive-dock--active {
  border-top-color: var(--accent);
  background: color-mix(in srgb, var(--accent-soft) 45%, var(--bg-sunken));
}

.shell__archive-dock--open.shell__archive-dock--has {
  box-shadow: 0 -4px 14px color-mix(in srgb, var(--accent) 8%, transparent);
}

.shell__archive-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 11px 14px;
  border: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
  text-align: left;
  transition: background var(--transition), color var(--transition);
}

.shell__archive-toggle:hover {
  background: var(--surface-hover);
}

.shell__archive-dock--active .shell__archive-toggle {
  font-weight: 600;
}

.shell__archive-dock .shell__section-badge {
  background: color-mix(in srgb, var(--accent) 22%, var(--surface));
  color: var(--accent);
}

.shell__archive-dock--has .shell__section-badge {
  background: color-mix(in srgb, var(--accent) 28%, var(--surface));
}

.shell__archive-list {
  max-height: min(38vh, 220px);
  overflow-y: auto;
  padding: 0 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.shell__item--archived {
  opacity: 0.88;
  color: var(--text-muted);
}

.shell__item--archived.shell__item--on {
  opacity: 1;
  color: var(--text);
}

.shell__empty {
  margin: 0;
  padding: 6px 14px;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.shell__main {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  position: relative;
}
</style>
