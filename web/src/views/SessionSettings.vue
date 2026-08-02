<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import type { ActionInfo } from '../api/client'
import { client, currentId, loadTools, meta, readOnly, toggleTool, tools } from '../app/useChat'
import { prefs } from '../app/usePrefs'
import { toast } from '../ui/useToast'

const actions = ref<ActionInfo[]>([])
const resetting = ref(false)

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
  <div class="settings">
    <section>
      <div class="settings__head">
        <h3 class="settings__title">本会话 · 工具</h3>
        <button
          v-if="!readOnly"
          class="btn btn--sm"
          :disabled="resetting"
          @click="resetToGlobalDefault"
        >
          {{ resetting ? '恢复中…' : '恢复全局默认' }}
        </button>
      </div>
      <p class="settings__lead">
        只影响当前会话；全局默认在「工具」页的「全局默认」里配置。
      </p>

      <details v-for="tool in reachable" :key="tool.name" class="settings__card panel">
        <summary class="catalog-card__summary settings__summary">
          <input
            type="checkbox"
            :checked="tool.enabled !== false"
            :disabled="readOnly"
            @click.stop
            @change="toggleTool(tool.name, ($event.target as HTMLInputElement).checked)"
          />
          <span class="settings__name">{{ tool.raw_name }}</span>
          <code class="settings__perm">{{ tool.permission }}</code>
        </summary>
        <div class="settings__body">
          <p class="settings__desc">{{ tool.description }}</p>
          <p class="settings__meta">最低模式 · {{ tool.min_mode }}</p>
        </div>
      </details>

      <details v-for="tool in blocked" :key="`b-${tool.name}`" class="settings__card settings__card--off panel">
        <summary class="catalog-card__summary settings__summary">
          <span class="settings__name">{{ tool.raw_name }}</span>
          <code class="settings__perm">需 {{ tool.min_mode }}</code>
        </summary>
        <div class="settings__body">
          <p class="settings__desc">{{ tool.description }}</p>
          <p class="settings__meta">当前模式 {{ meta?.work_mode ?? '—' }} 下不可启用</p>
        </div>
      </details>
    </section>

    <section>
      <h3 class="settings__title">动作</h3>
      <p class="settings__lead">动作不可关闭；展开查看说明。</p>
      <details v-for="action in actions" :key="action.name" class="settings__card settings__card--off panel">
        <summary class="catalog-card__summary settings__summary">
          <span class="settings__name">{{ action.raw_name }}</span>
          <code class="settings__perm">{{ action.flow === 'await_human' ? 'HITL' : 'auto' }}</code>
        </summary>
        <div class="settings__body">
          <p class="settings__desc">{{ action.description }}</p>
          <p class="settings__meta">可见模式 · {{ action.visible_in.join('、') }}</p>
        </div>
      </details>
    </section>

    <section>
      <h3 class="settings__title">显示</h3>
      <label v-for="item in DISPLAY" :key="item.key" class="settings__row">
        <input v-model="prefs[item.key]" type="checkbox" />
        <span class="settings__name">{{ item.label }}</span>
      </label>
      <label class="settings__row settings__row--range">
        <span class="settings__name">侧栏块折叠阈值（行）</span>
        <input
          v-model.number="prefs.asideFoldLineThreshold"
          class="settings__range"
          type="range"
          min="0"
          max="64"
          step="1"
        />
        <input
          v-model.number="prefs.asideFoldLineThreshold"
          class="settings__num input"
          type="number"
          min="0"
          max="128"
        />
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

.settings__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

.settings__title {
  margin: 0 0 8px;
  font-size: var(--text-md);
  font-weight: 600;
}

.settings__head .settings__title {
  margin: 0;
}

.settings__lead {
  margin: 0 0 10px;
  font-size: var(--text-sm);
  color: var(--text-muted);
  line-height: var(--leading);
}

.settings__card {
  margin-bottom: 6px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.settings__card--off {
  opacity: 0.65;
}

.settings__summary {
  padding: 10px 12px;
}

.settings__body {
  padding: 0 12px 12px 36px;
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
  margin: 0 0 6px;
  color: var(--text-muted);
  font-size: var(--text-sm);
  line-height: 1.5;
  white-space: normal;
  word-break: break-word;
}

.settings__meta {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--text-faint);
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

.settings__row--range {
  flex-wrap: wrap;
  cursor: default;
}

.settings__range {
  flex: 1;
  min-width: 120px;
}

.settings__num {
  width: 4.5rem;
  padding: 4px 8px;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.settings__note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}
</style>
