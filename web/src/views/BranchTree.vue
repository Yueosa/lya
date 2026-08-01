<!--
  分支树：SVG 图，从上往下长。

  布局用经典的 tidy-tree 递归摆放——叶子按顺序占位，父节点居中对齐到子节点的
  中点。上一代也是这个算法，概念没问题，问题出在节点太胖（236×64 塞两行预览），
  几个分叉就铺满一屏。这里把节点压到 168×40 只放一行，横向密度高一倍多。

  面板从右侧推进来、可拖宽。做成侧栏而不是弹窗，是为了能一边看树一边看对话
  ——切分支之后想立刻确认切对了没有。
-->

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { MessageRecord } from '../api/wire'
import { currentId, loadTree, switchBranch } from '../app/useChat'

const props = defineProps<{ open: boolean }>()
defineEmits<{ close: [] }>()

/** 节点尺寸。比上一代小一半多，一屏能看下更多分支。 */
const NODE_W = 168
const NODE_H = 40
const GAP_X = 22
const GAP_Y = 34
const PAD = 16

const nodes = ref<MessageRecord[]>([])
const activeLeaf = ref<number | null>(null)
const leaves = ref<Set<number>>(new Set())
const loading = ref(false)
const picked = ref<MessageRecord | null>(null)
/** 折叠起来的子树。 */
const collapsed = ref<Set<number>>(new Set())

const width = ref(Number(localStorage.getItem('lya.tree.width')) || 460)
const dragging = ref(false)

watch(() => props.open, (open) => void (open && refresh()), { immediate: true })
watch(currentId, () => void (props.open && refresh()))
onMounted(() => void (props.open && refresh()))

async function refresh(): Promise<void> {
  loading.value = true
  const data = await loadTree()
  if (data) {
    nodes.value = data.nodes
    activeLeaf.value = data.active_leaf_id
    leaves.value = new Set(data.leaves)
  }
  loading.value = false
}

interface Placed {
  record: MessageRecord
  x: number
  y: number
  onPath: boolean
  hasChildren: boolean
  folded: boolean
}

interface Edge {
  x1: number
  y1: number
  x2: number
  y2: number
  onPath: boolean
}

/** 摆位置。 */
const layout = computed(() => {
  const all = [...nodes.value].sort((a, b) => a.sort_key - b.sort_key)
  const byId = new Map(all.map((node) => [node.id, node]))
  const children = new Map<number | null, MessageRecord[]>()
  for (const node of all) {
    const list = children.get(node.parent_id) ?? []
    list.push(node)
    children.set(node.parent_id, list)
  }

  // 当前分支：从激活叶一路往上，用来把主干高亮出来
  const onPath = new Set<number>()
  let cursor = activeLeaf.value
  while (cursor !== null) {
    onPath.add(cursor)
    cursor = byId.get(cursor)?.parent_id ?? null
  }

  const centers = new Map<number, number>()
  const depths = new Map<number, number>()
  let cursorX = 0

  /** 叶子按顺序占位，父节点居中对齐到首尾子节点的中点。 */
  const place = (id: number, depth: number): number => {
    depths.set(id, depth)
    const kids = collapsed.value.has(id) ? [] : (children.get(id) ?? [])
    if (kids.length === 0) {
      const center = cursorX + NODE_W / 2
      cursorX += NODE_W + GAP_X
      centers.set(id, center)
      return center
    }
    const spans = kids.map((kid) => place(kid.id, depth + 1))
    const center = (spans[0]! + spans.at(-1)!) / 2
    centers.set(id, center)
    return center
  }

  for (const root of children.get(null) ?? []) place(root.id, 0)

  const placed: Placed[] = all
    .filter((node) => centers.has(node.id))
    .map((node) => ({
      record: node,
      x: centers.get(node.id)! - NODE_W / 2 + PAD,
      y: PAD + depths.get(node.id)! * (NODE_H + GAP_Y),
      onPath: onPath.has(node.id),
      hasChildren: (children.get(node.id) ?? []).length > 0,
      folded: collapsed.value.has(node.id),
    }))

  const edges: Edge[] = []
  for (const item of placed) {
    const parent = item.record.parent_id
    if (parent === null) continue
    const from = placed.find((candidate) => candidate.record.id === parent)
    if (!from) continue
    edges.push({
      x1: from.x + NODE_W / 2,
      y1: from.y + NODE_H,
      x2: item.x + NODE_W / 2,
      y2: item.y,
      onPath: item.onPath && from.onPath,
    })
  }

  return {
    placed,
    edges,
    w: Math.max(...placed.map((item) => item.x + NODE_W), 0) + PAD,
    h: Math.max(...placed.map((item) => item.y + NODE_H), 0) + PAD,
  }
})

