<script setup lang="ts">
import { computed, onMounted } from 'vue'

import { loadTree, meta, readOnly, state, tree } from '../../app/useChat'

onMounted(() => {
  void loadTree()
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
</script>

<template>
  <div class="session-tab">
    <section class="session-tab__section">
      <h3 class="session-tab__title">基本信息</h3>
      <dl class="session-tab__grid">
        <dt>标题</dt>
        <dd>{{ meta?.title || '未命名' }}</dd>
        <dt>ID</dt>
        <dd><code>{{ meta?.id }}</code></dd>
        <dt>状态</dt>
        <dd>{{ readOnly ? '已归档' : '活跃' }}</dd>
        <dt>创建</dt>
        <dd>{{ fmtTime(meta?.created_at) }}</dd>
        <dt>更新</dt>
        <dd>{{ fmtTime(meta?.updated_at) }}</dd>
      </dl>
    </section>

    <section class="session-tab__section">
      <h3 class="session-tab__title">统计</h3>
      <dl class="session-tab__grid">
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

    <p v-if="readOnly" class="session-tab__note">已归档 · 只读</p>
  </div>
</template>

<style scoped>
.session-tab {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.session-tab__section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.session-tab__title {
  margin: 0;
  font-size: var(--text-md);
  font-weight: 600;
}

.session-tab__grid {
  display: grid;
  grid-template-columns: 5.5em 1fr;
  gap: 6px 10px;
  margin: 0;
  font-size: var(--text-sm);
}

.session-tab__grid dt {
  color: var(--text-muted);
}

.session-tab__grid dd {
  margin: 0;
  word-break: break-all;
}

.session-tab__note {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}
</style>
