<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { ApiMode, Mode } from '../../api/wire'
import {
  defaultModel,
  loadModels,
  meta,
  models,
  readOnly,
  setApiMode,
  setIdentity,
  setMode,
  setModel,
  setStyle,
  state,
} from '../../app/useChat'
import Icon from '../../ui/Icon.vue'
import type { IconKey } from '../../ui/icons'
import Picker from '../../ui/Picker.vue'
import type { PickerOption } from '../../ui/Picker.vue'
import { toast } from '../../ui/useToast'

const editingIdentity = ref(false)
const editingStyle = ref(false)
const draftIdentity = ref('')
const draftStyle = ref('')

const MODES: { id: Mode; label: string; icon: IconKey }[] = [
  { id: 'ask', label: '问答', icon: 'modeAsk' },
  { id: 'edit', label: '编辑', icon: 'modeEdit' },
  { id: 'agent', label: '代理', icon: 'modeAgent' },
]

onMounted(() => void loadModels())

watch(
  () => meta.value?.identity,
  () => {
    if (!editingIdentity.value) draftIdentity.value = meta.value?.identity ?? ''
  },
  { immediate: true },
)

watch(
  () => meta.value?.style,
  () => {
    if (!editingStyle.value) draftStyle.value = meta.value?.style ?? ''
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

const identityEmpty = computed(() => !meta.value?.identity?.trim())
const styleEmpty = computed(() => !meta.value?.style?.trim())

function startEditIdentity(): void {
  draftIdentity.value = meta.value?.identity ?? ''
  editingIdentity.value = true
}

function startEditStyle(): void {
  draftStyle.value = meta.value?.style ?? ''
  editingStyle.value = true
}

async function saveIdentity(): Promise<void> {
  if (readOnly.value) return
  const ok = await setIdentity(draftIdentity.value.trim() || null)
  if (!ok) return
  editingIdentity.value = false
  toast('身份已保存', 'success')
}

async function saveStyle(): Promise<void> {
  if (readOnly.value) return
  const ok = await setStyle(draftStyle.value.trim() || null)
  if (!ok) return
  editingStyle.value = false
  toast('口吻已保存', 'success')
}
</script>

<template>
  <div class="session-tab">
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
          <h3 class="session-tab__title">身份</h3>
          <button v-if="!readOnly && !editingIdentity" class="btn btn--sm" @click="startEditIdentity">
            编辑
          </button>
        </div>
        <textarea
          v-if="editingIdentity"
          v-model="draftIdentity"
          class="input session-tab__edit"
          rows="6"
          placeholder="这段对话里你是谁"
        />
        <pre v-else-if="!identityEmpty" class="session-tab__pre">{{ meta?.identity }}</pre>
        <p v-else class="session-tab__lead">（未设置身份）</p>
        <div v-if="editingIdentity" class="session-tab__actions">
          <button class="btn btn--sm btn--primary" @click="saveIdentity">保存</button>
          <button class="btn btn--sm" @click="editingIdentity = false">取消</button>
        </div>
        <p v-if="!editingIdentity" class="session-tab__meta">
          只属于这段对话。改「提示词 → 身份」不会影响它，那份只用在新建的会话上。
        </p>
      </section>

      <section class="session-tab__section">
        <div class="session-tab__head-row">
          <h3 class="session-tab__title">口吻</h3>
          <button v-if="!readOnly && !editingStyle" class="btn btn--sm" @click="startEditStyle">
            编辑
          </button>
        </div>
        <textarea
          v-if="editingStyle"
          v-model="draftStyle"
          class="input session-tab__edit"
          rows="8"
          placeholder="口癖、few-shot、游戏原句参考"
        />
        <pre v-else-if="!styleEmpty" class="session-tab__pre">{{ meta?.style }}</pre>
        <p v-else class="session-tab__lead">（未设置口吻）</p>
        <div v-if="editingStyle" class="session-tab__actions">
          <button class="btn btn--sm btn--primary" @click="saveStyle">保存</button>
          <button class="btn btn--sm" @click="editingStyle = false">取消</button>
        </div>
      </section>
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
}

.session-tab__meta {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.session-tab__pre {
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: var(--leading);
  white-space: pre-wrap;
  word-break: break-word;
}

.session-tab__edit {
  width: 100%;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: var(--leading);
  resize: vertical;
}

.session-tab__actions {
  display: flex;
  gap: 8px;
}
</style>
