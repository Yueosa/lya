<!--
  东京夜外壳：底栏地铁线导航；会话切换仅在对话页顶栏出现。
-->

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { SessionMeta } from '../api/wire'
import {
  archivedSessions,
  createSession,
  currentId,
  openSession,
  refreshSessions,
  removeSession,
  rename,
  sessions,
  setArchived,
} from '../app/useChat'
import { setSidebarCollapsed } from '../app/useShell'
import Icon from '../ui/Icon.vue'
import { openContextMenu } from '../ui/useContextMenu'
import { confirm, confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'
import { NAV_ICONS } from './icons'
import { NAV_ITEMS, type ShellProps, type View } from './types'

const props = defineProps<ShellProps>()
const emit = defineEmits<{ navigate: [view: View] }>()

const showArchived = ref(false)

const archiveCount = computed(() => archivedSessions.value.length)

const isAdmin = computed(() =>
  ['memory', 'tools', 'models', 'config', 'theme'].includes(props.view),
)

interface MetroStation {
  view: View
  label: string
  icon: keyof typeof NAV_ICONS
}

const STATIONS: MetroStation[] = [
  { view: 'home', label: '首页', icon: 'home' },
  { view: 'chat', label: '对话', icon: 'chat' },
  ...NAV_ITEMS.map((item) => ({ view: item.view, label: item.label, icon: item.icon })),
]

const TRACK_SEGMENTS = STATIONS.length - 1

onMounted(() => {
  void refreshSessions()
  setSidebarCollapsed(true)
})

watch(
  () => props.view,
  () => {
    showArchived.value = false
    setSidebarCollapsed(true)
  },
)

function isActive(view: View): boolean {
  if (view === 'chat') return props.view === 'chat' || props.view === 'settings'
  return props.view === view
}

function go(view: View): void {
  emit('navigate', view)
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
  <div
    class="tokyo-shell"
    :class="{
      'tokyo-shell--chat': view === 'chat' || view === 'settings',
      'tokyo-shell--admin': isAdmin,
      'tokyo-shell--home': view === 'home',
    }"
  >
    <main class="tokyo-shell__stage">
      <header v-if="view === 'chat' || view === 'settings'" class="tokyo-shell__sessions">
        <div class="tokyo-shell__sessions-scroll">
          <button
            v-for="session in sessions"
            :key="session.id"
            class="tokyo-shell__session-tab"
            :class="{ 'tokyo-shell__session-tab--on': session.id === currentId }"
            :title="session.title || '未命名'"
            @click="open(session.id)"
            @contextmenu.prevent="sessionMenu($event, session)"
          >
            {{ session.title || '未命名' }}
          </button>
          <button
            v-if="archiveCount"
            class="tokyo-shell__session-tab tokyo-shell__session-tab--archive"
            :class="{ 'tokyo-shell__session-tab--on': showArchived }"
            @click="showArchived = !showArchived"
          >
            归档 · {{ archiveCount }}
          </button>
          <template v-if="showArchived">
            <button
              v-for="session in archivedSessions"
              :key="session.id"
              class="tokyo-shell__session-tab tokyo-shell__session-tab--archived"
              :class="{ 'tokyo-shell__session-tab--on': session.id === currentId }"
              :title="session.title || '未命名'"
              @click="open(session.id)"
              @contextmenu.prevent="sessionMenu($event, session)"
            >
              {{ session.title || '未命名' }}
            </button>
          </template>
        </div>
        <button class="tokyo-shell__session-new btn btn--ghost btn--sm" v-tip="'新对话'" @click="start">
          <Icon name="plus" size="sm" />
        </button>
      </header>

      <div class="tokyo-shell__body">
        <slot />
      </div>
    </main>

    <nav class="tokyo-shell__metro" aria-label="地铁线导航">
      <div class="tokyo-shell__track" aria-hidden="true">
        <span
          v-for="n in TRACK_SEGMENTS"
          :key="n"
          class="tokyo-shell__track-seg"
          :class="`tokyo-shell__track-seg--${(n - 1) % 3}`"
          :style="{ animationDelay: `${(n - 1) * 0.4}s` }"
        />
      </div>
      <div class="tokyo-shell__stations">
        <button
          v-for="station in STATIONS"
          :key="station.view"
          class="tokyo-shell__station"
          :class="{ 'tokyo-shell__station--on': isActive(station.view) }"
          :title="station.label"
          @click="go(station.view)"
        >
          <span class="tokyo-shell__dot" aria-hidden="true" />
          <span class="tokyo-shell__station-body">
            <span class="tokyo-shell__icon" v-html="NAV_ICONS[station.icon]" />
            <span class="tokyo-shell__station-text">{{ station.label }}</span>
          </span>
        </button>
      </div>
    </nav>
  </div>
</template>

<style scoped>
.tokyo-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background:
    radial-gradient(ellipse 90% 60% at 50% 0%, color-mix(in srgb, var(--accent) 10%, transparent), transparent),
    radial-gradient(ellipse 70% 50% at 100% 100%, color-mix(in srgb, var(--info) 8%, transparent), transparent),
    var(--bg);
}

