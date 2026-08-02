<script setup lang="ts">
import { computed } from 'vue'

import { ICONS, type IconKey } from './icons'

const props = withDefaults(
  defineProps<{
    name: IconKey
    size?: 'sm' | 'md' | 'lg'
  }>(),
  { size: 'md' },
)

const html = computed(() => ICONS[props.name])
</script>

<template>
  <span class="icon" :class="`icon--${size}`" v-html="html" aria-hidden="true" />
</template>

<style scoped>
.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: currentColor;
  line-height: 0;
}

.icon :deep(svg) {
  display: block;
}

.icon--sm :deep(svg) {
  width: 14px;
  height: 14px;
}

.icon--md :deep(svg) {
  width: 16px;
  height: 16px;
}

.icon--lg :deep(svg) {
  width: 18px;
  height: 18px;
}
</style>
