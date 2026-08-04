<!--
  应用根。

  它做三件事：选外壳、切视图、挂浮层宿主。**外壳由主题决定、视图只有一份**——
  这条边界写在 shell/types.ts 里。
-->

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import { shellFor } from '../shell/registry'
import type { View } from '../shell/types'
import { themeId } from '../themes'
import UiHost from '../ui/UiHost.vue'
import ChatView from '../views/ChatView.vue'
import ConfigView from '../views/ConfigView.vue'
import HomeView from '../views/HomeView.vue'
import MemoryView from '../views/MemoryView.vue'
import ModelsView from '../views/ModelsView.vue'
import PersonaView from '../views/PersonaView.vue'
import SessionPanel from '../views/session/SessionPanel.vue'
import StorageView from '../views/StorageView.vue'
import ThemeView from '../views/ThemeView.vue'
import ToolsView from '../views/ToolsView.vue'
import SessionsView from '../views/SessionsView.vue'
import {
  archivedSessions,
  bootstrap,
  client,
  currentId,
  hydrating,
  loadModels,
  openSession,
  refreshRuntimeDefaults,
  refreshSessions,
  sessions,
} from './useChat'
import { savedSession, setView, view } from './useNav'
import { setupImageLightbox } from '../ui/useImageLightbox'

const shell = computed(() => shellFor(themeId.value))

/**
 * 加载遮罩放在这里而不是 ChatView 里：页面切换有 out-in 过渡，等 ChatView 挂上来
 * 的时候快照往往已经到了，遮罩根本没机会画出来（MC 主题过渡是 0ms，所以只有它「正常」）。
 * 再加一个最短显示时长，免得快的时候闪一下反而更难看。
 */
const BUSY_MIN_MS = 240
const busy = ref(false)
let busyTimer: number | null = null
let busySince = 0

watch(hydrating, (on) => {
  if (busyTimer !== null) {
    clearTimeout(busyTimer)
    busyTimer = null
  }
  if (on) {
    busySince = performance.now()
    busy.value = true
    return
  }
  const rest = BUSY_MIN_MS - (performance.now() - busySince)
  if (rest <= 0) {
    busy.value = false
    return
  }
  busyTimer = window.setTimeout(() => {
    busy.value = false
    busyTimer = null
  }, rest)
})

// 图片令牌要尽早拿，否则先渲染出来的本地图片会是坏的
void bootstrap()
void loadModels()
void refreshRuntimeDefaults()
// 会话列表在这里拉而不是各外壳自己拉：原先只有 DefaultShell 的 onMounted 拉过，
// 于是 MC 外壳的主菜单永远显示「0 活跃 0 归档」，splash 也抽不到会话名——数据在
// 服务端好好的，只是没人去取。谁需要会话列表不该由排版决定
const sessionsReady = refreshSessions()
const stopLightbox = setupImageLightbox()

/**
 * 回到刷新前的位置。
 *
 * 会话可能已经不在了——在别处删掉、或者换了个数据目录，所以要等列表回来核对一遍。
 * 对不上就退回首页，别让用户对着一个永远加载不出来的空对话页发呆。
 *
 * 核对期间要盖住遮罩：那会儿视图已经是 `chat` 而 `currentId` 还是空的，不盖就会先闪
 * 一下空对话页（`settings` 更难看，会闪出「在左侧选一个会话」）。
 */
const needsRestore = view.value === 'chat' || view.value === 'settings'
const restoring = ref(needsRestore)

void (async () => {
  if (!needsRestore) return
  try {
    const id = savedSession()
    await sessionsReady
    const known =
      id !== null && [...sessions.value, ...archivedSessions.value].some((s) => s.id === id)
    if (!known) {
      setView('home')
      return
    }
    await openSession(id)
  } finally {
    restoring.value = false
  }
})()

// 全局事件：配置或会话列表在别处变了，这边跟着刷新。和会话流分开订阅——
// 它们与「当前打开哪个会话」无关，换会话不该断掉
onMounted(() => {
  const stop = client.subscribeGlobal((kind) => {
    if (kind === 'sessions_changed') void refreshSessions()
    if (kind === 'config_changed') {
      void loadModels()
      void refreshRuntimeDefaults()
    }
  })
  onUnmounted(() => {
    stop()
    stopLightbox()
    if (busyTimer !== null) clearTimeout(busyTimer)
  })
})

function navigate(next: View): void {
  // 没有会话时会话设置无处可挂；对话页本身会显示空态，侧栏里再建
  if (next === 'settings' && !currentId.value) {
    setView('chat')
    return
  }
  setView(next)
}
</script>

<template>
  <component :is="shell" :view="view" @navigate="navigate">
    <Transition name="lya-page" mode="out-in">
      <HomeView v-if="view === 'home'" key="home" />
      <ChatView v-else-if="view === 'chat'" :key="`chat-${currentId ?? 'none'}`" />
      <SessionsView v-else-if="view === 'sessions'" key="sessions" @opened="setView('chat')" />
      <SessionPanel v-else-if="view === 'settings' && currentId" :key="`settings-${currentId}`" layout="page" />
      <MemoryView v-else-if="view === 'memory'" key="memory" />
      <ToolsView v-else-if="view === 'tools'" key="tools" />
      <ModelsView v-else-if="view === 'models'" key="models" />
      <ThemeView v-else-if="view === 'theme'" key="theme" />
      <PersonaView v-else-if="view === 'persona'" key="persona" />
      <ConfigView v-else-if="view === 'config'" key="config" />
      <StorageView v-else-if="view === 'storage'" key="storage" />
      <div v-else key="todo" class="app__todo">
        <p>在左侧选一个会话，或点「新对话」开始。</p>
      </div>
    </Transition>

    <Transition name="lya-veil">
      <div v-if="busy || restoring" class="app__busy" aria-live="polite">
        <span class="app__busy-spinner" aria-hidden="true" />
        <span class="app__busy-text">加载中…</span>
      </div>
    </Transition>
  </component>

  <UiHost />
</template>

<style scoped>
.app__todo {
  padding: 24px;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.app__busy {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  background: color-mix(in srgb, var(--bg) 94%, transparent);
  pointer-events: none;
}

.app__busy-spinner {
  width: 28px;
  height: 28px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: lya-spin 0.75s linear infinite;
}

.app__busy-text {
  padding: 8px 16px;
  border-radius: var(--radius-pill);
  background: var(--surface);
  border: var(--border-width) solid var(--border);
  color: var(--text-muted);
  font-size: var(--text-sm);
}
</style>
