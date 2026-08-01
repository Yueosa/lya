<!--
  分支树。

  上一代用的是自上而下的 SVG 图，概念对但画得笨重——节点 236×64 塞两行预览，
  几个分叉就铺满一屏。这里换成**缩进列表**：树的深度用缩进表达，分叉处才拉开
  层级。对话树通常又深又窄（一路问下去，偶尔重新生成一次分个叉），缩进比铺开
  的图省地方得多，也不用处理平移缩放。

  点节点弹出详情，看那一步到底调了什么、返回了什么。
-->

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import type { MessageRecord, SessionTree } from '../api/wire'
import { loadTree, switchBranch } from '../app/useChat'

const tree = ref<SessionTree | null>(null)
const picked = ref<MessageRecord | null>(null)

onMounted(async () => {
  tree.value = await loadTree()
})

/** 按父子关系铺成带缩进的一列。 */
interface Row {
  record: MessageRecord
  depth: number
  /** 这一层有几个兄弟，用来标出分叉点。 */
  siblings: number
  onActivePath: boolean
}

const rows = computed<Row[]>(() => {
  const data = tree.value
  if (!data) return []

  const children = new Map<number | null, MessageRecord[]>()
  for (const node of [...data.nodes].sort((a, b) => a.sort_key - b.sort_key)) {
    const list = children.get(node.parent_id) ?? []
    list.push(node)
    children.set(node.parent_id, list)
  }

  // 当前分支：从激活叶一路往上，用来高亮
  const active = new Set<number>()
  let cursor = data.active_leaf_id
  const byId = new Map(data.nodes.map((node) => [node.id, node]))
  while (cursor !== null && cursor !== undefined) {
    active.add(cursor)
    cursor = byId.get(cursor)?.parent_id ?? null
  }

  const out: Row[] = []
  const walk = (parent: number | null, depth: number): void => {
    const group = children.get(parent) ?? []
    for (const node of group) {
      out.push({
        record: node,
        // 只有一个孩子时不加缩进：一路直下的对话没必要越缩越深
        depth,
        siblings: group.length,
        onActivePath: active.has(node.id),
      })
      walk(node.id, group.length > 1 ? depth + 1 : depth)
    }
  }
  walk(null, 0)
  return out
})

/** 是不是叶节点——只有叶能切过去。 */
const leaves = computed(() => new Set(tree.value?.leaves ?? []))

function preview(record: MessageRecord): string {
  const text = record.payload.openai?.content?.trim()
  if (text) return text.replace(/\s+/g, ' ').slice(0, 60)
  if (record.payload.lya.hitl) return '（等你决定）'
  return '（无正文）'
}

function roleIcon(record: MessageRecord): string {
  switch (record.payload.role) {
    case 'user':
      return '🧑'
    case 'assistant':
      return record.payload.openai?.tool_calls?.length ? '🔧' : '🤖'
    case 'tool':
      return '↩'
    case 'hitl':
      return '✋'
    default:
      return '·'
  }
}

async function jump(record: MessageRecord): Promise<void> {
  if (!leaves.value.has(record.id)) return
  await switchBranch(record.id)
  tree.value = await loadTree()
}

/** 详情弹窗里要展示的调用。 */
const calls = computed(() => picked.value?.payload.openai?.tool_calls ?? [])
</script>

<template>
  <div class="tree">
    <h2 class="tree__title">分支</h2>
    <p v-if="!tree" class="tree__hint">正在读取…</p>
    <p v-else-if="rows.length === 0" class="tree__hint">这个会话还没有消息。</p>

    <ul class="tree__list">
      <li
        v-for="row in rows"
        :key="row.record.id"
        class="tree__row"
        :class="{ 'tree__row--active': row.onActivePath, 'tree__row--fork': row.siblings > 1 }"
        :style="{ paddingLeft: `${row.depth * 20 + 8}px` }"
      >
        <button class="tree__node" @click="picked = row.record">
          <span>{{ roleIcon(row.record) }}</span>
          <span class="tree__preview">{{ preview(row.record) }}</span>
          <span class="tree__id">#{{ row.record.id }}</span>
        </button>
        <button
          v-if="leaves.has(row.record.id) && !row.onActivePath"
          class="btn btn--sm"
          @click="jump(row.record)"
        >
          切到这条
        </button>
      </li>
    </ul>

    <!-- 详情：那一步到底调了什么 -->
    <div v-if="picked" class="overlay" @click.self="picked = null">
      <div class="dialog tree__detail">
        <h3 class="dialog__title">#{{ picked.id }} · {{ picked.payload.role }}</h3>
        <p class="dialog__message">{{ new Date(picked.created_at).toLocaleString('zh-CN') }}</p>

        <pre v-if="picked.payload.lya.reasoning" class="tree__block">💭 {{ picked.payload.lya.reasoning }}</pre>
        <pre v-if="picked.payload.openai?.content" class="tree__block">{{ picked.payload.openai.content }}</pre>

        <div v-for="call in calls" :key="call.id" class="tree__block">
          🔧 {{ call.function.name }}
          <pre class="tree__args">{{ call.function.arguments }}</pre>
        </div>

        <pre v-if="picked.payload.lya.hitl" class="tree__block">{{
          JSON.stringify(picked.payload.lya.hitl, null, 2)
        }}</pre>

        <div class="dialog__actions">
          <button class="btn" @click="picked = null">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tree {
  padding: 20px;
}

.tree__title {
  margin: 0 0 4px;
  font-size: var(--text-lg);
}

.tree__hint {
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.tree__list {
  list-style: none;
  margin: 12px 0 0;
  padding: 0;
}

.tree__row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 8px;
  border-left: var(--border-width) solid transparent;
}

.tree__row--active {
  border-left-color: var(--accent);
  background: var(--accent-soft);
}

/* 分叉点标出来，不然缩进变化看着像随机的 */
.tree__row--fork .tree__preview::before {
  content: '⑂ ';
  color: var(--info);
}

.tree__node {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
}

.tree__node:hover {
  background: var(--surface-hover);
}

.tree__preview {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tree__id {
  color: var(--text-faint);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.tree__detail {
  width: 620px;
}

.tree__block {
  margin: 0;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 220px;
  overflow: auto;
}

.tree__args {
  margin: 4px 0 0;
  color: var(--text-muted);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
