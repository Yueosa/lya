<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { ActionInfo, ToolInfo } from '../api/client'
import { client } from '../app/useChat'
import Picker from '../ui/Picker.vue'
import type { PickerOption } from '../ui/Picker.vue'
import ViewHead from '../ui/ViewHead.vue'
import { toast } from '../ui/useToast'
import {
  buildToolsEnabledPayload,
  readGlobalToolsMode,
  toolLimits,
  type GlobalToolsMode,
} from '../utils/toolLimits'
import { schemaFields } from '../utils/schemaFields'
import MarkdownBody from './MarkdownBody.vue'

type CatalogItem = ToolInfo | ActionInfo

const tools = ref<ToolInfo[]>([])
const actions = ref<ActionInfo[]>([])
const tab = ref<'tools' | 'actions'>('tools')
const loading = ref(true)
const selected = ref<CatalogItem | null>(null)
const savingGlobal = ref(false)

const globalMode = ref<GlobalToolsMode>('all')
const globalEnabled = ref<Set<string>>(new Set())
const maxParallelTools = ref(3)
const shellConfirm = ref('unknown')

const shellConfirmOptions: PickerOption[] = [
  { value: 'always', label: '每条都问' },
  { value: 'unknown', label: '已知只读的直接放行，其余都问' },
  { value: 'risky', label: '只有命中风险规则才问' },
]

onMounted(load)

async function load(): Promise<void> {
  loading.value = true
  try {
    const [toolList, actionList, config] = await Promise.all([
      client.tools(),
      client.actions(),
      client.config(),
    ])
    tools.value = toolList
    actions.value = actionList
    readGlobalForm(config.runtime)
  } catch {
    toast('读取工具列表失败', 'error')
  } finally {
    loading.value = false
  }
}

function readGlobalForm(runtime: Record<string, unknown>): void {
  const { mode, enabled } = readGlobalToolsMode(runtime)
  globalMode.value = mode
  globalEnabled.value = new Set(enabled)
  const agent = (runtime['agent'] ?? {}) as Record<string, unknown>
  const shell = (runtime['shell'] ?? {}) as Record<string, unknown>
  maxParallelTools.value = Number(agent['max_parallel_tools'] ?? 3)
  shellConfirm.value = String(shell['confirm'] ?? 'unknown')
}

function setGlobalMode(mode: GlobalToolsMode): void {
  globalMode.value = mode
  if (mode === 'custom' && globalEnabled.value.size === 0) {
    globalEnabled.value = new Set(tools.value.map((tool) => tool.name))
  }
}

function isGlobalChecked(name: string): boolean {
  if (globalMode.value === 'all') return true
  if (globalMode.value === 'none') return false
  return globalEnabled.value.has(name)
}

function toggleGlobalTool(name: string, checked: boolean): void {
  if (globalMode.value === 'all') {
    globalMode.value = 'custom'
    globalEnabled.value = new Set(
      tools.value.filter((tool) => tool.name !== name).map((tool) => tool.name),
    )
    if (checked) globalEnabled.value.add(name)
    return
  }
  if (globalMode.value === 'none') {
    globalMode.value = 'custom'
    globalEnabled.value = new Set(checked ? [name] : [])
    return
  }
  const next = new Set(globalEnabled.value)
  if (checked) next.add(name)
  else next.delete(name)
  globalEnabled.value = next
}

async function saveGlobalDefaults(): Promise<void> {
  savingGlobal.value = true
  try {
    const applied = (await client.writeRuntime({
      tools: buildToolsEnabledPayload(globalMode.value, globalEnabled.value),
      agent: { max_parallel_tools: maxParallelTools.value },
      shell: { confirm: shellConfirm.value },
    })) as Record<string, unknown>
    readGlobalForm(applied)
    toast('全局默认已保存', 'success')
  } catch (error) {
    toast(`保存失败：${errMsg(error)}`, 'error')
  } finally {
    savingGlobal.value = false
  }
}

