<!--
  历史里已经答复过的打断。

  只读回显，靠的是后端存在 `lya.meta.answer` 里的**原始作答**——不是从渲染给模型
  看的那段中文里反解出来的。当初特意加那个字段就是为了这一刻：回看时能看到你当时
  勾了哪几个选项，而不是一句「用户选择了：甲、乙」。
-->

<script setup lang="ts">
import { computed } from 'vue'

import type { FormAnswer } from '../api/client'
import type { HitlBlock } from '../api/wire'

const props = defineProps<{ hitl: HitlBlock; answer?: unknown }>()

/** 表单作答；形状对不上就当没有，别让一条脏数据把整条消息渲染崩掉。 */
const formAnswer = computed<FormAnswer | null>(() => {
  const answer = props.answer
  if (props.hitl.type !== 'form' || !answer || typeof answer !== 'object') return null
  return 'items' in answer ? (answer as FormAnswer) : null
})

/** 确认与模式切换的答复只有「同意与否」加一句备注。 */
const decision = computed<{ approved: boolean; note?: string } | null>(() => {
  const answer = props.answer
  if (!answer || typeof answer !== 'object' || !('approved' in answer)) return null
  return answer as { approved: boolean; note?: string }
})

/** 把选中的 key 换回用户当时看到的文案。 */
function labelsOf(questionId: string, values: string[]): string {
  if (props.hitl.type !== 'form') return values.join('、')
  const question = props.hitl.questions.find((item) => item.id === questionId)
  if (!question) return values.join('、')
  if (question.kind === 'text') return values.join('')
  return values
    .map((value) => question.options?.find((option) => option.key === value)?.label ?? value)
    .join('、')
}

function promptOf(questionId: string): string {
  if (props.hitl.type !== 'form') return questionId
  return props.hitl.questions.find((item) => item.id === questionId)?.text ?? questionId
}

const title = computed(() => {
  switch (props.hitl.type) {
    case 'form':
      return props.hitl.title
    case 'tool_confirm':
      return `${props.hitl.tool_name} 的执行确认`
    case 'mode_change':
      return `切换到 ${props.hitl.to_mode} 模式`
  }
})
</script>

<template>
  <div class="record">
    <div class="record__head">
      <span>✋</span>
      <span class="record__title">{{ title }}</span>
      <span
        v-if="decision"
        class="record__verdict"
        :class="decision.approved ? 'record__verdict--yes' : 'record__verdict--no'"
      >
        {{ decision.approved ? '已同意' : '已拒绝' }}
      </span>
    </div>

    <dl v-if="formAnswer" class="record__answers">
      <template v-for="item in formAnswer.items" :key="item.question_id">
        <dt>{{ promptOf(item.question_id) }}</dt>
        <dd>
          {{ labelsOf(item.question_id, item.values) }}
          <span v-if="item.note" class="record__note">（{{ item.note }}）</span>
        </dd>
      </template>
    </dl>
    <p v-if="formAnswer?.freetext" class="record__note">{{ formAnswer.freetext }}</p>
    <p v-if="decision?.note" class="record__note">{{ decision.note }}</p>
  </div>
</template>

<style scoped>
.record {
  padding: 8px 12px;
  border: var(--border-width) solid var(--border);
  border-left: 3px solid var(--accent);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  font-size: var(--text-sm);
}

.record__head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.record__title {
  flex: 1;
  min-width: 0;
  color: var(--text-muted);
}

.record__verdict {
  font-size: var(--text-xs);
}

.record__verdict--yes {
  color: var(--success);
}

.record__verdict--no {
  color: var(--text-faint);
}

.record__answers {
  margin: 6px 0 0;
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 2px 10px;
}

.record__answers dt {
  color: var(--text-muted);
}

.record__answers dd {
  margin: 0;
}

.record__note {
  margin: 4px 0 0;
  color: var(--text-faint);
}
</style>
