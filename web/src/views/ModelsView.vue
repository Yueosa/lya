<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { ModelInfo, ProbeResult } from '../api/client'
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

function modelParam(model: ModelInfo, key: string): string | null {
  const value = model.params?.[key]
  if (value === undefined || value === null) return null
  return typeof value === 'string' ? value : JSON.stringify(value)
}

function probeModelsText(baseUrl: string): string {
  const result = probeByUrl.value.get(baseUrl)
  if (!result?.ok || !result.models.length) return ''
  return result.models.join('\n')
}

function errMsg(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
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
      error: errMsg(error),
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

          <p class="page__hint">
            来自 <code>models.toml</code>。对话里选「显示名」；发请求用的是「API model」列。
            其余字段（如 <code>max_tokens</code>、<code>thinking</code>）原样透传进请求体，在原始文件里编辑即可。
          </p>

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
                  <th>API model</th>
                  <th>max_tokens</th>
                  <th>密钥</th>
                  <th>能力</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="model in selected.models" :key="model.id">
                  <td><strong>{{ model.name }}</strong></td>
                  <td><code class="mono mono--strong">{{ model.id }}</code></td>
                  <td><code class="mono">{{ modelParam(model, 'model') || '—' }}</code></td>
                  <td><code class="mono">{{ modelParam(model, 'max_tokens') || '—' }}</code></td>
                  <td>
                    <span v-if="model.api_key_placeholder" class="pill pill--bad">未配置</span>
                    <span v-else class="pill pill--key">{{ model.api_key_masked }}</span>
                  </td>
                  <td>
                    <span v-if="!model.capabilities.length" class="muted">—</span>
                    <span v-for="cap in model.capabilities" :key="cap" class="pill">{{ cap }}</span>
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
