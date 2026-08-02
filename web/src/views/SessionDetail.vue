<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { client, loadTree, meta, readOnly, setPersona, state, tree } from '../app/useChat'
import { toast } from '../ui/useToast'

const globalPersona = ref<string | null>(null)
const loading = ref(true)
const editing = ref(false)
const draftPersona = ref('')

onMounted(async () => {
  loading.value = true
  void loadTree()
  try {
    const cfg = await client.config()
    globalPersona.value = cfg.persona ?? null
  } catch {
    try {
      const raw = await client.rawConfig('persona')
      globalPersona.value = raw.trim() || null
    } catch {
      globalPersona.value = null
    }
  } finally {
    loading.value = false
  }
})

watch(
  () => meta.value?.persona,
  () => {
    if (!editing.value) draftPersona.value = meta.value?.persona ?? ''
  },
  { immediate: true },
)

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

const stats = computed(() => {
  const messages = state.value.messages
  let user = 0
  let assistant = 0
  let tools = 0
  let reasoningChars = 0
  for (const record of messages) {
    if (record.payload.role === 'user') user += 1
    if (record.payload.role === 'assistant') {
      assistant += 1
      if (record.payload.openai?.tool_calls?.length) tools += record.payload.openai.tool_calls.length
    }
    if (record.payload.lya.reasoning) reasoningChars += record.payload.lya.reasoning.length
  }
  const branches = tree.value
    ? new Set(tree.value.map((n) => n.parent_id)).size - 1
    : null
  return { user, assistant, tools, reasoningChars, branches, total: messages.length }
})

function fmtTime(iso: string | undefined): string {
  if (!iso) return '—'
  return new Date(iso).toLocaleString('zh-CN', { hour12: false })
}

function startEditPersona(): void {
  draftPersona.value = meta.value?.persona ?? ''
  editing.value = true
}

async function savePersona(): Promise<void> {
  if (readOnly.value) return
  const text = draftPersona.value.trim()
  await setPersona(text || null)
  editing.value = false
  toast('人设已保存', 'success')
}
</script>

<template>
  <div class="detail">
    <p v-if="loading" class="detail__hint">加载中…</p>
    <template v-else>
      <section class="detail__section">
        <h3 class="detail__title">会话</h3>
        <dl class="detail__grid">
          <dt>标题</dt>
          <dd>{{ meta?.title || '未命名' }}</dd>
          <dt>ID</dt>
          <dd><code>{{ meta?.id }}</code></dd>
          <dt>状态</dt>
          <dd>{{ readOnly ? '已归档' : '活跃' }}</dd>
          <dt>模式</dt>
          <dd>{{ meta?.work_mode ?? '—' }}</dd>
          <dt>模型</dt>
          <dd>{{ meta?.model_id || '默认' }}</dd>
          <dt>创建</dt>
          <dd>{{ fmtTime(meta?.created_at) }}</dd>
          <dt>更新</dt>
          <dd>{{ fmtTime(meta?.updated_at) }}</dd>
        </dl>
      </section>

      <section class="detail__section">
        <h3 class="detail__title">统计</h3>
        <dl class="detail__grid">
          <dt>消息</dt>
          <dd>{{ stats.total }}</dd>
          <dt>用户</dt>
          <dd>{{ stats.user }}</dd>
          <dt>助手</dt>
          <dd>{{ stats.assistant }}</dd>
          <dt>工具调用</dt>
          <dd>{{ stats.tools }}</dd>
          <dt>思考字数</dt>
          <dd>{{ stats.reasoningChars }}</dd>
          <dt>分支节点</dt>
          <dd>{{ stats.branches ?? '—' }}</dd>
        </dl>
      </section>

      <section class="detail__section">
        <div class="detail__head-row">
          <h3 class="detail__title">人设 · {{ personaSource }}</h3>
          <button v-if="!readOnly && !editing" class="btn btn--sm" @click="startEditPersona">编辑</button>
        </div>
        <textarea
          v-if="editing"
          v-model="draftPersona"
          class="input detail__edit"
          rows="6"
          placeholder="留空则使用全局/默认人设"
        />
        <pre v-else class="detail__pre">{{ effectivePersona }}</pre>
        <div v-if="editing" class="detail__actions">
          <button class="btn btn--sm btn--primary" @click="savePersona">保存</button>
          <button class="btn btn--sm" @click="editing = false">取消</button>
        </div>
        <p v-if="!editing && personaSource !== '会话'" class="detail__note">
          当前未设置会话专属人设，生效的是{{ personaSource === '全局' ? '全局配置' : '内置默认' }}。
        </p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.detail {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.detail__hint {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.detail__section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.detail__head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.detail__title {
  margin: 0;
  font-size: var(--text-md);
  font-weight: 600;
}

.detail__grid {
  display: grid;
  grid-template-columns: 5.5em 1fr;
  gap: 6px 10px;
  margin: 0;
  font-size: var(--text-sm);
}

.detail__grid dt {
  color: var(--text-muted);
}

.detail__grid dd {
  margin: 0;
  word-break: break-all;
}

.detail__pre {
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

.detail__edit {
  height: auto;
  padding: 8px 12px;
  line-height: var(--leading);
  resize: vertical;
}

.detail__actions {
  display: flex;
  gap: 8px;
}

.detail__note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-xs);
}
</style>
