<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import { client, currentId, loadTools, meta, readOnly, toggleTool, tools } from '../../app/useChat'
import { toast } from '../../ui/useToast'

const resetting = ref(false)

onMounted(loadTools)

const reachable = computed(() => tools.value.filter((tool) => !outOfReach(tool.min_mode)))
const blocked = computed(() => tools.value.filter((tool) => outOfReach(tool.min_mode)))

function outOfReach(minMode: string): boolean {
  const order = ['ask', 'edit', 'agent']
  const current = order.indexOf(meta.value?.work_mode ?? 'agent')
  return order.indexOf(minMode) > current
}

async function resetToGlobalDefault(): Promise<void> {
  const id = currentId.value
  if (!id || readOnly.value) return
  resetting.value = true
  try {
    await client.patchSession(id, { enabled_tools: null })
    await loadTools()
    toast('已恢复为全局默认', 'success')
  } catch (error) {
    toast(`恢复失败：${error instanceof Error ? error.message : String(error)}`, 'error')
  } finally {
    resetting.value = false
  }
}
</script>

<template>
  <div class="session-tab">
    <div class="session-tab__head">
      <button
        v-if="!readOnly"
        class="btn btn--sm"
        :disabled="resetting"
        @click="resetToGlobalDefault"
      >
        {{ resetting ? '恢复中…' : '恢复全局默认' }}
      </button>
    </div>

    <details v-for="tool in reachable" :key="tool.name" class="session-tab__card panel">
      <summary class="catalog-card__summary session-tab__summary">
        <input
          type="checkbox"
          :checked="tool.enabled !== false"
          :disabled="readOnly"
          @click.stop
          @change="toggleTool(tool.name, ($event.target as HTMLInputElement).checked)"
        />
        <span class="session-tab__name">{{ tool.raw_name }}</span>
        <code class="session-tab__perm">{{ tool.permission }}</code>
      </summary>
      <div class="session-tab__body">
        <p class="session-tab__desc">{{ tool.description }}</p>
        <p class="session-tab__meta">最低模式 · {{ tool.min_mode }}</p>
      </div>
    </details>

    <details v-for="tool in blocked" :key="`b-${tool.name}`" class="session-tab__card session-tab__card--off panel">
      <summary class="catalog-card__summary session-tab__summary">
        <span class="session-tab__name">{{ tool.raw_name }}</span>
        <code class="session-tab__perm">需 {{ tool.min_mode }}</code>
      </summary>
      <div class="session-tab__body">
        <p class="session-tab__desc">{{ tool.description }}</p>
        <p class="session-tab__meta">当前模式 {{ meta?.work_mode ?? '—' }} 下不可启用</p>
      </div>
    </details>
  </div>
</template>

<style scoped>
.session-tab {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.session-tab__head {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 4px;
}

.session-tab__lead {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-muted);
  line-height: var(--leading);
}

.session-tab__card {
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.session-tab__card--off {
  opacity: 0.65;
}

.session-tab__summary {
  padding: 10px 12px;
}

.session-tab__body {
  padding: 0 12px 12px 36px;
}

.session-tab__name {
  flex: 1;
  font-size: var(--text-sm);
  font-weight: 600;
}

.session-tab__perm {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.session-tab__desc {
  margin: 0 0 6px;
  color: var(--text-muted);
  font-size: var(--text-sm);
  line-height: 1.5;
  white-space: normal;
  word-break: break-word;
}

.session-tab__meta {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--text-faint);
}
</style>
