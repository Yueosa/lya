<!-- 右侧抽屉壳（会话 / 分支等）。 -->

<script setup lang="ts">
import Icon from '../../ui/Icon.vue'

defineProps<{
  title: string
  wide?: boolean
}>()
defineEmits<{ close: [] }>()
</script>

<template>
  <aside class="chat__side" :class="{ 'chat__side--wide': wide }">
    <header class="chat__side-head">
      <strong>{{ title }}</strong>
      <span class="chat__gap" />
      <button class="btn btn--sm btn--ghost" @click="$emit('close')">
        <Icon name="chevronRight" size="sm" />
      </button>
    </header>
    <div class="chat__side-body" :class="{ 'chat__side-body--flush': wide }"><slot /></div>
  </aside>
</template>