.tokyo-shell__stage {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tokyo-shell__sessions {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px 8px;
  border-bottom: 1px solid color-mix(in srgb, var(--info) 22%, var(--border));
  background: color-mix(in srgb, var(--surface) 35%, var(--bg-sunken));
}

.tokyo-shell__sessions-scroll {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-wrap: nowrap;
  gap: 6px;
  overflow-x: auto;
  padding-bottom: 2px;
}

.tokyo-shell__session-tab {
  flex-shrink: 0;
  max-width: 200px;
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  background: var(--surface);
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: var(--transition);
}

.tokyo-shell__session-tab:hover {
  border-color: color-mix(in srgb, var(--info) 40%, var(--border));
  color: var(--text);
}

.tokyo-shell__session-tab--on {
  border-color: var(--info);
  background: color-mix(in srgb, var(--info) 12%, var(--surface));
  color: var(--text);
  box-shadow: 0 0 12px color-mix(in srgb, var(--info) 20%, transparent);
}

.tokyo-shell__session-tab--archive {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
}

.tokyo-shell__session-tab--archived {
  opacity: 0.85;
}

.tokyo-shell__session-new {
  flex-shrink: 0;
}

.tokyo-shell__body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.tokyo-shell__body > :deep(*) {
  flex: 1;
  min-height: 0;
  min-width: 0;
}

.tokyo-shell--admin .tokyo-shell__body {
  overflow: auto;
}

.tokyo-shell--home .tokyo-shell__body {
  overflow: hidden;
}

.tokyo-shell__metro {
  flex-shrink: 0;
  position: relative;
  z-index: 5;
  min-height: 78px;
  padding: 6px 16px calc(10px + env(safe-area-inset-bottom, 0px));
  background: color-mix(in srgb, var(--bg-sunken) 94%, black);
  border-top: 1px solid color-mix(in srgb, var(--accent) 20%, var(--border));
}

.tokyo-shell__track {
  position: absolute;
  left: 28px;
  right: 28px;
  top: 14px;
  display: flex;
  align-items: center;
  gap: 2px;
  height: 3px;
  pointer-events: none;
}

.tokyo-shell__track-seg {
  flex: 1;
  height: 100%;
  border-radius: var(--radius-pill);
  animation: tokyo-track-glow 3.6s ease-in-out infinite;
}

.tokyo-shell__track-seg--0 {
  background: linear-gradient(90deg, var(--accent), var(--info));
  box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 40%, transparent);
}

.tokyo-shell__track-seg--1 {
  background: linear-gradient(90deg, var(--info), color-mix(in srgb, var(--info) 55%, var(--success)));
  box-shadow: 0 0 10px color-mix(in srgb, var(--info) 35%, transparent);
}

.tokyo-shell__track-seg--2 {
  background: linear-gradient(90deg, color-mix(in srgb, var(--info) 45%, var(--success)), var(--success));
  box-shadow: 0 0 10px color-mix(in srgb, var(--success) 35%, transparent);
}

@keyframes tokyo-track-glow {
  0%,
  100% {
    opacity: 0.72;
    filter: brightness(0.95);
  }

  50% {
    opacity: 1;
    filter: brightness(1.08);
  }
}

@media (prefers-reduced-motion: reduce) {
  .tokyo-shell__track-seg {
    animation: none;
    opacity: 0.88;
  }
}

.tokyo-shell__stations {
  position: relative;
  z-index: 1;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  gap: clamp(4px, 1.5vw, 16px);
  overflow-x: auto;
  padding: 0 8px;
  scrollbar-width: thin;
}

.tokyo-shell__station {
  position: relative;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  min-width: 58px;
  max-width: 76px;
  padding: 0 6px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
  transition: var(--transition);
}

.tokyo-shell__station:hover {
  color: var(--text);
}

.tokyo-shell__station:hover .tokyo-shell__dot {
  transform: scale(1.1);
  border-color: color-mix(in srgb, var(--info) 55%, var(--border-strong));
}

.tokyo-shell__station--on {
  color: var(--accent);
}

.tokyo-shell__station--on .tokyo-shell__dot {
  background: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 65%, white);
  box-shadow:
    0 0 0 3px var(--accent-soft),
    0 0 16px color-mix(in srgb, var(--accent) 55%, transparent);
}

.tokyo-shell__dot {
  position: relative;
  z-index: 1;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  border: 2px solid color-mix(in srgb, var(--info) 38%, var(--border-strong));
  background: color-mix(in srgb, var(--surface) 85%, var(--bg));
  box-shadow: 0 0 8px color-mix(in srgb, var(--info) 14%, transparent);
  transition: var(--transition);
}

.tokyo-shell__station--on .tokyo-shell__dot::after {
  content: '';
  position: absolute;
  left: 50%;
  bottom: -6px;
  transform: translateX(-50%);
  border: 4px solid transparent;
  border-top-color: var(--accent);
  opacity: 0.9;
}

.tokyo-shell__station-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 3px;
  min-height: 34px;
  width: 100%;
}

.tokyo-shell__station-text {
  font-size: 11px;
  line-height: 1.15;
  text-align: center;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tokyo-shell__icon {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.tokyo-shell__icon :deep(svg) {
  width: 15px;
  height: 15px;
  display: block;
}
</style>
