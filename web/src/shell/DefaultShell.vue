<!--
  默认外壳：左侧栏 + 内容区（lianclaw 布局）。
-->

<script setup lang="ts">
import { computed, watch } from 'vue'

import { createSession, currentId, openSession, sessions } from '../app/useChat'
import { setSidebarCollapsed, sidebarCollapsed } from '../app/useShell'
import Icon from '../ui/Icon.vue'
import ArchiveDock from './ArchiveDock.vue'
import { NAV_ITEMS, type ShellProps, type View } from './types'
import { NAV_ICONS } from './icons'
import { openSessionMenu } from './sessionMenu'

const props = defineProps<ShellProps>()
const emit = defineEmits<{ navigate: [view: View] }>()

const collapsed = sidebarCollapsed

const activeCount = computed(() => sessions.value.length)
const viewingActive = computed(
  () => currentId.value !== null && sessions.value.some((s) => s.id === currentId.value),
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
            @contextmenu.prevent="openSessionMenu($event, session)"
          >
            {{ session.title || '未命名' }}
          </button>
          <p v-if="sessions.length === 0" class="shell__empty">暂无会话</p>
        </div>
      </div>

      <ArchiveDock>
        <template #icon>
          <span class="shell__section-icon" v-html="NAV_ICONS.archive" />
        </template>
        <template #chevron>
          <Icon name="chevronRight" size="sm" />
        </template>
        <template #default="{ items }">
          <button
            v-for="session in items"
            :key="session.id"
            class="shell__item shell__item--archived"
            :class="{ 'shell__item--on': session.id === currentId && view === 'chat' }"
            :title="session.title || '未命名'"
            @click="open(session.id)"
            @contextmenu.prevent="openSessionMenu($event, session)"
          >
            {{ session.title || '未命名' }}
          </button>
        </template>
      </ArchiveDock>
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

.shell__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 10px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

/* 主题钩在共用的 archive-dock 上：默认外壳比 SessionsView / BA 更重一点描边 */
:deep(.archive-dock) {
  border-top-width: 2px;
  border-top-color: color-mix(in srgb, var(--accent) 28%, transparent);
  background: color-mix(in srgb, var(--surface) 55%, var(--bg-sunken));
}

:deep(.archive-dock--has) {
  border-top-color: color-mix(in srgb, var(--accent) 45%, transparent);
}

:deep(.archive-dock--active) {
  border-top-color: var(--accent);
  background: color-mix(in srgb, var(--accent-soft) 45%, var(--bg-sunken));
}

:deep(.archive-dock--open.archive-dock--has) {
  box-shadow: 0 -4px 14px color-mix(in srgb, var(--accent) 8%, transparent);
}

:deep(.archive-dock__head) {
  padding: 11px 14px;
  color: var(--text);
  font-size: var(--text-sm);
  font-weight: 400;
}

:deep(.archive-dock__head:hover) {
  background: var(--surface-hover);
  color: var(--text);
}

:deep(.archive-dock--active .archive-dock__head) {
  font-weight: 600;
}

:deep(.archive-dock__count) {
  background: color-mix(in srgb, var(--accent) 22%, var(--surface));
  color: var(--accent);
}

:deep(.archive-dock--has .archive-dock__count) {
  background: color-mix(in srgb, var(--accent) 28%, var(--surface));
}

:deep(.archive-dock__list) {
  max-height: min(38vh, 220px);
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