function errMsg(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

const sortedTools = computed(() =>
  [...tools.value].sort((a, b) => a.raw_name.localeCompare(b.raw_name, 'zh-CN')),
)
const sortedActions = computed(() =>
  [...actions.value].sort((a, b) => a.raw_name.localeCompare(b.raw_name, 'zh-CN')),
)

const items = computed((): CatalogItem[] =>
  tab.value === 'tools' ? sortedTools.value : sortedActions.value,
)

const selectedLimits = computed(() =>
  selected.value && isTool(selected.value) ? toolLimits(selected.value.name) : [],
)

watch(
  items,
  (list) => {
    if (!list.length) {
      selected.value = null
      return
    }
    if (!selected.value || !list.some((item) => item.name === selected.value!.name)) {
      selected.value = list[0]!
    }
  },
  { immediate: true },
)

function select(item: CatalogItem): void {
  selected.value = item
}

function isTool(item: CatalogItem): item is ToolInfo {
  return 'permission' in item
}

function globalStatusLabel(name: string): string {
  if (globalMode.value === 'all') return '默认启用'
  if (globalMode.value === 'none') return '默认关闭'
  return globalEnabled.value.has(name) ? '默认启用' : '默认关闭'
}
</script>

<template>
  <div class="split-view">
    <ViewHead title="工具" />

    <div class="split-view__body">
      <aside class="split-view__list">
        <details v-if="tab === 'tools'" class="tools-global panel">
          <summary class="catalog-card__summary tools-global__summary">
            <span class="tools-global__title">全局默认</span>
            <span class="tools-global__hint">新会话</span>
          </summary>
          <div class="tools-global__body">
            <p class="page__hint">
              只影响<strong>新建会话</strong>的初始 tool 列表；当前会话请在聊天侧栏「设置」里改。
            </p>
            <div class="seg-row">
              <button
                class="btn btn--sm"
                :class="{ 'btn--primary': globalMode === 'all' }"
                @click="setGlobalMode('all')"
              >
                全部启用
              </button>
              <button
                class="btn btn--sm"
                :class="{ 'btn--primary': globalMode === 'custom' }"
                @click="setGlobalMode('custom')"
              >
                自定义
              </button>
              <button
                class="btn btn--sm"
                :class="{ 'btn--primary': globalMode === 'none' }"
                @click="setGlobalMode('none')"
              >
                全部关闭
              </button>
            </div>
            <div v-if="globalMode === 'custom'" class="tools-global__checks">
              <label v-for="tool in sortedTools" :key="tool.name" class="tools-global__check">
                <input
                  type="checkbox"
                  :checked="globalEnabled.has(tool.name)"
                  @change="toggleGlobalTool(tool.name, ($event.target as HTMLInputElement).checked)"
                />
                <span>{{ tool.raw_name }}</span>
              </label>
            </div>
            <label class="field">
              <span class="field__label">同条消息并行 tool 上限</span>
              <input
                v-model.number="maxParallelTools"
                class="input"
                type="number"
                min="1"
                max="10"
              />
            </label>
            <label class="field">
              <span class="field__label">bash 命令确认</span>
              <Picker v-model="shellConfirm" :options="shellConfirmOptions" />
            </label>
            <div class="row row--end">
              <button class="btn btn--primary" :disabled="savingGlobal" @click="saveGlobalDefaults">
                {{ savingGlobal ? '保存中…' : '保存全局默认' }}
              </button>
            </div>
          </div>
        </details>

        <div class="split-view__list-tabs">
          <button
            class="btn btn--sm"
            :class="{ 'btn--primary': tab === 'tools' }"
            @click="tab = 'tools'"
          >
            工具 · {{ sortedTools.length }}
          </button>
          <button
            class="btn btn--sm"
            :class="{ 'btn--primary': tab === 'actions' }"
            @click="tab = 'actions'"
          >
            动作 · {{ sortedActions.length }}
          </button>
        </div>

        <p v-if="loading" class="split-view__hint">加载中…</p>
        <div v-else class="split-view__list-scroll">
          <button
            v-for="item in items"
            :key="item.name"
            class="split-view__list-item"
            :class="{ 'split-view__list-item--on': selected?.name === item.name }"
            @click="select(item)"
          >
            <span class="split-view__list-title">{{ item.raw_name }}</span>
            <span class="split-view__list-meta">{{ item.name }}</span>
          </button>
          <p v-if="items.length === 0" class="split-view__hint">暂无条目</p>
        </div>
      </aside>

      <main class="split-view__main">
        <Transition name="lya-split" mode="out-in">
          <div v-if="!selected" key="_empty" class="split-view__empty">选择一条工具或动作</div>
          <div v-else :key="selected.name" class="page__pane">
            <header class="split-view__detail-head">
              <h3>{{ selected.raw_name }}</h3>
              <code class="muted">{{ selected.name }}</code>
            </header>

            <section class="detail-section">
              <h4 class="detail-section__title">说明</h4>
              <p class="prose">{{ selected.description || '—' }}</p>
            </section>

            <div class="seg-row">
              <template v-if="isTool(selected)">
                <span class="pill">权限 {{ selected.permission }}</span>
                <span class="pill">最低模式 · {{ selected.min_mode }}</span>
                <span class="pill">{{ globalStatusLabel(selected.name) }}</span>
              </template>
              <template v-else>
                <span class="pill">
                  {{ selected.flow === 'await_human' ? '需人工确认' : '自动继续' }}
                </span>
                <span class="pill">不可关闭</span>
                <span v-for="mode in selected.visible_in" :key="mode" class="pill">{{ mode }}</span>
              </template>
            </div>

            <p v-if="!isTool(selected)" class="page__hint">
              动作由模型自行调用，不提供开关；可见性仅由工作模式决定。
            </p>

            <section v-if="selectedLimits.length" class="detail-section">
              <h4 class="detail-section__title">内置限制（只读）</h4>
              <table class="schema-table">
                <tbody>
                  <tr v-for="row in selectedLimits" :key="row.label">
                    <th scope="row">{{ row.label }}</th>
                    <td>{{ row.value }}</td>
                  </tr>
                </tbody>
              </table>
            </section>

            <section v-if="selected.prompt_hint" class="detail-section">
              <h4 class="detail-section__title">用法说明</h4>
              <MarkdownBody variant="doc" :text="selected.prompt_hint" />
            </section>

            <section v-if="schemaFields(selected.parameters).length" class="detail-section">
              <h4 class="detail-section__title">参数</h4>
              <table class="schema-table">
                <thead>
                  <tr>
                    <th>名称</th>
                    <th>类型</th>
                    <th>必填</th>
                    <th>说明</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="field in schemaFields(selected.parameters)" :key="field.name">
                    <td><code>{{ field.name }}</code></td>
                    <td>{{ field.type }}</td>
                    <td>{{ field.required ? '是' : '否' }}</td>
                    <td>{{ field.description || '—' }}</td>
                  </tr>
                </tbody>
              </table>
            </section>
          </div>
        </Transition>
      </main>
    </div>
  </div>
</template>

<style scoped>
.tools-global {
  margin: 8px 8px 0;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.tools-global__summary {
  padding: 10px 12px;
}

.tools-global__title {
  font-size: var(--text-sm);
  font-weight: 600;
}

.tools-global__hint {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.tools-global__body {
  padding: 0 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.tools-global__checks {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 220px;
  overflow: auto;
  padding: 8px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
}

.tools-global__check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-sm);
  cursor: pointer;
}

.tools-global__check input {
  width: auto;
}

@media (max-width: 640px) {
  .field {
    grid-template-columns: 1fr;
  }
}
</style>