function preview(record: MessageRecord): string {
  const text = record.payload.openai?.content?.trim()
  if (text) return text.replace(/\s+/g, ' ').slice(0, 22)
  if (record.payload.lya.hitl) return '等你决定'
  if (record.payload.openai?.tool_calls?.length) {
    return record.payload.openai.tool_calls.map((call) => call.function.name).join(' ')
  }
  return '（无正文）'
}

function icon(record: MessageRecord): string {
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

function toggleFold(id: number): void {
  const next = new Set(collapsed.value)
  if (!next.delete(id)) next.add(id)
  collapsed.value = next
}

async function jump(record: MessageRecord): Promise<void> {
  if (!leaves.value.has(record.id) || record.id === activeLeaf.value) return
  await switchBranch(record.id)
  await refresh()
}

/** 拖左边缘调宽度。 */
function startResize(event: PointerEvent): void {
  dragging.value = true
  const startX = event.clientX
  const startWidth = width.value
  const move = (moved: PointerEvent): void => {
    width.value = Math.min(Math.max(320, startWidth - (moved.clientX - startX)), window.innerWidth * 0.8)
  }
  const up = (): void => {
    dragging.value = false
    localStorage.setItem('lya.tree.width', String(Math.round(width.value)))
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}
</script>

<template>
  <aside class="panel-tree" :class="{ 'panel-tree--open': open }" :style="{ '--local-w': `${width}px` }">
    <div v-if="open" class="panel-tree__grip" @pointerdown.prevent="startResize" />

    <header class="panel-tree__head">
      <strong>分支</strong>
      <span class="panel-tree__gap" />
      <button class="btn btn--sm btn--ghost" title="刷新" @click="refresh">↻</button>
      <button class="btn btn--sm btn--ghost" title="关闭" @click="$emit('close')">›</button>
    </header>

    <p v-if="loading" class="panel-tree__hint">正在读取…</p>
    <p v-else-if="layout.placed.length === 0" class="panel-tree__hint">这个会话还没有消息。</p>

    <div v-else class="panel-tree__scroll">
      <svg :width="layout.w" :height="layout.h">
        <line
          v-for="(edge, at) in layout.edges"
          :key="at"
          :x1="edge.x1"
          :y1="edge.y1"
          :x2="edge.x2"
          :y2="edge.y2"
          class="edge"
          :class="{ 'edge--on': edge.onPath }"
        />

        <g
          v-for="item in layout.placed"
          :key="item.record.id"
          class="node"
          :class="{
            'node--on': item.onPath,
            'node--leaf': leaves.has(item.record.id),
            'node--here': item.record.id === activeLeaf,
          }"
          :transform="`translate(${item.x}, ${item.y})`"
        >
          <rect
            :width="NODE_W"
            :height="NODE_H"
            rx="6"
            @click="picked = item.record"
            @dblclick="jump(item.record)"
          />
          <text class="node__icon" x="10" y="25">{{ icon(item.record) }}</text>
          <text class="node__text" x="30" y="25">{{ preview(item.record) }}</text>

          <!-- 折叠：子树多了之后，收起看不到的部分 -->
          <g
            v-if="item.hasChildren"
            class="node__fold"
            :transform="`translate(${NODE_W - 14}, ${NODE_H - 12})`"
            @click.stop="toggleFold(item.record.id)"
          >
            <circle r="7" />
            <text y="4">{{ item.folded ? '+' : '−' }}</text>
          </g>
        </g>
      </svg>
    </div>

    <p class="panel-tree__foot">单击看详情，双击叶节点切过去</p>

    <!-- 详情 -->
    <div v-if="picked" class="overlay" @click.self="picked = null">
      <div class="dialog panel-tree__detail">
        <h3 class="dialog__title">#{{ picked.id }} · {{ picked.payload.role }}</h3>
        <p class="dialog__message">{{ new Date(picked.created_at).toLocaleString('zh-CN') }}</p>

        <pre v-if="picked.payload.lya.reasoning" class="panel-tree__block">💭 {{ picked.payload.lya.reasoning }}</pre>
        <pre v-if="picked.payload.openai?.content" class="panel-tree__block">{{ picked.payload.openai.content }}</pre>
        <div v-for="call in picked.payload.openai?.tool_calls ?? []" :key="call.id" class="panel-tree__block">
          🔧 {{ call.function.name }}
          <pre class="panel-tree__args">{{ call.function.arguments }}</pre>
        </div>
        <pre v-if="picked.payload.lya.hitl" class="panel-tree__block">{{
          JSON.stringify(picked.payload.lya.hitl, null, 2)
        }}</pre>

        <div class="dialog__actions">
          <button
            v-if="leaves.has(picked.id) && picked.id !== activeLeaf"
            class="btn btn--primary"
            @click="jump(picked!).then(() => (picked = null))"
          >
            切到这条分支
          </button>
          <button class="btn" @click="picked = null">关闭</button>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.panel-tree {
  position: relative;
  flex-shrink: 0;
  width: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--bg-sunken);
  border-left: 0 solid var(--border);
  transition: width 0.18s ease;
}

