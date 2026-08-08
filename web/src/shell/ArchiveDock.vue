<!--
  归档会话抽屉：栏底一条，收起来只占一行。

  状态在 useArchiveDock（展开记忆、正在看归档就自动打开）。这里只画壳——头、计数、
  列表槽。行长什么样由调用方用默认插槽决定：会话列表是整行、Momotalk 是联系人卡、
  默认侧栏是一行字。抄第二遍 markup 的那次，蔚蓝档案外壳干脆漏掉了归档。
-->

<script setup lang="ts">
import { useArchiveDock } from './useArchiveDock'

withDefaults(
  defineProps<{
    /** 头上那几个字。外壳偏短，会话列表视图写全称。 */
    label?: string
    /** 没有归档时整块藏起来（会话列表）；外壳始终占一行，空时出 emptyText。 */
    hideWhenEmpty?: boolean
    emptyText?: string | null
  }>(),
  {
    label: '归档对话',
    hideWhenEmpty: false,
    emptyText: '暂无归档',
  },
)

const { open, count, viewing, items } = useArchiveDock()

defineExpose({ open, count, viewing, items })

function toggle(): void {
  open.value = !open.value
}
</script>

<template>
  <div
    v-if="!hideWhenEmpty || count > 0"
    class="archive-dock"
    :class="{
      'archive-dock--open': open,
      'archive-dock--has': count > 0,
      'archive-dock--active': viewing,
    }"
  >
    <button
      class="archive-dock__head"
      type="button"
      :aria-expanded="open"
      @click="toggle"
    >
      <slot name="icon" />
      <span class="archive-dock__label">{{ label }}</span>
      <span v-if="count" class="archive-dock__count">{{ count }}</span>
      <span class="archive-dock__chevron">
        <slot name="chevron">›</slot>
      </span>
    </button>

    <div v-if="open" class="archive-dock__list">
      <slot :items="items" />
      <p v-if="!count && emptyText" class="archive-dock__empty">{{ emptyText }}</p>
    </div>
  </div>
</template>

<style scoped>
.archive-dock {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-top: var(--border-width) solid var(--border);
}

.archive-dock__head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  width: 100%;
  padding: 10px 12px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-xs);
  font-weight: 700;
  text-align: left;
  cursor: pointer;
}

.archive-dock__head:hover {
  color: var(--accent);
}

.archive-dock__label {
  flex: 1;
}

.archive-dock__count {
  padding: 1px 8px;
  border-radius: var(--radius-pill);
  background: var(--surface-active);
  font-size: var(--text-xs);
  font-weight: 700;
}

.archive-dock__chevron {
  display: inline-flex;
  align-items: center;
  font-size: 15px;
  line-height: 1;
  transition: transform var(--transition);
}

.archive-dock--open .archive-dock__chevron {
  transform: rotate(90deg);
}

.archive-dock__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.archive-dock__empty {
  margin: 0;
  padding: 6px 14px;
  color: var(--text-faint);
  font-size: var(--text-xs);
}
</style>
