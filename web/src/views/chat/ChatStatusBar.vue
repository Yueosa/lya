<!-- 流式进行中的状态条。 -->

<script setup lang="ts">
import { computed } from 'vue'

import { elapsed, phase, round } from '../../app/useChat'

const elapsedText = computed(() => `${(elapsed.value / 1000).toFixed(1)}s`)
</script>

<template>
  <div v-if="phase" class="chat__status">
    <span>{{ phase.text }}</span>
    <span v-if="round > 1" class="chat__dim">第 {{ round }} 轮</span>
    <span class="chat__gap" />
    <span class="chat__dim">{{ elapsedText }}</span>
  </div>
</template>
