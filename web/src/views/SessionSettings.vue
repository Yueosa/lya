<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import type { ActionInfo } from '../api/client'
import { client, loadTools, meta, readOnly, toggleTool, tools } from '../app/useChat'
import { prefs } from '../app/usePrefs'

const actions = ref<ActionInfo[]>([])

onMounted(async () => {
  await loadTools()
  try {
    actions.value = await client.actions()
  } catch {
    // optional
  }
})

const reachable = computed(() => tools.value.filter((tool) => !outOfReach(tool.min_mode)))
const blocked = computed(() => tools.value.filter((tool) => outOfReach(tool.min_mode)))

function outOfReach(minMode: string): boolean {
  const order = ['ask', 'edit', 'agent']
  const current = order.indexOf(meta.value?.work_mode ?? 'agent')
  return order.indexOf(minMode) > current
}

const DISPLAY: { key: keyof typeof prefs; label: string }[] = [
  { key: 'hideReasoning', label: '隐藏思考' },
  { key: 'hideTools', label: '隐藏工具调用' },
  { key: 'hideResolvedHitl', label: '隐藏已答复的打断' },
  { key: 'hideNotices', label: '隐藏模式变更' },
  { key: 'followStream', label: '跟随流式输出' },
]
</script>

<template>
  <div class="settings">
    <section>
      <h3 class="settings__title">工具</h3>
      <label v-for="tool in reachable" :key="tool.name" class="settings__card">
        <div class="settings__card-head">
          <input
            type="checkbox"
            :checked="tool.enabled !== false"
            @change="toggleTool(tool.name, ($event.target as HTMLInputElement).checked)"
          />
          <span class="settings__name">{{ tool.raw_name }}</span>
          <code class="settings__perm">{{ tool.permission }}</code>
        </div>
        <p class="settings__desc">{{ tool.description }}</p>
      </label>
      <div v-for="tool in blocked" :key="`b-${tool.name}`" class="settings__card settings__card--off">
        <div class="settings__card-head">
          <span class="settings__name">{{ tool.raw_name }}</span>
          <code class="settings__perm">需 {{ tool.min_mode }}</code>
        </div>
        <p class="settings__desc">{{ tool.description }}</p>
      </div>
    </section>

    <section>
      <h3 class="settings__title">动作</h3>
      <div v-for="action in actions" :key="action.name" class="settings__card settings__card--off">
        <div class="settings__card-head">
          <span class="settings__name">{{ action.raw_name }}</span>
          <code class="settings__perm">{{ action.flow === 'await_human' ? 'HITL' : 'auto' }}</code>
        </div>
        <p class="settings__desc">{{ action.description }}</p>
      </div>
    </section>

    <section>
      <h3 class="settings__title">显示</h3>
      <label v-for="item in DISPLAY" :key="item.key" class="settings__row">
        <input v-model="prefs[item.key]" type="checkbox" />
        <span class="settings__name">{{ item.label }}</span>
      </label>
    </section>

    <p v-if="readOnly" class="settings__note">已归档 · 只读</p>
  </div>
</template>

<style scoped>
.settings {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.settings__title {
  margin: 0 0 8px;
  font-size: var(--text-md);
  font-weight: 600;
}

.settings__card {
  display: block;
  padding: 10px 12px;
  margin-bottom: 6px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  cursor: pointer;
}

.settings__card:hover {
  background: var(--surface-hover);
}

.settings__card--off {
  opacity: 0.65;
  cursor: default;
}

.settings__card-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.settings__name {
  flex: 1;
  font-size: var(--text-sm);
  font-weight: 600;
}

.settings__perm {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.settings__desc {
  margin: 0;
  padding-left: 24px;
  color: var(--text-muted);
  font-size: var(--text-sm);
  line-height: 1.5;
  white-space: normal;
  word-break: break-word;
}

.settings__row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  cursor: pointer;
}

.settings__row:hover {
  background: var(--surface-hover);
}

.settings__note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}
</style>
