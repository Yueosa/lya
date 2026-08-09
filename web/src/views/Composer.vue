<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import { canSend, currentId, pendingHitl, running, readComposerDraft, send, stop, writeComposerDraft } from '../app/useChat'
import ContextUsagePanel from './ContextUsagePanel.vue'
import HitlTray from './HitlTray.vue'

const draft = ref('')

watch(
  currentId,
  (id) => {
    draft.value = readComposerDraft(id)
    void nextTick(grow)
  },
  { immediate: true },
)

watch(draft, (text) => {
  writeComposerDraft(currentId.value, text)
})

const input = ref<HTMLTextAreaElement | null>(null)

const MAX_HEIGHT = 150

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
  writeComposerDraft(currentId.value, '')
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
</script>

<template>
  <div class="composer">
    <HitlTray />

    <div class="composer__row">
      <ContextUsagePanel />

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
  padding: 10px 20px 12px;
}

.composer__row {
  max-width: 1320px;
  margin: 0 auto;
  display: flex;
  align-items: flex-end;
  gap: 10px;
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

.composer__hint {
  max-width: 1320px;
  margin: 6px auto 0;
  font-size: var(--text-xs);
  color: var(--text-faint);
  text-align: center;
}

@media (max-width: 720px) {
  .composer {
    padding: 8px 12px 10px;
  }
}
</style>
