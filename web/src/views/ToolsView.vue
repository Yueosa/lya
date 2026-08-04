<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { ActionInfo, ToolInfo } from '../api/client'
import { client } from '../app/useChat'
import ViewHead from '../ui/ViewHead.vue'
import { toast } from '../ui/useToast'
import { readGlobalToolsMode, toolLimits } from '../utils/toolLimits'
import { schemaFields } from '../utils/schemaFields'
import MarkdownBody from './MarkdownBody.vue'

type CatalogItem = ToolInfo | ActionInfo

const tools = ref<ToolInfo[]>([])
const actions = ref<ActionInfo[]>([])
const tab = ref<'tools' | 'actions'>('tools')
const loading = ref(true)
const selected = ref<CatalogItem | null>(null)

const globalMode = ref<'all' | 'none' | 'custom'>('all')
const globalEnabled = ref<Set<string>>(new Set())

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
    const { mode, enabled } = readGlobalToolsMode(config.runtime)
    globalMode.value = mode
    globalEnabled.value = new Set(enabled)
  } catch {
    toast('读取工具列表失败', 'error')
  } finally {
    loading.value = false
  }
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

            <section v-if="selectedLimits.length" class="detail-section">
              <h4 class="detail-section__title">内置限制与栈行为</h4>
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