.panel-tree--open {
  width: var(--local-w);
  border-left-width: var(--border-width);
}

/* 左边缘的拖拽把手 */
.panel-tree__grip {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
  z-index: 2;
}

.panel-tree__grip:hover {
  background: var(--accent-soft);
}

.panel-tree__head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px;
  border-bottom: var(--border-width) solid var(--border);
}

.panel-tree__gap {
  flex: 1;
}

.panel-tree__hint,
.panel-tree__foot {
  margin: 0;
  padding: 10px 12px;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.panel-tree__foot {
  border-top: var(--border-width) solid var(--border);
}

.panel-tree__scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px;
}

.edge {
  stroke: var(--border-strong);
  stroke-width: 1.5;
  stroke-opacity: 0.5;
}

/* 主干高亮，一眼看出当前走的是哪条 */
.edge--on {
  stroke: var(--accent);
  stroke-width: 2.5;
  stroke-opacity: 1;
}

.node rect {
  fill: var(--surface);
  stroke: var(--border);
  stroke-width: 1;
  cursor: pointer;
}

.node:hover rect {
  fill: var(--surface-hover);
}

.node--on rect {
  stroke: var(--accent);
  stroke-width: 2;
  fill: var(--accent-soft);
}

/* 当前所在的那一片叶子单独标出来 */
.node--here rect {
  stroke: var(--info);
  stroke-width: 2.5;
}

.node--leaf:not(.node--here) rect {
  stroke-dasharray: none;
}

.node__icon,
.node__text {
  font-size: 11px;
  fill: var(--text);
  pointer-events: none;
}

.node__text {
  font-family: var(--font-ui);
}

.node__fold {
  cursor: pointer;
}

.node__fold circle {
  fill: var(--bg-sunken);
  stroke: var(--border-strong);
  stroke-width: 1;
}

.node__fold text {
  font-size: 11px;
  fill: var(--text-muted);
  text-anchor: middle;
}

.panel-tree__detail {
  width: 620px;
}

.panel-tree__block {
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

.panel-tree__args {
  margin: 4px 0 0;
  color: var(--text-muted);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
