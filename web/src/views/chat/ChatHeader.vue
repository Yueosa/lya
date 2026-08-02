<!-- 聊天页顶栏：标题与抽屉开关。 -->

<script setup lang="ts">
import { meta, readOnly } from '../../app/useChat'
import { setSidebarCollapsed, sidebarCollapsed } from '../../app/useShell'
import Icon from '../../ui/Icon.vue'

defineProps<{
  detailOpen: boolean
  settingsOpen: boolean
  treeOpen: boolean
}>()

defineEmits<{
  toggleDetail: []
  toggleSettings: []
  toggleTree: []
}>()
</script>

<template>
  <header class="chat__head" :class="{ 'chat__head--sidebar-collapsed': sidebarCollapsed }">
    <button
      v-if="sidebarCollapsed"
      class="btn btn--ghost chat__sidebar-btn"
      v-tip="'展开侧栏'"
      @click="setSidebarCollapsed(false)"
    >
      <Icon name="menu" size="sm" />
    </button>
    <span class="chat__title">{{ meta?.title || '未命名会话' }}</span>
    <span v-if="readOnly" class="chat__tag">已归档</span>
    <span class="chat__gap" />
    <button class="btn btn--sm" :class="{ 'btn--on': detailOpen }" @click="$emit('toggleDetail')">
      <Icon name="info" size="sm" />
      <span>详情</span>
    </button>
    <button class="btn btn--sm" :class="{ 'btn--on': settingsOpen }" @click="$emit('toggleSettings')">
      <Icon name="settings" size="sm" />
      <span>设置</span>
    </button>
    <button class="btn btn--sm" :class="{ 'btn--on': treeOpen }" @click="$emit('toggleTree')">
      <Icon name="branch" size="sm" />
      <span>分支</span>
    </button>
  </header>
</template>
