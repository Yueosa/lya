<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import type { MessageRecord } from '../api/wire'
import { currentId, loadTree, readOnly, switchToBranch } from '../app/useChat'
import { projectVisibleTree, treeNode } from '../model/branchTree'
import Icon from '../ui/Icon.vue'

const props = defineProps<{ open: boolean }>()
defineEmits<{ close: [] }>()

const NODE_W = 168
const NODE_H = 40
const GAP_X = 22
const GAP_Y = 34
const PAD = 16
const TEXT_X = 30
const TEXT_RIGHT = 18

const nodes = ref<MessageRecord[]>([])
const rawNodes = ref<MessageRecord[]>([])
const activeLeaf = ref<number | null>(null)
const loading = ref(false)
const picked = ref<MessageRecord | null>(null)
const switching = ref(false)
const width = ref(Number(localStorage.getItem('lya.tree.width')) || 460)
const resizing = ref(false)

watch(() => props.open, (open) => void (open && refresh()), { immediate: true })
watch(currentId, () => void (props.open && refresh()))

async function refresh(): Promise<void> {
  loading.value = true
  const data = await loadTree()
  if (data) {
    rawNodes.value = data.nodes
    const projected = projectVisibleTree(data.nodes, data.nodes.filter(treeNode), data.active_leaf_id)
    nodes.value = projected.nodes
    activeLeaf.value = projected.activeLeaf
  }
  loading.value = false
}

interface Placed {
  record: MessageRecord
  x: number
  y: number
  onPath: boolean
  isLeaf: boolean
}

interface Edge {
  x1: number
  y1: number
  x2: number
  y2: number
  onPath: boolean
}

