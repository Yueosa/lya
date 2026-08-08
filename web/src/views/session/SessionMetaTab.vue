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

const editingPersona = ref(false)
const draftPersona = ref('')

const MODES: { id: Mode; label: string; icon: IconKey }[] = [
  { id: 'ask', label: '问答', icon: 'modeAsk' },
  { id: 'edit', label: '编辑', icon: 'modeEdit' },
  { id: 'agent', label: '代理', icon: 'modeAgent' },
]

// 不用再读全局配置了：人设是会话级的，这一屏要显示的东西全在 meta 里
onMounted(() => void loadModels())

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

/*
  人设是这个会话自己的一份。

  「设置」里那份叫**默认人设**，作用只是新会话建起来时从它抄一份进来——不是所有会话每轮
  都去读它。反过来做的话，改一次默认人设会把每段正在进行的对话都换掉性格，而上面几十条
  聊天记录还是旧性格写的，模型下一轮得同时扮演两个人。所以这一屏不再有「跟随全局」这个
  状态，也不需要去读全局配置。

  留空是明确的一种选择：这个会话不要人设段。
*/
const personaEmpty = computed(() => !meta.value?.persona?.trim())

function startEditPersona(): void {
  draftPersona.value = meta.value?.persona ?? ''
  editingPersona.value = true
}

async function savePersona(): Promise<void> {
  if (readOnly.value) return
  // 传空串而不是 null：null 在后端是「回退到默认人设」的意思，那正是要避免的
  const ok = await setPersona(draftPersona.value.trim())
  if (!ok) return
  editingPersona.value = false
  toast('人设已保存', 'success')
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
          <h3 class="session-tab__title">人设</h3>
          <button v-if="!readOnly && !editingPersona" class="btn btn--sm" @click="startEditPersona">
            编辑
          </button>
        </div>
        <textarea
          v-if="editingPersona"
          v-model="draftPersona"
          class="input session-tab__edit"
          rows="6"
          placeholder="留空则这段对话不带人设"
        />
        <pre v-else-if="!personaEmpty" class="session-tab__pre">{{ meta?.persona }}</pre>
        <p v-else class="session-tab__lead">（这段对话不带人设）</p>
        <div v-if="editingPersona" class="session-tab__actions">
          <button class="btn btn--sm btn--primary" @click="savePersona">保存</button>
          <button class="btn btn--sm" @click="editingPersona = false">取消</button>
        </div>
        <!-- 说清这份是这段对话自己的：改「设置 → 人设」不会动它，那边只管新会话 -->
        <p v-if="!editingPersona" class="session-tab__meta">
          只属于这段对话。改默认人设不会影响它，那份只用在新建的会话上。
        </p>
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
