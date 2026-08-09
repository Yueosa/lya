<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'

import type { FormAnswerItem } from '../api/client'
import type { HitlBlock } from '../api/wire'
import {
  focusedHitlId,
  pendingHitl,
  pendingHitlBatch,
  pendingHitlId,
  canNavHitlNext,
  canNavHitlPrev,
  canSubmitFocusedHitl,
  hitlFocusBlocksSubmit,
  navigateHitlBatch,
  replyHitl,
} from '../app/useChat'
import Icon from '../ui/Icon.vue'
import { toast } from '../ui/useToast'

const busy = ref(false)
/** 表单作答：题目 id → 选中的值。 */
const picked = reactive<Record<string, string[]>>({})
/** 表单作答：题目 id → 备注。 */
const notes = reactive<Record<string, string>>({})
/** 工具确认备注：按 HITL 消息 id 分别保存，批内切换不丢。 */
const hitlRemarks = reactive<Record<number, string>>({})

const remark = computed({
  get(): string {
    const id = focusedHitlId.value ?? pendingHitlId.value
    return id !== null ? (hitlRemarks[id] ?? '') : ''
  },
  set(value: string) {
    const id = focusedHitlId.value ?? pendingHitlId.value
    if (id !== null) hitlRemarks[id] = value
  },
})

watch(pendingHitlId, (id, prev) => {
  if (prev !== null && prev !== id) delete hitlRemarks[prev]
  busy.value = false
  if (id !== null) {
    void nextTick(() => {
      trayBody.value?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
      trayBody.value
        ?.querySelector<HTMLElement>('button, textarea, input, [tabindex]')
        ?.focus({ preventScroll: true })
    })
  }
})

watch(pendingHitl, (block) => {
  if (block?.type === 'form') {
    for (const key of Object.keys(picked)) delete picked[key]
    for (const key of Object.keys(notes)) delete notes[key]
  }
  if (!block) {
    for (const key of Object.keys(hitlRemarks)) delete hitlRemarks[Number(key)]
    busy.value = false
  }
  void nextTick(growAllTextareas)
})

const block = computed<HitlBlock | null>(() => pendingHitl.value)

const batchNav = computed(() => pendingHitlBatch.value)

const TEXTAREA_MAX = 120
const trayBody = ref<HTMLElement | null>(null)

