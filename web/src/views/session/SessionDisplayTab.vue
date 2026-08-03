<script setup lang="ts">
import { prefs } from '../../app/usePrefs'

const DISPLAY: { key: keyof typeof prefs; label: string }[] = [
  { key: 'hideReasoning', label: '隐藏思考' },
  { key: 'hideTools', label: '隐藏工具调用' },
  { key: 'hideResolvedHitl', label: '隐藏已答复的打断' },
  { key: 'hideNotices', label: '隐藏模式变更' },
  { key: 'followStream', label: '跟随流式输出' },
  { key: 'autoCollapseAside', label: '流式结束后自动收起思考/工具' },
  { key: 'codeBlockWrap', label: '代码块自动换行' },
]
</script>

<template>
  <div class="session-tab">

    <label v-for="item in DISPLAY" :key="item.key" class="session-tab__row">
      <input v-model="prefs[item.key]" type="checkbox" />
      <span class="session-tab__name">{{ item.label }}</span>
    </label>

    <label class="session-tab__row session-tab__row--range">
      <span class="session-tab__name">侧栏块折叠阈值（行）</span>
      <input
        v-model.number="prefs.asideFoldLineThreshold"
        class="session-tab__range"
        type="range"
        min="0"
        max="64"
        step="1"
      />
      <input
        v-model.number="prefs.asideFoldLineThreshold"
        class="session-tab__num input"
        type="number"
        min="0"
        max="128"
      />
    </label>
  </div>
</template>

<style scoped>
.session-tab {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.session-tab__banner {
  margin: 0 0 12px;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--info) 12%, var(--surface));
  border: var(--border-width) solid color-mix(in srgb, var(--info) 35%, transparent);
  font-size: var(--text-sm);
  line-height: var(--leading);
  color: var(--text-muted);
}

.session-tab__banner strong {
  color: var(--text);
}

.session-tab__row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  cursor: pointer;
}

.session-tab__row:hover {
  background: var(--surface-hover);
}

.session-tab__row--range {
  flex-wrap: wrap;
  cursor: default;
}

.session-tab__name {
  flex: 1;
  font-weight: 500;
}

.session-tab__range {
  flex: 1;
  min-width: 120px;
}

.session-tab__num {
  width: 4.5rem;
  padding: 4px 8px;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}
</style>
