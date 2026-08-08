<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { errorText, type ModelInfo, type ProbeResult } from '../api/client'
import type { ApiMode } from '../api/wire'
import { client, loadModels, models } from '../app/useChat'
import ViewHead from '../ui/ViewHead.vue'
import { toast } from '../ui/useToast'
import { groupBy } from '../utils/groupBy'

interface GatewayGroup {
  baseUrl: string
  models: ModelInfo[]
}

const loading = ref(true)
const selectedUrl = ref<string | null>(null)
const probingUrl = ref<string | null>(null)
const probeByUrl = ref<Map<string, ProbeResult>>(new Map())

onMounted(async () => {
  try {
    await loadModels()
  } catch {
    toast('读取模型失败', 'error')
  } finally {
    loading.value = false
  }
})

const modelGroups = computed((): GatewayGroup[] =>
  groupBy(models.value, (m) => m.base_url).map(([baseUrl, items]) => ({
    baseUrl,
    models: items,
  })),
)

const selected = computed(() =>
  modelGroups.value.find((group) => group.baseUrl === selectedUrl.value) ?? null,
)

watch(
  modelGroups,
  (groups) => {
    if (!groups.length) {
      selectedUrl.value = null
      return
    }
    if (!selectedUrl.value || !groups.some((group) => group.baseUrl === selectedUrl.value)) {
      selectedUrl.value = groups[0]!.baseUrl
    }
  },
  { immediate: true },
)

function gatewayLabel(url: string): string {
  try {
    const parsed = new URL(url)
    const path = parsed.pathname.replace(/\/$/, '')
    return path && path !== '/' ? `${parsed.host}${path}` : parsed.host
  } catch {
    return url.length > 36 ? `${url.slice(0, 33)}…` : url
  }
}

function modeCaps(model: ModelInfo, mode: ApiMode): string {
  return model.modes[mode]?.capabilities.join(', ') ?? '—'
}

function modeStackHint(model: ModelInfo, mode: ApiMode): string {
  if (model.modes[mode]) return modeCaps(model, mode)
  return mode === 'responses' ? '未配置 modes.responses' : '未配置 modes.completions'
}

function formatContext(value: number | null | undefined): string {
  if (value == null) return '—'
  if (value >= 1_048_576 && value % 1_048_576 === 0) return `${value / 1_048_576}M`
  if (value >= 1024 && value % 1024 === 0) return `${value / 1024}K`
  return String(value)
}

function probeModelsText(baseUrl: string): string {
  const result = probeByUrl.value.get(baseUrl)
  if (!result?.ok || !result.models.length) return ''
  return result.models.join('\n')
}


async function probeGroup(baseUrl: string, items: ModelInfo[]): Promise<void> {
  const target = items.find((item) => !item.api_key_placeholder) ?? items[0]
  if (!target) return
  probingUrl.value = baseUrl
  try {
    const result = await client.probeModel(target.id)
    probeByUrl.value = new Map(probeByUrl.value).set(baseUrl, result)
  } catch (error) {
    probeByUrl.value = new Map(probeByUrl.value).set(baseUrl, {
      ok: false,
      models: [],
      error: errorText(error),
    })
  } finally {
    probingUrl.value = null
  }
}

function selectGroup(baseUrl: string): void {
  selectedUrl.value = baseUrl
}
</script>

<template>
  <div class="split-view">
    <ViewHead title="模型" />

    <div class="split-view__body">
      <aside class="split-view__list">
        <p v-if="loading" class="split-view__hint">加载中…</p>
        <div v-else class="split-view__list-scroll">
          <button
            v-for="group in modelGroups"
            :key="group.baseUrl"
            class="split-view__list-item"
            :class="{ 'split-view__list-item--on': selectedUrl === group.baseUrl }"
            @click="selectGroup(group.baseUrl)"
          >
            <span class="split-view__list-title">{{ gatewayLabel(group.baseUrl) }}</span>
            <span class="split-view__list-meta">{{ group.models.length }} 个 · {{ group.baseUrl }}</span>
          </button>
          <p v-if="modelGroups.length === 0" class="split-view__hint">暂无模型，请检查 models.toml</p>
        </div>
      </aside>

      <main class="split-view__main">
        <Transition name="lya-split" mode="out-in">
          <div v-if="!selected" key="_empty" class="split-view__empty">选择一个网关</div>
          <div v-else :key="selected.baseUrl" class="page__pane">
          <header class="split-view__detail-head">
            <h3>{{ gatewayLabel(selected.baseUrl) }}</h3>
          </header>

          <div class="panel form-panel">
            <div class="row">
              <span class="gateway-card__label">网关地址</span>
              <code class="mono mono--strong">{{ selected.baseUrl }}</code>
              <span class="row__grow" />
              <span class="pill">{{ selected.models.length }} 个模型</span>
              <button
                class="btn btn--sm"
                :disabled="probingUrl === selected.baseUrl"
                @click="probeGroup(selected.baseUrl, selected.models)"
              >
                {{ probingUrl === selected.baseUrl ? '探测中…' : '探测 /models' }}
              </button>
            </div>
          </div>

          <div class="panel gateway-card">
            <table class="models-table">
              <thead>
                <tr>
                  <th>显示名</th>
                  <th>配置 ID</th>
                  <th>context</th>
                  <th>密钥</th>
                  <th title="chat/completions 栈；DuckDuckGo web_search 可用">Completions</th>
                  <th title="POST /responses；原生 web_search；无 modes.responses 则不可用于 Responses 会话">Responses</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="model in selected.models" :key="model.id">
                  <td><strong>{{ model.name }}</strong></td>
                  <td><code class="mono mono--strong">{{ model.id }}</code></td>
                  <td><code class="mono">{{ formatContext(model.context_window) }}</code></td>
                  <td>
                    <span v-if="model.api_key_placeholder" class="pill pill--bad">未配置</span>
                    <span v-else class="pill pill--key">{{ model.api_key_masked }}</span>
                  </td>
                  <td>
                    <span v-if="!model.modes.completions" class="muted" :title="modeStackHint(model, 'completions')">—</span>
                    <span v-else class="pill">{{ modeCaps(model, 'completions') }}</span>
                  </td>
                  <td>
                    <span v-if="!model.modes.responses" class="muted" :title="modeStackHint(model, 'responses')">—</span>
                    <span v-else class="pill">{{ modeCaps(model, 'responses') }}</span>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <section v-if="probeByUrl.get(selected.baseUrl)?.ok" class="detail-section">
            <h4 class="detail-section__title">
              远端 /models · {{ probeByUrl.get(selected.baseUrl)!.models.length }} 个
            </h4>
            <pre class="pre-block pre-block--compact">{{ probeModelsText(selected.baseUrl) }}</pre>
          </section>
          <p
            v-else-if="probeByUrl.get(selected.baseUrl) && !probeByUrl.get(selected.baseUrl)!.ok"
            class="page__error"
          >
            {{ probeByUrl.get(selected.baseUrl)!.error }}
          </p>
          </div>
        </Transition>
      </main>
    </div>
  </div>
</template>