function growTextarea(el: HTMLTextAreaElement | null): void {
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, TEXTAREA_MAX)}px`
  el.style.overflowY = el.scrollHeight > TEXTAREA_MAX ? 'auto' : 'hidden'
}

function onTextareaInput(event: Event): void {
  growTextarea(event.currentTarget as HTMLTextAreaElement)
}

function growAllTextareas(): void {
  trayBody.value
    ?.querySelectorAll('textarea.tray__textarea')
    .forEach((node) => growTextarea(node as HTMLTextAreaElement))
}

function toggle(questionId: string, key: string, multi: boolean): void {
  const current = picked[questionId] ?? []
  if (!multi) {
    picked[questionId] = current[0] === key ? [] : [key]
    return
  }
  picked[questionId] = current.includes(key)
    ? current.filter((value) => value !== key)
    : [...current, key]
}

function isPicked(questionId: string, key: string): boolean {
  return (picked[questionId] ?? []).includes(key)
}

async function submitForm(): Promise<void> {
  const current = block.value
  if (current?.type !== 'form' || busy.value) return

  const items: FormAnswerItem[] = current.questions
    .map((question) => {
      const values =
        question.kind === 'text'
          ? notes[question.id]
            ? [notes[question.id]!]
            : []
          : (picked[question.id] ?? [])
      const item: FormAnswerItem = { question_id: question.id, values }
      if (question.kind !== 'text' && notes[question.id]) item.note = notes[question.id]!
      return item
    })
    .filter((item) => item.values.length > 0 || item.note)

  if (items.length === 0) {
    toast('请至少回答一题', 'info')
    return
  }

  busy.value = true
  try {
    await replyHitl({
      kind: 'form',
      answer: {
        form_id: current.form_id,
        items,
        ...(remark.value.trim() ? { freetext: remark.value.trim() } : {}),
      },
    })
  } finally {
    busy.value = false
  }
}

async function answerConfirm(approved: boolean): Promise<void> {
  if (busy.value || !canSubmitFocusedHitl.value) return
  busy.value = true
  try {
    await replyHitl({
      kind: 'confirm',
      approved,
      ...(remark.value.trim() ? { note: remark.value.trim() } : {}),
    })
  } finally {
    busy.value = false
  }
}

async function answerMode(approved: boolean): Promise<void> {
  if (busy.value) return
  busy.value = true
  try {
    await replyHitl({ kind: 'mode_change', approved })
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div v-if="block" class="tray panel">
    <div ref="trayBody" class="tray__body">
      <!-- 表单 -->
      <template v-if="block.type === 'form'">
        <h3 class="tray__title">{{ block.title }}</h3>
        <div v-for="question in block.questions" :key="question.id" class="tray__q">
          <p class="tray__prompt">{{ question.text }}</p>

          <textarea
            v-if="question.kind === 'text'"
            v-model="notes[question.id]"
            class="input tray__textarea"
            rows="1"
            placeholder="在这里回答…"
            @input="onTextareaInput"
          />
          <div v-else class="tray__options">
            <button
              v-for="option in question.options ?? []"
              :key="option.key"
              class="btn btn--sm"
              :class="{ 'btn--primary': isPicked(question.id, option.key) }"
              @click="toggle(question.id, option.key, question.kind === 'multi')"
            >
              {{ option.label }}
            </button>
          </div>

          <textarea
            v-if="question.allow_note && question.kind !== 'text'"
            v-model="notes[question.id]"
            class="input tray__textarea tray__note"
            rows="1"
            placeholder="补充说明（可不填）"
            @input="onTextareaInput"
          />
        </div>
      </template>

      <!-- 工具确认 -->
      <template v-else-if="block.type === 'tool_confirm'">
        <div v-if="batchNav" class="tray__batch-nav">
          <button
            type="button"
            class="tray__batch-nav-btn"
            :disabled="!canNavHitlPrev"
            aria-label="上一条待确认工具"
            @click="navigateHitlBatch(-1)"
          >
            <Icon name="chevronLeft" size="sm" />
          </button>
          <span class="tray__batch-nav-label">{{ batchNav.index }} / {{ batchNav.total }}</span>
          <button
            type="button"
            class="tray__batch-nav-btn"
            :disabled="!canNavHitlNext"
            aria-label="下一条待确认工具"
            @click="navigateHitlBatch(1)"
          >
            <Icon name="chevronRight" size="sm" />
          </button>
        </div>
        <h3 class="tray__title">要执行 {{ block.tool_name }} 吗</h3>
        <p class="tray__summary">{{ block.summary }}</p>

        <details
          v-if="block.steps?.length"
          class="tray__fold"
          :open="block.steps.length <= 3"
        >
          <summary class="tray__fold-summary">执行步骤（{{ block.steps.length }}）</summary>
          <ol class="tray__steps">
            <li v-for="(step, at) in block.steps" :key="at">
              <span v-if="step.connector" class="tray__connector">{{ step.connector }}</span>
              <code class="tray__raw">{{ step.raw }}</code>
              <span class="tray__explain">{{ step.explain }}</span>
              <span v-if="step.risk" class="tray__risk"><Icon name="warning" size="sm" /> {{ step.risk }}</span>
            </li>
          </ol>
        </details>

        <details v-if="block.reasons?.length" class="tray__fold" :open="block.reasons.length <= 2">
          <summary class="tray__fold-summary">风险说明（{{ block.reasons.length }}）</summary>
          <ul class="tray__reasons">
            <li v-for="(reason, at) in block.reasons" :key="at">{{ reason }}</li>
          </ul>
        </details>
        <p v-if="hitlFocusBlocksSubmit" class="tray__hint">
          请先回到当前待确认项（◀）再点拒绝或放行。
        </p>
      </template>

      <!-- 模式切换 -->
      <template v-else>
        <h3 class="tray__title">它想切到 {{ block.to_mode }} 模式</h3>
        <p class="tray__summary">{{ block.reason }}</p>
      </template>
    </div>

    <div class="tray__footer">
      <textarea
        v-if="block.type === 'form'"
        v-model="remark"
        class="input tray__textarea"
        rows="1"
        placeholder="还想补充点什么（可不填）"
        @input="onTextareaInput"
      />
      <textarea
        v-else-if="block.type === 'tool_confirm'"
        v-model="remark"
        class="input tray__textarea"
        rows="1"
        placeholder="附一句话给它（可不填）"
        @input="onTextareaInput"
      />

      <div class="tray__actions">
        <template v-if="block.type === 'form'">
          <button class="btn btn--primary" :disabled="busy" @click="submitForm">
            {{ busy ? '提交中…' : '提交' }}
          </button>
        </template>
        <template v-else-if="block.type === 'tool_confirm'">
          <button class="btn" :disabled="busy || !canSubmitFocusedHitl" @click="answerConfirm(false)">拒绝</button>
          <button class="btn btn--danger" :disabled="busy || !canSubmitFocusedHitl" @click="answerConfirm(true)">
            {{ busy ? '执行中…' : '放行' }}
          </button>
        </template>
        <template v-else>
          <button class="btn" :disabled="busy" @click="answerMode(false)">不用</button>
          <button class="btn btn--primary" :disabled="busy" @click="answerMode(true)">同意</button>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tray {
  margin: 0 5% 10px;
  max-height: min(58vh, 420px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-color: var(--accent);
}

.tray__body {
  flex: 1;
  min-height: 0;
  padding: 14px 16px 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
}

.tray__footer {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px 16px 14px;
  border-top: var(--border-width) solid var(--border);
  background: var(--surface);
}

.tray__title {
  margin: 0;
  font-size: var(--text-md);
}

.tray__summary {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.tray__q {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.tray__textarea.input {
  width: 100%;
  min-width: 0;
  height: auto;
  min-height: var(--ctl-h-md);
  padding-top: 8px;
  padding-bottom: 8px;
  line-height: 1.5;
  resize: none;
  overflow-y: hidden;
  box-sizing: border-box;
}

.tray__prompt {
  margin: 0;
  font-size: var(--text-sm);
}

.tray__options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.tray__note {
  margin-top: 2px;
}

.tray__fold {
  margin: 0;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
}

.tray__fold-summary {
  padding: 8px 10px;
  cursor: pointer;
  font-size: var(--text-sm);
  color: var(--text-muted);
  list-style: none;
}

.tray__fold-summary::-webkit-details-marker {
  display: none;
}

.tray__fold[open] .tray__fold-summary {
  border-bottom: var(--border-width) solid var(--border);
}

.tray__steps {
  margin: 0;
  padding: 8px 10px 10px 1.2em;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: var(--text-sm);
  max-height: 220px;
  overflow-y: auto;
}

.tray__steps li {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.tray__connector {
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.tray__raw {
  padding: 3px 6px;
  border-radius: var(--radius-sm);
  background: var(--surface);
  font-family: var(--font-mono);
  word-break: break-all;
  white-space: pre-wrap;
}

.tray__explain {
  color: var(--text-muted);
}

.tray__risk {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--danger);
}

.tray__reasons {
  margin: 0;
  padding: 8px 10px 10px 1.2em;
  color: var(--text-muted);
  font-size: var(--text-sm);
  max-height: 160px;
  overflow-y: auto;
}

.tray__hint {
  margin: 0 0 8px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  background: var(--warning-soft, color-mix(in srgb, var(--warning) 12%, transparent));
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.tray__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.tray__batch-nav {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.tray__batch-nav-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  min-height: 28px;
  padding: 4px 8px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text-muted);
  cursor: pointer;
}

.tray__batch-nav-btn:hover:not(:disabled) {
  color: var(--text);
  border-color: var(--border-strong);
  background: var(--surface-hover);
}

.tray__batch-nav-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.tray__batch-nav-label {
  min-width: 3.5rem;
  text-align: center;
  font-size: var(--text-sm);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
