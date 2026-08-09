<!--
  提示词：全局五段 + 新会话默认身份/口吻。

  [environment] [operations] [voice] 全局生效；[identity] [style] 仅作为新会话起点。
-->

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import type { PromptSectionKey } from '../api/client'
import { errorText } from '../api/client'
import { client } from '../app/client'
import { configState, ensureConfig, reloadConfig } from '../app/useConfig'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'

const SECTIONS: { key: PromptSectionKey; label: string; hint: string; rows: number }[] = [
  {
    key: 'environment',
    label: '环境',
    hint: 'lya / 什亭之匣、老师是谁。全局生效，改完立刻 reload。',
    rows: 6,
  },
  {
    key: 'operations',
    label: '运行',
    hint: '工具、动作、HITL、失败策略。全局生效。',
    rows: 8,
  },
  {
    key: 'voice',
    label: '表达修正',
    hint: '去大模型八股；不禁普拉娜 OS 口癖。全局生效。',
    rows: 10,
  },
  {
    key: 'identity',
    label: '身份（新会话默认）',
    hint: '角色是谁、经历与动机。创建会话时抄一份，已有会话不受影响。',
    rows: 14,
  },
  {
    key: 'style',
    label: '口吻（新会话默认）',
    hint: '口癖、游戏原句参考、few-shot。创建会话时抄一份。',
    rows: 16,
  },
]

const active = ref<PromptSectionKey>('identity')
const drafts = ref<Record<PromptSectionKey, string>>({
  environment: '',
  operations: '',
  voice: '',
  identity: '',
  style: '',
})
const saving = ref(false)

const loading = configState.loading
const loadError = configState.error

const activeSection = computed(() => SECTIONS.find((s) => s.key === active.value)!)

onMounted(async () => {
  await ensureConfig()
  syncFromConfig()
  if (loadError.value) toast(`读取提示词失败：${loadError.value}`, 'error')
})

function syncFromConfig(): void {
  const p = configState.prompt.value
  if (!p) return
  drafts.value = { ...p }
}

async function save(): Promise<void> {
  saving.value = true
  try {
    await client.writePromptSection(active.value, drafts.value[active.value])
    void reloadConfig()
    toast('已保存', 'success')
  } catch (error) {
    toast(`保存失败：${errorText(error)}`, 'error')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="page">
    <ViewHead title="提示词" />

    <div class="page__body prompt">
      <p v-if="loadError" class="page__error">{{ loadError }}</p>
      <p v-else-if="loading" class="split-view__hint">正在读取…</p>

      <div v-else class="prompt__layout">
        <nav class="prompt__nav" aria-label="提示词段落">
          <button
            v-for="item in SECTIONS"
            :key="item.key"
            type="button"
            class="prompt__nav-btn"
            :class="{ 'prompt__nav-btn--on': active === item.key }"
            @click="active = item.key"
          >
            {{ item.label }}
          </button>
        </nav>

        <section class="page__pane prompt__editor">
          <p class="page__hint">{{ activeSection.hint }}</p>
          <p class="page__hint">
            写入 <code>prompt.toml</code> 的 <code>[{{ active }}]</code> 节。留空则该段回退内置默认。
          </p>
          <textarea
            v-model="drafts[active]"
            class="input prompt__text"
            :rows="activeSection.rows"
            :placeholder="`${activeSection.label}正文`"
          />
          <div class="row row--end">
            <button class="btn btn--primary" :disabled="saving" @click="save">
              {{ saving ? '保存中…' : '保存此段' }}
            </button>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.prompt__layout {
  display: grid;
  grid-template-columns: minmax(140px, 200px) 1fr;
  gap: 16px;
  align-items: start;
}

.prompt__nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.prompt__nav-btn {
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text);
  text-align: left;
  font-size: var(--text-sm);
  cursor: pointer;
}

.prompt__nav-btn--on {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, var(--surface));
}

.prompt__text {
  width: 100%;
  height: auto;
  padding: 10px 12px;
  line-height: var(--leading);
  resize: vertical;
  font-family: var(--font-mono);
}

@media (max-width: 720px) {
  .prompt__layout {
    grid-template-columns: 1fr;
  }

  .prompt__nav {
    flex-direction: row;
    flex-wrap: wrap;
  }
}
</style>
