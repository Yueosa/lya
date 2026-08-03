<!--
  应用根。

  它做三件事：选外壳、切视图、挂浮层宿主。**外壳由主题决定、视图只有一份**——
  这条边界写在 shell/types.ts 里。
-->

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

import { shellFor } from '../shell/registry'
import type { View } from '../shell/types'
import { themeId } from '../themes'
import UiHost from '../ui/UiHost.vue'
import ChatView from '../views/ChatView.vue'
import ConfigView from '../views/ConfigView.vue'
import HomeView from '../views/HomeView.vue'
import MemoryView from '../views/MemoryView.vue'
import ModelsView from '../views/ModelsView.vue'
import SessionPanel from '../views/session/SessionPanel.vue'
import ThemeView from '../views/ThemeView.vue'
import ToolsView from '../views/ToolsView.vue'
import SessionsView from '../views/SessionsView.vue'
import {
  bootstrap,
  client,
  currentId,
  loadModels,
  refreshRuntimeDefaults,
  refreshSessions,
} from './useChat'
import { setupImageLightbox } from '../ui/useImageLightbox'

const view = ref<View>('home')

const shell = computed(() => shellFor(themeId.value))

// 图片令牌要尽早拿，否则先渲染出来的本地图片会是坏的
void bootstrap()
void loadModels()
void refreshRuntimeDefaults()
const stopLightbox = setupImageLightbox()

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
  })
})

function navigate(next: View): void {
  // 没有会话时会话设置无处可挂；对话页本身会显示空态，侧栏里再建
  if (next === 'settings' && !currentId.value) {
    view.value = 'chat'
    return
  }
  view.value = next
}
</script>

<template>
  <component :is="shell" :view="view" @navigate="navigate">
    <Transition name="lya-page" mode="out-in">
      <HomeView v-if="view === 'home'" key="home" />
      <ChatView v-else-if="view === 'chat'" :key="`chat-${currentId ?? 'none'}`" />
      <SessionsView v-else-if="view === 'sessions'" key="sessions" @opened="view = 'chat'" />
      <SessionPanel v-else-if="view === 'settings' && currentId" :key="`settings-${currentId}`" layout="page" />
      <MemoryView v-else-if="view === 'memory'" key="memory" />
      <ToolsView v-else-if="view === 'tools'" key="tools" />
      <ModelsView v-else-if="view === 'models'" key="models" />
      <ThemeView v-else-if="view === 'theme'" key="theme" />
      <ConfigView v-else-if="view === 'config'" key="config" />
      <div v-else key="todo" class="app__todo">
        <p>在左侧选一个会话，或点「新对话」开始。</p>
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
</style>
