<!--
  思考与工具调用的折叠卡片。

  流式过程中展开（让人看见它在干什么），一结束就收起（回看时这些是噪音）。
  用户手动动过之后就听用户的，不再自动收——自动行为覆盖手动操作是很招人烦的。
-->

<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{
  icon: string
  label: string
  /** 还在进行中。 */
  busy?: boolean
  /** 出错了，标题显示为危险色。 */
  failed?: boolean
}>()

const open = ref(props.busy ?? false)
/** 用户是不是自己点过。 */
const touched = ref(false)

watch(
  () => props.busy,
  (busy, was) => {
    if (touched.value) return
    // 只在「从进行中变成结束」这一刻收起
    if (was && !busy) open.value = false
    if (busy) open.value = true
  },
)

function toggle(): void {
  touched.value = true
  open.value = !open.value
}
</script>

<template>
  <div class="fold" :class="{ 'fold--failed': failed }">
    <button class="fold__head" type="button" @click="toggle">
      <span class="fold__icon">{{ icon }}</span>
      <span class="fold__label">{{ label }}</span>
      <span v-if="busy" class="fold__dot" />
      <span class="fold__caret">{{ open ? '▾' : '▸' }}</span>
    </button>
    <div v-if="open" class="fold__body">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.fold {
  min-width: 0;
  margin: 4px 0;
  border: var(--border-width) solid var(--border);
  border-left: 3px solid var(--info);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  overflow: hidden;
}

.fold--failed {
  border-left-color: var(--danger);
}

.fold__head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 10px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
}

.fold__head:hover {
  background: var(--surface-hover);
}

.fold__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
}

.fold--failed .fold__label {
  color: var(--danger);
}

/* 进行中的呼吸点 */
.fold__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--info);
  animation: pulse 1.2s ease-in-out infinite;
}

@keyframes pulse {
  50% {
    opacity: 0.25;
  }
}

.fold__caret {
  color: var(--text-faint);
}

.fold__body {
  padding: 8px 10px;
  border-top: var(--border-width) solid var(--border);
  /* 工具输出可能很长，给个上限，别把整屏顶走 */
  max-height: 320px;
  overflow: auto;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
