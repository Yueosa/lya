<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import type { MessageRecord } from '../api/wire'
import { currentId, deleteMessage, loadTree, readOnly, switchToBranch } from '../app/useChat'
import {
  callArguments,
  defaultTreeFilters,
  nodeIcon,
  nodePreview,
  nodeStatusTag,
  prettyJson,
  projectVisibleTree,
  treeNode,
  type TreeFilters,
} from '../model/branchTree'
import Icon from '../ui/Icon.vue'
import { confirmAsync } from '../ui/useDialog'
import { fmtBubbleTooltip } from '../utils/dateFormat'

const props = defineProps<{ open: boolean }>()
defineEmits<{ close: [] }>()

const NODE_W = 168
const NODE_H = 40
const GAP_X = 22
const GAP_Y = 34
const PAD = 16
const TEXT_X = 30
const TEXT_RIGHT = 18

const FILTER_KEY = 'lya.tree.filters.v2'
const MIN_PANEL_WIDTH = 240
const MAX_PANEL_RATIO = 0.8
const PANEL_PAD = 32

function loadFilters(): TreeFilters {
  try {
    const raw = localStorage.getItem(FILTER_KEY)
    if (!raw) return { ...defaultTreeFilters }
    return { ...defaultTreeFilters, ...JSON.parse(raw) }
  } catch {
    return { ...defaultTreeFilters }
  }
}

const nodes = ref<MessageRecord[]>([])
const rawNodes = ref<MessageRecord[]>([])
const activeLeaf = ref<number | null>(null)
const loading = ref(false)
const picked = ref<MessageRecord | null>(null)
const switching = ref(false)
const deleting = ref(false)
const width = ref(MIN_PANEL_WIDTH)
/** 当前打开期间的手动宽度；关闭面板后丢弃，避免污染下次自动计算。 */
const manualWidth = ref<number | null>(null)
const resizing = ref(false)
const scrollEl = ref<HTMLElement | null>(null)
const filters = ref<TreeFilters>(loadFilters())

watch(
  filters,
  (value) => {
    localStorage.setItem(FILTER_KEY, JSON.stringify(value))
    if (props.open) void refresh()
  },
  { deep: true },
)

watch(() => props.open, (open) => {
  if (open) {
    manualWidth.value = null
    void refresh()
  }
}, { immediate: true })
watch(currentId, () => void (props.open && refresh()))

function visibleNodes(): MessageRecord[] {
  return rawNodes.value.filter((node) => treeNode(node, filters.value))
}

