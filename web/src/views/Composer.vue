<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import type { Mode } from '../api/wire'
import {
  canSend,
  defaultModel,
  loadModels,
  meta,
  models,
  pendingHitl,
  running,
  send,
  setMode,
  setModel,
  stop,
} from '../app/useChat'
import Icon from '../ui/Icon.vue'
import type { IconKey } from '../ui/icons'
import HitlTray from './HitlTray.vue'

const draft = ref('')
const input = ref<HTMLTextAreaElement | null>(null)
const modelOpen = ref(false)
const modelWrap = ref<HTMLElement | null>(null)

const MAX_HEIGHT = 150

const MODES: { id: Mode; label: string; icon: IconKey }[] = [
  { id: 'ask', label: '问答', icon: 'modeAsk' },
  { id: 'edit', label: '编辑', icon: 'modeEdit' },
  { id: 'agent', label: '代理', icon: 'modeAgent' },
]

const mode = computed(() => meta.value?.work_mode ?? 'agent')

const modelLabel = computed(() => {
  const id = meta.value?.model_id
  if (id) return models.value.find((m) => m.id === id)?.name ?? id
  return defaultModel.value?.name ?? '默认模型'
})

const blocked = computed(() => pendingHitl.value !== null)

const placeholder = computed(() =>
  blocked.value ? '先答复 HITL' : '输入消息…（Enter 发送，Shift+Enter 换行）',
)

function grow(): void {
  const el = input.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, MAX_HEIGHT)}px`
  el.style.overflowY = el.scrollHeight > MAX_HEIGHT ? 'auto' : 'hidden'
}

watch(draft, () => void nextTick(grow))

async function submit(): Promise<void> {
  const text = draft.value
  if (!text.trim() || !canSend.value) return
  draft.value = ''
  await nextTick()
  grow()
  await send(text)
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void submit()
  }
}

async function pickModel(id: string | null): Promise<void> {
  modelOpen.value = false
  await setModel(id)
}

function onDocClick(event: MouseEvent): void {
  if (!modelOpen.value) return
  if (modelWrap.value && !modelWrap.value.contains(event.target as Node)) modelOpen.value = false
}

onMounted(() => {
  void loadModels()
  document.addEventListener('click', onDocClick)
})
onUnmounted(() => document.removeEventListener('click', onDocClick))
</script>

<template>
  <div class="composer">
    <HitlTray />

    <!-- |模型|输入|模式|发送| -->
    <div class="composer__row">
      <div ref="modelWrap" class="composer__model">
        <button
          class="btn composer__model-btn"
          :aria-expanded="modelOpen"
          v-tip="'选择模型'"
          @click.stop="modelOpen = !modelOpen"
        >
          <span class="composer__model-label">{{ modelLabel }}</span>
          <Icon class="composer__caret" name="chevronDown" size="sm" />
        </button>
        <div v-if="modelOpen" class="composer__menu panel">
          <button
            v-for="model in models"
            :key="model.id"
            class="composer__option"
            :class="{ 'composer__option--on': model.id === meta?.model_id }"
            :disabled="model.api_key_placeholder"
            @click="pickModel(model.id)"
          >
            {{ model.name }}
            <span v-if="model.api_key_placeholder" class="composer__warn">未配密钥</span>
          </button>
          <p v-if="models.length === 0" class="composer__empty">无可用模型，请检查设置</p>
        </div>
      </div>

      <textarea
        ref="input"
        v-model="draft"
        class="composer__input"
        rows="1"
        :placeholder="placeholder"
        :disabled="blocked"
        @keydown="onKeydown"
        @input="grow"
      />

      <div class="seg" role="tablist">
        <button
          v-for="item in MODES"
          :key="item.id"
          class="seg__btn"
          :class="[`seg__btn--${item.id}`, { 'seg__btn--on': mode === item.id }]"
          @click="setMode(item.id)"
        >
          <Icon class="seg__icon" :name="item.icon" size="sm" />
          <span>{{ item.label }}</span>
        </button>
      </div>

      <button v-if="running" class="btn composer__action composer__action--stop" @click="stop">
        停止
      </button>
      <button
        v-else
        class="btn btn--primary composer__action"
        :disabled="!canSend || !draft.trim()"
        @click="submit"
      >
        发送
      </button>
    </div>
  </div>
</template>

<style scoped>
.composer {
  flex-shrink: 0;
  padding: 10px 20px 16px;
}

.composer__row {
  max-width: 1320px;
  margin: 0 auto;
  display: flex;
  align-items: flex-end;
  gap: 10px;
}

.composer__model {
  position: relative;
  flex-shrink: 0;
}

.composer__model-btn {
  min-width: 88px;
  max-width: 200px;
  height: 44px;
  gap: 6px;
}

.composer__model-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.composer__caret {
  flex-shrink: 0;
  color: var(--text-faint);
  transition: transform 0.15s ease;
}

.composer__model-btn[aria-expanded='true'] .composer__caret {
  transform: rotate(180deg);
}

.composer__menu {
  position: absolute;
  left: 0;
  bottom: calc(100% + 6px);
  z-index: 30;
  min-width: 220px;
  max-height: 280px;
  overflow-y: auto;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.composer__option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
}

.composer__option:hover:not(:disabled) {
  background: var(--surface-hover);
}

.composer__option--on {
  background: var(--accent-soft);
}

.composer__option:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.composer__warn {
  color: var(--danger);
  font-size: var(--text-xs);
}

.composer__empty {
  margin: 0;
  padding: 8px;
  color: var(--text-muted);
  font-size: var(--text-xs);
}

.composer__input {
  flex: 1 1 360px;
  min-width: 180px;
  min-height: 44px;
  padding: 10px 18px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-pill);
  background: var(--bg-sunken);
  color: var(--text);
  font: inherit;
  font-size: var(--text-md);
  line-height: 1.5;
  box-sizing: border-box;
  resize: none;
  outline: none;
  max-height: 150px;
}

.composer__input:focus {
  border-color: var(--border-strong);
  box-shadow: var(--shadow-focus);
}

.composer__action {
  flex-shrink: 0;
  min-width: 56px;
  height: 44px;
  padding: 0 14px;
}

.composer__action--stop {
  border-color: var(--danger);
  color: var(--danger);
}

.composer__action--stop:hover {
  background: var(--danger-soft);
}

@media (max-width: 720px) {
  .composer {
    padding: 8px 12px 12px;
  }

  .composer__model-btn {
    max-width: 120px;
  }
}
</style>
