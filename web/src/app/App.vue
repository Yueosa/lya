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
import BranchTree from '../views/BranchTree.vue'
import ChatView from '../views/ChatView.vue'
import SessionSettings from '../views/SessionSettings.vue'
import SessionsView from '../views/SessionsView.vue'
import { bootstrap, currentId, loadModels } from './useChat'

const theme = ref(currentTheme())
const view = ref<View>('sessions')

const shell = computed(() => shellFor(theme.value))

// 图片令牌要尽早拿，否则先渲染出来的本地图片会是坏的
void bootstrap()
void loadModels()

function navigate(next: View): void {
  // 「开始对话」没有会话可开时先去列表，免得进到一个空白的聊天页
  const needsSession = next === 'chat' || next === 'tree' || next === 'settings'
  view.value = needsSession && !currentId.value ? 'sessions' : next
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
    <BranchTree v-else-if="view === 'tree' && currentId" :key="currentId" />
    <SessionSettings v-else-if="view === 'settings' && currentId" :key="currentId" />
    <div v-else class="app__todo">
      <p>先在会话列表里打开一个会话。</p>
    </div>
  </component>

  <!-- 主题切换先挂在角上；显示偏好已经收进会话设置页 -->
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