async function refresh(): Promise<void> {
  loading.value = true
  const data = await loadTree()
  if (data) {
    rawNodes.value = data.nodes
    const visible = visibleNodes()
    const projected = projectVisibleTree(data.nodes, visible, data.active_leaf_id)
    nodes.value = projected.nodes
    activeLeaf.value = projected.activeLeaf
  }
  loading.value = false
  await nextTick()
  fitPanelWidth()
  scrollToActive()
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
  const visibleNodesList = [...nodes.value].sort((a, b) => a.sort_key - b.sort_key)
  const byId = new Map(visibleNodesList.map((node) => [node.id, node]))
  const children = new Map<number | null, MessageRecord[]>()
  for (const node of visibleNodesList) {
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
  for (const node of visibleNodesList) {
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

  const placed: Placed[] = visibleNodesList
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

const pickedPlaced = computed(() =>
  picked.value ? layout.value.placed.find((item) => item.record.id === picked.value!.id) : null,
)

const canSwitchPicked = computed(() => {
  if (!picked.value || readOnly.value) return false
  if (picked.value.id === activeLeaf.value) return false
  return layout.value.placed.some((item) => item.record.id === picked.value!.id)
})

const canDeletePicked = computed(() => {
  if (!picked.value || readOnly.value) return false
  return pickedPlaced.value?.isLeaf ?? false
})

function clampPanelWidth(value: number): number {
  return Math.min(Math.max(value, MIN_PANEL_WIDTH), window.innerWidth * MAX_PANEL_RATIO)
}

function panelWidthForLayout(): number {
  if (layout.value.placed.length === 0) return MIN_PANEL_WIDTH
  return clampPanelWidth(layout.value.w + PANEL_PAD)
}

function fitPanelWidth(): void {
  width.value = manualWidth.value === null ? panelWidthForLayout() : clampPanelWidth(manualWidth.value)
}

watch(
  () => layout.value.w,
  () => {
    if (props.open && manualWidth.value === null) fitPanelWidth()
  },
)

function scrollToActive(): void {
  const el = scrollEl.value
  if (!el || activeLeaf.value === null) return
  const item = layout.value.placed.find((p) => p.record.id === activeLeaf.value)
  if (!item) return
  const cx = item.x + NODE_W / 2
  const cy = item.y + NODE_H / 2
  el.scrollTo({
    left: Math.max(0, cx - el.clientWidth / 2),
    top: Math.max(0, cy - el.clientHeight / 2),
    behavior: 'smooth',
  })
}

function roleLabel(record: MessageRecord): string {
  switch (record.payload.role) {
    case 'user':
      return '用户'
    case 'assistant':
      return '助手'
    case 'tool':
      return '工具'
    case 'hitl':
      return 'HITL'
    case 'system':
      return '系统'
    default:
      return record.payload.role
  }
}

function openNode(record: MessageRecord): void {
  picked.value = record
  copied.value = false
}

function closeModal(): void {
  picked.value = null
}

const copied = ref(false)

async function copyRaw(): Promise<void> {
  if (!picked.value) return
  await navigator.clipboard.writeText(prettyJson(picked.value.payload))
  copied.value = true
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

async function confirmDelete(): Promise<void> {
  const record = picked.value
  if (!record || !canDeletePicked.value) return
  await confirmAsync({
    title: '删除消息',
    message: '只能删末端消息。',
    confirmText: '删除',
    danger: true,
    run: async () => {
      deleting.value = true
      try {
        await deleteMessage(record.id)
        closeModal()
        await refresh()
      } finally {
        deleting.value = false
      }
    },
  })
}

function startResize(event: PointerEvent): void {
  resizing.value = true
  const startX = event.clientX
  const startWidth = width.value
  const move = (moved: PointerEvent): void => {
    const next = clampPanelWidth(startWidth - (moved.clientX - startX))
    width.value = next
    manualWidth.value = next
  }
  const up = (): void => {
    resizing.value = false
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

    <div v-else ref="scrollEl" class="panel-tree__scroll">
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
            [`node--${item.record.payload.role}`]: true,
          }"
          :transform="`translate(${item.x}, ${item.y})`"
          @click="openNode(item.record)"
        >
          <rect :width="NODE_W" :height="NODE_H" rx="6" />
          <foreignObject x="6" y="11" width="20" height="20">
            <Icon :name="nodeIcon(item.record)" size="sm" />
          </foreignObject>
          <text
            class="node__text"
            :x="TEXT_X"
            y="25"
            :clip-path="`url(#tree-clip-${item.record.id})`"
          >
            {{ nodePreview(item.record) }}
          </text>
          <text
            v-if="nodeStatusTag(item.record)"
            class="node__tag"
            :x="NODE_W - 6"
            y="14"
            text-anchor="end"
          >
            {{ nodeStatusTag(item.record) }}
          </text>
        </g>
      </svg>
    </div>

    <div class="panel-tree__filters">
      <label><input v-model="filters.hideTools" type="checkbox" /> 隐藏工具</label>
      <label><input v-model="filters.hideHitl" type="checkbox" /> 隐藏 HITL</label>
      <label><input v-model="filters.hideModeChange" type="checkbox" /> 隐藏模式变更</label>
      <label><input v-model="filters.hideSystem" type="checkbox" /> 隐藏 system</label>
    </div>
  </aside>

  <Transition name="lya-modal">
    <div v-if="picked" class="overlay" @click.self="closeModal">
      <div class="dialog tree-modal">
        <header class="tree-modal__head">
          <h3 class="dialog__title">
            #{{ picked.id }} · {{ roleLabel(picked) }}
            <span v-if="picked.payload.status !== 'complete'" class="tree-modal__status">
              {{ picked.payload.status }}
            </span>
          </h3>
          <button class="btn btn--sm btn--ghost" @click="closeModal">
            <Icon name="close" size="sm" />
          </button>
        </header>

        <dl class="tree-modal__facts">
          <div><dt>类型</dt><dd>{{ picked.payload.kind }}</dd></div>
          <div><dt>状态</dt><dd>{{ picked.payload.status }}</dd></div>
          <div><dt>父节点</dt><dd>{{ picked.parent_id ?? '根' }}</dd></div>
          <div><dt>序号</dt><dd>{{ picked.sort_key }}</dd></div>
          <div><dt>时间</dt><dd>{{ fmtBubbleTooltip(picked.created_at) }}</dd></div>
        </dl>

        <section v-if="picked.payload.lya.reasoning" class="tree-modal__section">
          <h4>思考</h4>
          <pre class="tree-modal__block">{{ picked.payload.lya.reasoning }}</pre>
        </section>

        <section v-if="picked.payload.openai?.tool_calls?.length" class="tree-modal__section">
          <h4>工具调用</h4>
          <div v-for="call in picked.payload.openai.tool_calls" :key="call.id" class="tree-modal__call">
            <div class="tree-modal__call-head">
              <code>{{ call.function.name }}</code>
              <span class="tree-modal__call-id">{{ call.id }}</span>
              <span v-if="callArguments(call.function.arguments).broken" class="tree-modal__warn">
                参数无效
              </span>
            </div>
            <pre class="tree-modal__block">{{ callArguments(call.function.arguments).text }}</pre>
          </div>
        </section>

        <section v-if="picked.payload.role === 'tool'" class="tree-modal__section">
          <h4>工具结果</h4>
          <div class="tree-modal__call-head">
            <span class="tree-modal__call-id">{{ picked.payload.openai?.tool_call_id }}</span>
          </div>
          <pre class="tree-modal__block">{{ picked.payload.openai?.content }}</pre>
        </section>

        <section v-if="picked.payload.lya.responses_items?.length" class="tree-modal__section">
          <h4>Responses items</h4>
          <pre class="tree-modal__block">{{ prettyJson(picked.payload.lya.responses_items) }}</pre>
        </section>

        <section v-if="picked.payload.lya.hitl" class="tree-modal__section">
          <h4>HITL</h4>
          <pre class="tree-modal__block">{{ prettyJson(picked.payload.lya.hitl) }}</pre>
        </section>

        <section v-if="picked.payload.role !== 'tool' && picked.payload.openai?.content" class="tree-modal__section">
          <h4>正文</h4>
          <pre class="tree-modal__block">{{ picked.payload.openai.content }}</pre>
        </section>

        <section class="tree-modal__section">
          <details>
            <summary class="tree-modal__summary">
              原始 payload
              <button class="btn btn--sm btn--ghost" @click.prevent="copyRaw">
                {{ copied ? '已复制' : '复制' }}
              </button>
            </summary>
            <pre class="tree-modal__block">{{ prettyJson(picked.payload) }}</pre>
          </details>
        </section>

        <div class="tree-modal__actions">
          <button
            v-if="canDeletePicked"
            class="btn btn--danger"
            :disabled="deleting"
            @click="confirmDelete"
          >
            {{ deleting ? '删除中…' : '删除' }}
          </button>
          <span class="tree-modal__gap" />
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

.panel-tree__hint {
  margin: 0;
  padding: 8px 12px;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.panel-tree__scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 4px;
}

.panel-tree__filters {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 12px;
  padding: 8px 12px;
  border-top: var(--border-width) solid var(--border);
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.panel-tree__filters label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  user-select: none;
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

.node--tool rect {
  stroke: color-mix(in srgb, var(--info) 50%, var(--border));
}

.node--hitl rect {
  stroke: color-mix(in srgb, var(--danger) 40%, var(--border));
}

.node--system rect {
  stroke: color-mix(in srgb, var(--text-faint) 60%, var(--border));
}

.node__text {
  font-size: 12px;
  fill: var(--text);
  pointer-events: none;
}

.node__tag {
  font-size: 9px;
  fill: var(--danger);
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

.tree-modal__status {
  margin-left: 6px;
  padding: 1px 6px;
  border-radius: var(--radius-pill);
  background: var(--surface-active);
  color: var(--text-muted);
  font-size: var(--text-xs);
  font-weight: 400;
}

.tree-modal__facts {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  margin: 0 0 4px;
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.tree-modal__facts div {
  display: flex;
  gap: 4px;
}

.tree-modal__facts dt {
  color: var(--text-faint);
}

.tree-modal__facts dd {
  margin: 0;
  font-family: var(--font-mono);
}

.tree-modal__call-head {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 4px;
  font-size: var(--text-xs);
}

.tree-modal__call-id {
  color: var(--text-faint);
  font-family: var(--font-mono);
}

.tree-modal__warn {
  padding: 1px 6px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--danger) 16%, transparent);
  color: var(--danger);
}

.tree-modal__summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  cursor: pointer;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--accent);
}

.tree-modal__section h4 {
  margin: 0 0 6px;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--accent);
}

.tree-modal__block {
  margin: 0 0 6px;
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
  align-items: center;
  flex-wrap: wrap;
}

.tree-modal__gap {
  flex: 1;
}
</style>
