<!--
  需要你决定时，从输入框上方滑出的托盘。

  **不用模态框**是有原因的：确认一条命令该不该跑，你往往需要看着上文——它为什么
  要跑这个、之前说了什么。模态框恰好把那些挡住。托盘只占底部一条，上面的对话
  照样能滚、能读。

  三种打断共用后端同一个端点，这里也共用一个组件：它们的语义是一样的——「结清
  当前挂起，让这一轮接着跑」。
-->

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'

import type { FormAnswerItem } from '../api/client'
import type { HitlBlock } from '../api/wire'
import { pendingHitl, pendingHitlBatch, replyHitl } from '../app/useChat'
import Icon from '../ui/Icon.vue'

const busy = ref(false)
/** 表单作答：题目 id → 选中的值。 */
const picked = reactive<Record<string, string[]>>({})
/** 表单作答：题目 id → 备注。 */
const notes = reactive<Record<string, string>>({})
/** 表单级补充说明，以及工具确认的备注。 */
const remark = ref('')

// 换了一个新的挂起就把上一次的作答清掉，免得串台
watch(pendingHitl, () => {
  for (const key of Object.keys(picked)) delete picked[key]
  for (const key of Object.keys(notes)) delete notes[key]
  remark.value = ''
  busy.value = false
})

const block = computed<HitlBlock | null>(() => pendingHitl.value)

const batchNav = computed(() => pendingHitlBatch.value)

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
  busy.value = true

  const items: FormAnswerItem[] = current.questions
    .map((question) => {
      const values =
        question.kind === 'text'
          ? notes[question.id]
            ? [notes[question.id]!]
            : []
          : (picked[question.id] ?? [])
      const item: FormAnswerItem = { question_id: question.id, values }
      // 文本题的输入就是答案本身，别再当成备注重复一遍
      if (question.kind !== 'text' && notes[question.id]) item.note = notes[question.id]!
      return item
    })
    // 没作答的题直接不出现，后端允许
    .filter((item) => item.values.length > 0 || item.note)

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
  if (busy.value) return
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
    <!-- 表单 -->
    <template v-if="block.type === 'form'">
      <h3 class="tray__title">{{ block.title }}</h3>
      <div v-for="question in block.questions" :key="question.id" class="tray__q">
        <p class="tray__prompt">{{ question.text }}</p>

        <input
          v-if="question.kind === 'text'"
          v-model="notes[question.id]"
          class="input"
          placeholder="在这里回答…"
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

        <input
          v-if="question.allow_note && question.kind !== 'text'"
          v-model="notes[question.id]"
          class="input tray__note"
          placeholder="补充说明（可不填）"
        />
      </div>

      <input v-model="remark" class="input" placeholder="还想补充点什么（可不填）" />
      <div class="tray__actions">
        <button class="btn btn--primary" :disabled="busy" @click="submitForm">
          {{ busy ? '提交中…' : '提交' }}
        </button>
      </div>
    </template>

    <!-- 工具确认 -->
    <template v-else-if="block.type === 'tool_confirm'">
      <div v-if="batchNav" class="tray__batch-nav">
        <span class="tray__batch-nav-label">‹ {{ batchNav.index }} / {{ batchNav.total }} ›</span>
      </div>
      <h3 class="tray__title">要执行 {{ block.tool_name }} 吗</h3>
      <p class="tray__summary">{{ block.summary }}</p>

      <!-- 逐段拆开：一长串 && 和 | 串起来的命令原样丢给人看，等于让人闭眼签字 -->
      <ol v-if="block.steps?.length" class="tray__steps">
        <li v-for="(step, at) in block.steps" :key="at">
          <span v-if="step.connector" class="tray__connector">{{ step.connector }}</span>
          <code class="tray__raw">{{ step.raw }}</code>
          <span class="tray__explain">{{ step.explain }}</span>
          <span v-if="step.risk" class="tray__risk"><Icon name="warning" size="sm" /> {{ step.risk }}</span>
        </li>
      </ol>

      <ul v-if="block.reasons?.length" class="tray__reasons">
        <li v-for="(reason, at) in block.reasons" :key="at">{{ reason }}</li>
      </ul>

      <input v-model="remark" class="input" placeholder="附一句话给它（可不填）" />
      <div class="tray__actions">
        <button class="btn" :disabled="busy" @click="answerConfirm(false)">拒绝</button>
        <button class="btn btn--danger" :disabled="busy" @click="answerConfirm(true)">
          {{ busy ? '执行中…' : '放行' }}
        </button>
      </div>
    </template>

    <!-- 模式切换 -->
    <template v-else>
      <h3 class="tray__title">它想切到 {{ block.to_mode }} 模式</h3>
      <p class="tray__summary">{{ block.reason }}</p>
      <div class="tray__actions">
        <button class="btn" :disabled="busy" @click="answerMode(false)">不用</button>
        <button class="btn btn--primary" :disabled="busy" @click="answerMode(true)">同意</button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.tray {
  margin: 0 5% 10px;
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  /* 强调色描边，一眼看出这里在等你 */
  border-color: var(--accent);
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

.tray__steps {
  margin: 0;
  padding-left: 1.2em;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: var(--text-sm);
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
  background: var(--bg-sunken);
  font-family: var(--font-mono);
  word-break: break-all;
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
  padding-left: 1.2em;
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
  justify-content: center;
}

.tray__batch-nav-label {
  font-size: var(--text-sm);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
