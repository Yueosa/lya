<!-- 右侧抽屉壳（详情 / 设置）。 -->

<script setup lang="ts">
import Icon from '../../ui/Icon.vue'

defineProps<{ title: string }>()
defineEmits<{ close: [] }>()
</script>

<template>
  <aside class="chat__side">
    <header class="chat__side-head">
      <strong>{{ title }}</strong>
      <span class="chat__gap" />
      <button class="btn btn--sm btn--ghost" @click="$emit('close')">
        <Icon name="chevronRight" size="sm" />
      </button>
    </header>
    <div class="chat__side-body"><slot /></div>
  </aside>
</template>
