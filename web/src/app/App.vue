<!--
  应用根。

  它做三件事：选外壳、切视图、挂浮层宿主。**外壳由主题决定、视图只有一份**——
  这条边界写在 shell/types.ts 里。
-->

<script setup lang="ts">
import { computed, ref } from 'vue'

import { shellFor } from '../shell/registry'
import type { View } from '../shell/types'
import { applyTheme, currentTheme, THEMES } from '../themes'
import UiHost from '../ui/UiHost.vue'
import ChatView from '../views/ChatView.vue'
import SessionsView from '../views/SessionsView.vue'
import { currentId, meta } from './useChat'

const theme = ref(currentTheme())
const view = ref<View>('sessions')

const shell = computed(() => shellFor(theme.value))

function navigate(next: View): void {
  // 「开始对话」没有会话可开时先去列表，免得进到一个空白的聊天页
  view.value = next === 'chat' && !currentId.value ? 'sessions' : next
}

function switchTheme(id: string): void {
  theme.value = id
  applyTheme(id)
}
</script>

<template>
  <component :is="shell" :view="view" @navigate="navigate">
    <ChatView v-if="view === 'chat'" />
    <SessionsView v-else-if="view === 'sessions'" @opened="view = 'chat'" />
    <div v-else class="app__todo">
      <p>{{ view }} 还没做。</p>
      <p class="app__hint">当前会话：{{ meta?.title ?? '（没有）' }}</p>
    </div>
  </component>

  <!-- 主题切换先挂在角上，等设置页做好再收进去 -->
  <div class="app__themes panel">
    <button
      v-for="item in THEMES"
      :key="item.id"
      class="btn btn--sm"
      :class="{ 'btn--primary': theme === item.id }"
      @click="switchTheme(item.id)"
    >
      {{ item.label }}
    </button>
  </div>

  <UiHost />
</template>

<style scoped>
.app__todo {
  padding: 24px;
}

.app__hint {
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.app__themes {
  position: fixed;
  right: 12px;
  bottom: 12px;
  z-index: 40;
  display: flex;
  gap: 6px;
  padding: 6px;
}
</style>
