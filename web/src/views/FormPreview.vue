<!--
  只读表单预览：展示 form 调用时的原始结构，不重复 HitlRecord 里的作答。
-->

<script setup lang="ts">
import type { FormCall } from '../utils/parseFormCall'

defineProps<{ form: FormCall; pending?: boolean }>()

function kindLabel(kind: FormCall['questions'][number]['kind']): string {
  switch (kind) {
    case 'single':
      return '单选'
    case 'multi':
      return '多选'
    case 'text':
      return '文本'
  }
}
</script>

<template>
  <div class="form-preview">
    <header class="form-preview__head">
      <strong class="form-preview__title">{{ form.title }}</strong>
      <span v-if="pending" class="form-preview__tag">等待填写</span>
    </header>

    <section v-for="question in form.questions" :key="question.id" class="form-preview__q">
      <p class="form-preview__prompt">
        <span>{{ question.text }}</span>
        <span class="form-preview__kind">{{ kindLabel(question.kind) }}</span>
      </p>

      <input
        v-if="question.kind === 'text'"
        class="input form-preview__input"
        disabled
        placeholder="文本回答"
      />

      <div v-else class="form-preview__options">
        <span v-for="option in question.options ?? []" :key="option.key" class="pill form-preview__pill">
          {{ option.label }}
        </span>
      </div>

      <input
        v-if="question.allow_note && question.kind !== 'text'"
        class="input form-preview__input"
        disabled
        placeholder="补充说明（可选）"
      />
    </section>
  </div>
</template>

<style scoped>
.form-preview {
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-family: var(--font-ui);
  white-space: normal;
}

.form-preview__head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.form-preview__title {
  flex: 1;
  min-width: 0;
  font-size: var(--text-sm);
  font-weight: 600;
}

.form-preview__tag {
  padding: 1px 8px;
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--info) 14%, var(--surface));
  color: var(--info);
  font-size: var(--text-xs);
}

.form-preview__q {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-preview__prompt {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 0;
  font-size: var(--text-sm);
}

.form-preview__kind {
  flex-shrink: 0;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.form-preview__options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.form-preview__pill {
  opacity: 0.72;
}

.form-preview__input {
  opacity: 0.72;
  cursor: not-allowed;
}
</style>
