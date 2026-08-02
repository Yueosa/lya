<script setup lang="ts">
import { setSidebarCollapsed, sidebarCollapsed } from '../app/useShell'
import Icon from './Icon.vue'

defineProps<{ title: string }>()
</script>

<template>
  <header class="view-head">
    <button
      v-if="sidebarCollapsed"
      class="view-head__menu btn btn--ghost"
      v-tip="'展开侧栏'"
      @click="setSidebarCollapsed(false)"
    >
      <Icon name="menu" size="sm" />
    </button>
    <h2 class="view-head__title">{{ title }}</h2>
    <div v-if="$slots.actions" class="view-head__actions">
      <slot name="actions" />
    </div>
  </header>
</template>
