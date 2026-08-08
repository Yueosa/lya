<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { ApiMode, Mode } from '../../api/wire'
import {
  client,
  defaultModel,
  loadModels,
  meta,
  models,
  readOnly,
  setApiMode,
  setMode,
  setModel,
  setPersona,
  state,
} from '../../app/useChat'
import Icon from '../../ui/Icon.vue'
import type { IconKey } from '../../ui/icons'
import Picker from '../../ui/Picker.vue'
import type { PickerOption } from '../../ui/Picker.vue'
import { toast } from '../../ui/useToast'

const globalPersona = ref<string | null>(null)
const loading = ref(true)
const editingPersona = ref(false)
const draftPersona = ref('')

const MODES: { id: Mode; label: string; icon: IconKey }[] = [
  { id: 'ask', label: '问答', icon: 'modeAsk' },
  { id: 'edit', label: '编辑', icon: 'modeEdit' },
  { id: 'agent', label: '代理', icon: 'modeAgent' },
]

onMounted(async () => {
  loading.value = true
  void loadModels()
  try {
    const cfg = await client.config()
    globalPersona.value = cfg.persona ?? null
  } catch {
    globalPersona.value = null
  } finally {
    loading.value = false
  }
})

watch(
  () => meta.value?.persona,
  () => {
    if (!editingPersona.value) draftPersona.value = meta.value?.persona ?? ''
  },
  { immediate: true },
)

const API_MODES: { id: ApiMode; label: string }[] = [
  { id: 'completions', label: 'Completions' },
  { id: 'responses', label: 'Responses' },
]

const mode = computed(() => meta.value?.work_mode ?? 'agent')
const apiMode = computed(() => meta.value?.api_mode ?? 'completions')
const canEditApiMode = computed(
  () => !readOnly.value && state.value.messages.length === 0,
)

const modelOptions = computed((): PickerOption[] => {
  const stack = apiMode.value
  const opts: PickerOption[] = [
    { value: '', label: `默认（${defaultModel.value?.name ?? '配置默认'}）` },
  ]
  for (const model of models.value) {
    if (!model.modes[stack]) continue
    opts.push({
      value: model.id,
      label: model.api_key_placeholder ? `${model.name}（未配密钥）` : model.name,
      disabled: model.api_key_placeholder,
    })
  }
  return opts
})

const modelPicker = computed({
  get: () => meta.value?.model_id ?? '',
  set: (value: string) => {
    if (readOnly.value) return
    void setModel(value || null)
  },
})

const modelLabel = computed(() => {
  const id = meta.value?.model_id
  if (id) return models.value.find((m) => m.id === id)?.name ?? id
  return defaultModel.value?.name ?? '默认模型'
})

const personaSource = computed(() => {
  if (meta.value?.persona) return '会话'
  if (globalPersona.value) return '全局'
  return '内置默认'
})

const effectivePersona = computed(() => {
  if (meta.value?.persona) return meta.value.persona
  if (globalPersona.value) return globalPersona.value
  return '（未单独配置，使用 lya 内置默认人设）'
})

function startEditPersona(): void {
  // 当前跟着全局走时，把全局那份正文预填进去。这样「编辑 → 保存」一次点击就等于
  // 把此刻的人设钉死在这个会话上，之后再改全局也不会牵动它——想要「这段对话里她
  // 不会变人」的人，不必自己去别处把正文抄过来。
  // 内置默认没有正文可填，留空即可：留空本来就是「跟随」的意思。
  draftPersona.value = meta.value?.persona ?? globalPersona.value ?? ''
  editingPersona.value = true
}

async function savePersona(): Promise<void> {
  if (readOnly.value) return
  const text = draftPersona.value.trim()
  const ok = await setPersona(text || null)
  if (!ok) return
  editingPersona.value = false
  toast('人设已保存', 'success')
}
</script>

<template>
  <div class="session-tab">
    <p v-if="loading" class="session-tab__hint">加载中…</p>
    <template v-else>
      <section class="session-tab__section">
        <h3 class="session-tab__title">工作模式</h3>
        <div class="seg" role="tablist">
          <button
            v-for="item in MODES"
            :key="item.id"
            class="seg__btn"
            :class="[`seg__btn--${item.id}`, { 'seg__btn--on': mode === item.id }]"
            :disabled="readOnly"
            @click="setMode(item.id)"
          >
            <Icon class="seg__icon" :name="item.icon" size="sm" />
            <span>{{ item.label }}</span>
          </button>
        </div>
      </section>

      <section class="session-tab__section">
        <h3 class="session-tab__title">API 栈</h3>
        <div v-if="canEditApiMode" class="seg" role="tablist">
          <button
            v-for="item in API_MODES"
            :key="item.id"
            class="seg__btn"
            :class="{ 'seg__btn--on': apiMode === item.id }"
            @click="setApiMode(item.id)"
          >
            <span>{{ item.label }}</span>
          </button>
        </div>
        <p v-if="canEditApiMode" class="session-tab__meta">发出第一条消息后锁定。</p>
        <template v-else>
          <p class="session-tab__lead">
            {{ API_MODES.find((item) => item.id === apiMode)?.label ?? apiMode }}
          </p>
          <p class="session-tab__meta">已有消息后锁定；要换栈请新建会话。</p>
        </template>
      </section>

      <section class="session-tab__section">
        <h3 class="session-tab__title">模型</h3>
        <p v-if="readOnly" class="session-tab__lead">{{ modelLabel }}</p>
        <Picker v-else v-model="modelPicker" :options="modelOptions" />
      </section>

      <section class="session-tab__section">
        <div class="session-tab__head-row">
          <h3 class="session-tab__title">人设 · {{ personaSource }}</h3>
          <button v-if="!readOnly && !editingPersona" class="btn btn--sm" @click="startEditPersona">
            编辑
          </button>
        </div>
        <textarea
          v-if="editingPersona"
          v-model="draftPersona"
          class="input session-tab__edit"
          rows="6"
          placeholder="留空则使用全局/默认人设"
        />
        <pre v-else class="session-tab__pre">{{ effectivePersona }}</pre>
        <div v-if="editingPersona" class="session-tab__actions">
          <button class="btn btn--sm btn--primary" @click="savePersona">保存</button>
          <button class="btn btn--sm" @click="editingPersona = false">取消</button>
        </div>
        <p v-if="!editingPersona && personaSource !== '会话'" class="session-tab__meta">
          当前未设置会话专属人设，生效的是{{ personaSource === '全局' ? '全局配置' : '内置默认' }}。
        </p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.session-tab {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.session-tab__hint {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.session-tab__section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.session-tab__head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.session-tab__title {
  margin: 0;
  font-size: var(--text-md);
  font-weight: 600;
}

.session-tab__lead {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-muted);
  line-height: var(--leading);
}

.session-tab__pre {
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 240px;
  overflow: auto;
}

.session-tab__edit {
  height: auto;
  padding: 8px 12px;
  line-height: var(--leading);
  resize: vertical;
}

.session-tab__actions {
  display: flex;
  gap: 8px;
}

.session-tab__meta {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--text-faint);
}
</style>