const layout = computed(() => {
  const visibleNodes = [...nodes.value].sort((a, b) => a.sort_key - b.sort_key)
  const byId = new Map(visibleNodes.map((node) => [node.id, node]))
  const children = new Map<number | null, MessageRecord[]>()
  for (const node of visibleNodes) {
    const list = children.get(node.parent_id) ?? []
    list.push(node)
    children.set(node.parent_id, list)
  }

  const onPath = new Set<number>()
  let cursor = activeLeaf.value
  while (cursor !== null) {
    onPath.add(cursor)
    cursor = byId.get(cursor)?.parent_id ?? null
  }

  const leafIds = new Set<number>()
  for (const node of visibleNodes) {
    if ((children.get(node.id) ?? []).length === 0) leafIds.add(node.id)
  }

  const centers = new Map<number, number>()
  const depths = new Map<number, number>()
  let cursorX = 0

  const place = (id: number, depth: number): number => {
    depths.set(id, depth)
    const kids = children.get(id) ?? []
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

  const placed: Placed[] = visibleNodes
    .filter((node) => centers.has(node.id))
    .map((node) => ({
      record: node,
      x: centers.get(node.id)! - NODE_W / 2 + PAD,
      y: PAD + depths.get(node.id)! * (NODE_H + GAP_Y),
      onPath: onPath.has(node.id),
      isLeaf: leafIds.has(node.id),
    }))

  const placedIds = new Set(placed.map((p) => p.record.id))
  const edges: Edge[] = []
  for (const item of placed) {
    let parent = item.record.parent_id
    while (parent !== null && !placedIds.has(parent)) {
      parent = byId.get(parent)?.parent_id ?? null
    }
    if (parent === null) continue
    const from = placed.find((c) => c.record.id === parent)
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
    w: Math.max(...placed.map((i) => i.x + NODE_W), NODE_W) + PAD,
    h: Math.max(...placed.map((i) => i.y + NODE_H), NODE_H) + PAD,
  }
})

const canSwitchPicked = computed(() => {
  if (!picked.value || readOnly.value) return false
  if (picked.value.id === activeLeaf.value) return false
  return layout.value.placed.some((item) => item.record.id === picked.value!.id)
})

function preview(record: MessageRecord): string {
  return (record.payload.openai?.content ?? '').replace(/\s+/g, ' ').slice(0, 28)
}

function roleIcon(record: MessageRecord): 'user' | 'bot' {
  return record.payload.role === 'user' ? 'user' : 'bot'
}

function openNode(record: MessageRecord): void {
  picked.value = record
}

function closeModal(): void {
  picked.value = null
}

async function confirmSwitch(): Promise<void> {
  const record = picked.value
  if (!record || !canSwitchPicked.value) return
  switching.value = true
  try {
    await switchToBranch(record.id)
    await refresh()
    closeModal()
  } finally {
    switching.value = false
  }
}

function startResize(event: PointerEvent): void {
  resizing.value = true
  const startX = event.clientX
  const startWidth = width.value
  const move = (moved: PointerEvent): void => {
    width.value = Math.min(Math.max(320, startWidth - (moved.clientX - startX)), window.innerWidth * 0.8)
  }
  const up = (): void => {
    resizing.value = false
    localStorage.setItem('lya.tree.width', String(Math.round(width.value)))
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}
</script>

<template>
  <aside
    class="panel-tree"
    :class="{ 'panel-tree--open': open, 'panel-tree--resizing': resizing }"
    :style="{ '--local-w': `${width}px` }"
  >
    <div v-if="open" class="panel-tree__grip" @pointerdown.prevent="startResize" />

    <header class="panel-tree__head">
      <strong>分支</strong>
      <span class="panel-tree__gap" />
      <button class="btn btn--sm btn--ghost" v-tip="'刷新'" @click="refresh">
        <Icon name="refresh" size="sm" />
      </button>
      <button class="btn btn--sm btn--ghost" @click="$emit('close')">
        <Icon name="chevronRight" size="sm" />
      </button>
    </header>

    <p v-if="loading" class="panel-tree__hint">加载中…</p>
    <p v-else-if="layout.placed.length === 0" class="panel-tree__hint">暂无分支</p>

    <div v-else class="panel-tree__scroll">
      <svg :width="layout.w" :height="layout.h">
        <defs>
          <clipPath v-for="item in layout.placed" :id="`tree-clip-${item.record.id}`" :key="item.record.id">
            <rect :x="TEXT_X" y="0" :width="NODE_W - TEXT_X - TEXT_RIGHT" :height="NODE_H" />
          </clipPath>
        </defs>
        <line
          v-for="(edge, at) in layout.edges"
          :key="at"
          :x1="edge.x1"
          :y1="edge.y1"
          :x2="edge.x2"
          :y2="edge.y2"
          class="edge"
          :class="{ 'edge--path': edge.onPath }"
        />
        <g
          v-for="item in layout.placed"
          :key="item.record.id"
          class="node"
          :class="{
            'node--path': item.onPath && item.record.id !== activeLeaf,
            'node--here': item.record.id === activeLeaf,
          }"
          :transform="`translate(${item.x}, ${item.y})`"
          @click="openNode(item.record)"
        >
          <rect :width="NODE_W" :height="NODE_H" rx="6" />
          <foreignObject x="6" y="11" width="20" height="20">
            <Icon :name="roleIcon(item.record)" size="sm" />
          </foreignObject>
          <text
            class="node__text"
            :x="TEXT_X"
            y="25"
            :clip-path="`url(#tree-clip-${item.record.id})`"
          >
            {{ preview(item.record) }}
          </text>
        </g>
      </svg>
    </div>

    <p class="panel-tree__foot">单击节点查看详情；在弹窗内切换分支</p>
  </aside>

  <Transition name="lya-modal">
    <div v-if="picked" class="overlay" @click.self="closeModal">
    <div class="dialog tree-modal">
      <header class="tree-modal__head">
        <h3 class="dialog__title">#{{ picked.id }} · {{ picked.payload.role }}</h3>
        <button class="btn btn--sm btn--ghost" @click="closeModal">
          <Icon name="close" size="sm" />
        </button>
      </header>

      <section v-if="picked.payload.lya.reasoning" class="tree-modal__section">
        <h4>思考</h4>
        <pre class="tree-modal__block">{{ picked.payload.lya.reasoning }}</pre>
      </section>
      <section v-if="picked.payload.openai?.content" class="tree-modal__section">
        <h4>正文</h4>
        <pre class="tree-modal__block">{{ picked.payload.openai.content }}</pre>
      </section>

      <div class="tree-modal__actions">
        <button
          v-if="canSwitchPicked"
          class="btn btn--primary"
          :disabled="switching"
          @click="confirmSwitch"
        >
          {{ switching ? '切换中…' : '切换到此分支' }}
        </button>
        <button v-else-if="picked.id === activeLeaf" class="btn" disabled>当前分支</button>
        <button class="btn" @click="closeModal">关闭</button>
      </div>
    </div>
  </div>
  </Transition>
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

.panel-tree--resizing {
  transition: none;
}

.panel-tree__grip {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
  z-index: 2;
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
  padding: 8px 12px;
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
  stroke: var(--info);
  stroke-opacity: 0.45;
  stroke-width: 1.7;
  stroke-linecap: round;
}

.edge--path {
  stroke: var(--accent);
  stroke-width: 2.4;
  stroke-opacity: 1;
}

.node {
  cursor: pointer;
}

.node rect {
  fill: var(--surface);
  stroke: var(--border);
  stroke-width: 1.1;
}

.node:hover rect {
  fill: var(--surface-hover);
}

.node--path:not(.node--here) rect {
  stroke: var(--accent);
  stroke-width: 2;
  fill: var(--accent-soft);
}

.node--here rect {
  stroke: var(--info);
  stroke-width: 2;
  fill: color-mix(in srgb, var(--info) 14%, var(--surface));
}

.node__icon,
.node__text {
  font-size: 12px;
  fill: var(--text);
  pointer-events: none;
}

.tree-modal {
  width: min(520px, calc(100vw - 32px));
  max-height: calc(100vh - 32px);
}

.tree-modal__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.tree-modal__section h4 {
  margin: 0 0 6px;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--accent);
}

.tree-modal__block {
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  border: var(--border-width) solid var(--border);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 240px;
  overflow: auto;
}

.tree-modal__actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  flex-wrap: wrap;
}
</style>
