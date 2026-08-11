<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { client } from '../app/client'
import { refreshSnapshot } from '../app/chat/snapshot'
import { errorText, type ContextUsageReport } from '../api/client'
import { currentId, running } from '../app/useChat'

const open = ref(false)
const refreshing = ref(false)
const compacting = ref(false)
const report = ref<ContextUsageReport | null>(null)
const error = ref<string | null>(null)
const compactHint = ref<string | null>(null)

const pct = computed(() => report.value?.pct ?? 0)
const pctLabel = computed(() => `${pct.value.toFixed(1)}%`)

const barTone = computed(() => {
  const value = pct.value
  if (value >= 90) return 'danger'
  if (value >= 70) return 'warn'
  return 'ok'
})

function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}

async function refresh(): Promise<void> {
  const id = currentId.value
  if (!id || refreshing.value) return
  refreshing.value = true
  error.value = null
  try {
    report.value = await client.contextUsage(id)
  } catch (err) {
    report.value = null
    error.value = errorText(err)
  } finally {
    refreshing.value = false
  }
}

async function compact(): Promise<void> {
  const id = currentId.value
  if (!id || compacting.value || running.value) return
  compacting.value = true
  compactHint.value = null
  error.value = null
  try {
    const result = await client.compactSession(id)
    if (result.pruned === 0) {
      compactHint.value = '没有可压缩的工具结果'
    } else {
      compactHint.value = `已压缩 ${result.pruned} 条，约省 ${formatTokens(result.saved_tokens)}`
    }
    await refreshSnapshot()
    await refresh()
  } catch (err) {
    error.value = errorText(err)
  } finally {
    compacting.value = false
  }
}

async function toggle(): Promise<void> {
  if (open.value) {
    open.value = false
    return
  }
  open.value = true
  if (!report.value && !refreshing.value) {
    await refresh()
  }
}

function onDocClick(event: MouseEvent): void {
  const target = event.target as Node | null
  if (!open.value || !target) return
  const root = document.querySelector('.context-usage')
  if (root && !root.contains(target)) open.value = false
}

watch(open, (value) => {
  if (value) document.addEventListener('click', onDocClick, true)
  else document.removeEventListener('click', onDocClick, true)
})

watch(
  currentId,
  (id) => {
    report.value = null
    error.value = null
    open.value = false
    if (id) void refresh()
  },
  { immediate: true },
)

watch(running, (now, prev) => {
  if (prev && !now && currentId.value) void refresh()
})

onBeforeUnmount(() => {
  document.removeEventListener('click', onDocClick, true)
})
</script>

<template>
  <div class="context-usage">
    <button
      type="button"
      class="context-usage__trigger btn"
      :class="{ 'context-usage__trigger--open': open }"
      :disabled="!currentId"
      :title="refreshing ? '估算中…' : report ? `上下文占用 ${pctLabel}（估算）` : '上下文占用（估算）'"
      aria-label="上下文占用"
      @click.stop="toggle"
    >
      <span v-if="refreshing && !report" class="context-usage__spinner" aria-hidden="true" />
      <span v-else-if="report" class="context-usage__badge">{{ pctLabel }}</span>
      <span v-else class="context-usage__icon" aria-hidden="true">◔</span>
    </button>

    <div v-if="open" class="context-usage__panel panel" role="dialog" aria-label="上下文占用">
      <header class="context-usage__head">
        <div class="context-usage__title">上下文占用</div>
        <div class="context-usage__subtitle">DeepSeek V4 词表 · 估算值</div>
      </header>

      <div v-if="refreshing && !report" class="context-usage__hint">正在估算…</div>
      <div v-else-if="error" class="context-usage__hint context-usage__hint--error">{{ error }}</div>
      <template v-else-if="report">
        <div class="context-usage__summary">
          <span class="context-usage__pct">{{ pctLabel }}</span>
          <span class="context-usage__total">
            {{ formatTokens(report.total) }} / {{ formatTokens(report.limit) }}
          </span>
        </div>

        <div class="context-usage__bar" :data-tone="barTone">
          <div class="context-usage__bar-fill" :style="{ width: `${Math.min(pct, 100)}%` }" />
        </div>

        <ul class="context-usage__list">
          <li v-for="item in report.categories" :key="item.id" class="context-usage__row">
            <span class="context-usage__label">{{ item.label }}</span>
            <span class="context-usage__value">{{ formatTokens(item.tokens) }}</span>
          </li>
        </ul>
      </template>

      <p v-if="compactHint" class="context-usage__hint">{{ compactHint }}</p>

      <footer class="context-usage__foot">
        <button
          type="button"
          class="btn btn--ghost context-usage__refresh"
          :disabled="!currentId || compacting || !!running"
          :title="running ? '回合进行中，无法压缩' : '裁掉较旧约一半工具结果；界面仍保留原文'"
          @click="compact"
        >
          {{ compacting ? '压缩中…' : '压缩' }}
        </button>
        <button
          type="button"
          class="btn btn--ghost context-usage__refresh"
          :disabled="refreshing"
          @click="refresh"
        >
          刷新
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.context-usage {
  position: relative;
  flex-shrink: 0;
}

.context-usage__trigger {
  min-width: 44px;
  height: 44px;
  padding: 0 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-pill);
}

.context-usage__trigger--open {
  border-color: var(--border-strong);
  background: var(--surface);
}

.context-usage__icon {
  font-size: 18px;
  line-height: 1;
  opacity: 0.85;
}

.context-usage__badge {
  font-size: var(--text-xs);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}

.context-usage__spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: context-usage-spin 0.7s linear infinite;
}

@keyframes context-usage-spin {
  to {
    transform: rotate(360deg);
  }
}

.context-usage__panel {
  position: absolute;
  left: 0;
  bottom: calc(100% + 8px);
  width: min(320px, calc(100vw - 32px));
  padding: 12px 14px;
  box-shadow: var(--shadow-float);
  z-index: 40;
}

.context-usage__head {
  margin-bottom: 10px;
}

.context-usage__title {
  font-size: var(--text-sm);
  font-weight: 600;
}

.context-usage__subtitle {
  margin-top: 2px;
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.context-usage__summary {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.context-usage__pct {
  font-size: var(--text-lg);
  font-weight: 600;
}

.context-usage__total {
  font-size: var(--text-xs);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.context-usage__bar {
  height: 6px;
  border-radius: var(--radius-pill);
  background: var(--bg-sunken);
  overflow: hidden;
  margin-bottom: 12px;
}

.context-usage__bar-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 0.2s ease;
}

.context-usage__bar[data-tone='warn'] .context-usage__bar-fill {
  background: var(--warn, #d4a017);
}

.context-usage__bar[data-tone='danger'] .context-usage__bar-fill {
  background: var(--danger);
}

.context-usage__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 6px;
}

.context-usage__row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  font-size: var(--text-sm);
}

.context-usage__label {
  color: var(--text-muted);
}

.context-usage__value {
  font-variant-numeric: tabular-nums;
}

.context-usage__hint {
  font-size: var(--text-sm);
  color: var(--text-muted);
  padding: 8px 0;
}

.context-usage__hint--error {
  color: var(--danger);
}

.context-usage__foot {
  margin-top: 10px;
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}

.context-usage__refresh {
  font-size: var(--text-xs);
  padding: 4px 10px;
}
</style>
