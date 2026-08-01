<!--
  默认外壳：左侧栏 + 内容区。

  东京夜和 MTF 都用它——这两套的排版本来就是同一种，差别在配色和质感上，那些
  已经由 token 表达了。只有确实需要另一种排版的主题（比如方块世界的标题画面）
  才另写一份。
-->

<script setup lang="ts">
import { NAV_ITEMS, type ShellProps } from './types'

defineProps<ShellProps>()
defineEmits<{ navigate: [view: import('./types').View] }>()
</script>

<template>
  <div class="shell">
    <aside class="shell__side">
      <div class="shell__brand">lya</div>
      <nav class="shell__nav">
        <button
          v-for="item in NAV_ITEMS"
          :key="item.view"
          class="btn btn--ghost shell__nav-item"
          :class="{ 'shell__nav-item--on': view === item.view }"
          @click="$emit('navigate', item.view)"
        >
          <span>{{ item.icon }}</span>
          <span>{{ item.label }}</span>
        </button>
      </nav>
    </aside>
    <main class="shell__main">
      <slot />
    </main>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  height: 100%;
}

.shell__side {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 12px;
  background: var(--bg-sunken);
  border-right: var(--border-width) solid var(--border);
}

.shell__brand {
  font-size: var(--text-lg);
  font-weight: 700;
  color: var(--accent);
  padding: 0 6px;
}

.shell__nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.shell__nav-item {
  justify-content: flex-start;
  width: 100%;
}

.shell__nav-item--on {
  background: var(--surface-active);
  color: var(--text);
}

.shell__main {
  flex: 1;
  min-width: 0;
  overflow: auto;
}
</style>
