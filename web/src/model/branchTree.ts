import type { HitlBlock, MessageRecord } from '../api/wire'

/** 分支树节点筛选：勾选即隐藏对应类型。 */
export interface TreeFilters {
  hideTools: boolean
  hideHitl: boolean
  hideModeChange: boolean
  hideSystem: boolean
}

export const defaultTreeFilters: TreeFilters = {
  hideTools: true,
  hideHitl: true,
  hideModeChange: true,
  hideSystem: true,
}

export function isModeChangeNode(record: MessageRecord): boolean {
  if (record.payload.role === 'system') {
    return record.payload.openai?.content?.startsWith('[模式变更]') ?? false
  }
  if (record.payload.role === 'hitl') {
    return record.payload.lya.hitl?.type === 'mode_change'
  }
  return false
}

/** 分支树里要画出来的节点；受筛选器控制。 */
export function treeNode(record: MessageRecord, filters: TreeFilters = defaultTreeFilters): boolean {
  if (isModeChangeNode(record) && filters.hideModeChange) return false

  if (record.payload.role === 'tool') {
    return !filters.hideTools
  }

  if (record.payload.role === 'hitl') {
    if (filters.hideHitl) return false
    return Boolean(record.payload.lya.hitl)
  }

  if (record.payload.role === 'system') {
    if (filters.hideSystem) return false
    return Boolean(record.payload.openai?.content?.trim())
  }

  if (record.payload.role === 'user') {
    return Boolean(record.payload.openai?.content?.trim())
  }

  if (record.payload.role === 'assistant') {
    const text = record.payload.openai?.content?.trim()
    const hasTools = (record.payload.openai?.tool_calls?.length ?? 0) > 0
    const interrupted = record.payload.status === 'interrupted'
    return Boolean(text) || hasTools || interrupted
  }

  return false
}

/** 节点上显示的短标签。 */
export function nodePreview(record: MessageRecord): string {
  const clip = (text: string, max = 28): string =>
    text.replace(/\s+/g, ' ').trim().slice(0, max)

  if (record.payload.role === 'tool') {
    const content = record.payload.openai?.content ?? ''
    return clip(content || '工具结果')
  }

  if (record.payload.role === 'hitl') {
    return clip(hitlTitle(record.payload.lya.hitl))
  }

  if (record.payload.role === 'system') {
    return clip(record.payload.openai?.content ?? '系统')
  }

  if (record.payload.role === 'assistant') {
    const text = record.payload.openai?.content?.trim()
    if (text) return clip(text)
    const call = record.payload.openai?.tool_calls?.[0]
    if (call) return clip(`调用 ${call.function.name}`)
    if (record.payload.status === 'interrupted') return '生成中断'
  }

  return clip(record.payload.openai?.content ?? '')
}

/** 节点图标键。 */
export function nodeIcon(
  record: MessageRecord,
): 'user' | 'bot' | 'tool' | 'warning' | 'info' | 'branch' {
  if (record.payload.role === 'user') return 'user'
  if (record.payload.role === 'tool') return 'tool'
  if (record.payload.role === 'hitl') {
    return record.payload.lya.hitl?.type === 'mode_change' ? 'branch' : 'warning'
  }
  if (record.payload.role === 'system') return 'info'
  return 'bot'
}

/** 节点状态后缀（pending / interrupted 等）。 */
export function nodeStatusTag(record: MessageRecord): string | null {
  const { status, role } = record.payload
  if (role === 'hitl' && status === 'pending') return '待答复'
  if (role === 'assistant' && status === 'interrupted') return '中断'
  if (role === 'assistant' && status === 'streaming') return '生成中'
  return null
}

/** 缩进过的 JSON；拿不动就退回 `String()`，别让整个面板炸掉。 */
export function prettyJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2) ?? String(value)
  } catch {
    return String(value)
  }
}

/**
 * 模型发出的参数原文，带一个「能不能用」的判断。
 *
 * 参数为空或不是合法 JSON，这次调用注定失败——面板要能一眼看出来，
 * 而不是只显示一个函数名。
 */
export function callArguments(raw: string): { text: string; broken: boolean } {
  const trimmed = raw.trim()
  if (!trimmed) return { text: '(模型未传参数)', broken: true }
  try {
    return { text: prettyJson(JSON.parse(trimmed)), broken: false }
  } catch {
    return { text: trimmed, broken: true }
  }
}

function hitlTitle(block: HitlBlock | undefined): string {
  if (!block) return 'HITL'
  switch (block.type) {
    case 'form':
      return block.title
    case 'tool_confirm':
      return `${block.tool_name} 确认`
    case 'mode_change':
      return `切换 ${block.to_mode}`
  }
}

function visibleParent(
  parentId: number | null,
  keepIds: Set<number>,
  byId: Map<number, MessageRecord>,
): number | null {
  let cursor = parentId
  while (cursor !== null) {
    if (keepIds.has(cursor)) return cursor
    cursor = byId.get(cursor)?.parent_id ?? null
  }
  return null
}

/** 跳过不可见中间节点，把 parent 接到最近可见祖先上。 */
export function projectVisibleTree(
  raw: MessageRecord[],
  visible: MessageRecord[],
  leaf: number | null,
): { nodes: MessageRecord[]; activeLeaf: number | null } {
  const byId = new Map(raw.map((node) => [node.id, node]))
  const keepIds = new Set(visible.map((node) => node.id))

  const nodes = visible
    .map((node) => ({
      ...node,
      parent_id: visibleParent(node.parent_id, keepIds, byId),
    }))
    .sort((a, b) => a.sort_key - b.sort_key)

  let activeLeaf = leaf
  while (activeLeaf !== null && !keepIds.has(activeLeaf)) {
    activeLeaf = byId.get(activeLeaf)?.parent_id ?? null
  }

  return { nodes, activeLeaf }
}

/**
 * 画一棵树要的尺寸。
 *
 * 由调用方给而不是写在这儿：这几个数必须和面板的 CSS 对得上，属于那一侧的事。放进来的话
 * 改一次样式要动两个文件，而且这个模块就没法用几个好读的小数字来测了。
 */
export interface TreeMetrics {
  /** 节点盒子的宽高。 */
  nodeW: number
  nodeH: number
  /** 同层相邻节点之间、以及上下两层之间的间距。 */
  gapX: number
  gapY: number
  /** 画布四周留白。 */
  pad: number
}

/** 一个摆好位置的节点。`x`/`y` 是它左上角在画布里的坐标。 */
export interface PlacedNode {
  record: MessageRecord
  x: number
  y: number
  /** 在当前分支（从根到 active leaf）上。 */
  onPath: boolean
  /** 没有子节点。 */
  isLeaf: boolean
}

/** 一条父子连线。 */
export interface TreeEdge {
  x1: number
  y1: number
  x2: number
  y2: number
  /** 两端都在当前分支上——只有这样才算「这条路走过」。 */
  onPath: boolean
}

/** 画布内容。`w`/`h` 是把所有节点和留白都装进去所需的尺寸。 */
export interface TreeLayout {
  placed: PlacedNode[]
  edges: TreeEdge[]
  w: number
  h: number
}

/**
 * 把消息树摆成坐标。
 *
 * 布局规则是经典的「自底向上定心」：叶子按出现顺序从左往右一个挨一个排开，父节点摆在它
 * **第一个和最后一个孩子的正中**。这样分叉看起来是对称地分开，而不是全都挤在左边。
 *
 * 传进来的应当是 [`projectVisibleTree`] 处理过的节点——被过滤掉的中间节点已经接到最近的
 * 可见祖先上了。连线那一步仍然会往上找最近的**已摆放**祖先：过滤之后可能出现某个父节点
 * 不在这批里，那时候连线要跨过它接到更上面，否则那一支看起来是断开的孤儿。
 *
 * 纯函数，不碰 DOM 也不碰 Vue：它原先长在 `BranchTree.vue` 的一个 computed 里，于是这段
 * 最该有回归网的几何计算必须挂起整个组件才碰得到。
 */
export function layoutTree(
  nodes: MessageRecord[],
  activeLeaf: number | null,
  metrics: TreeMetrics,
): TreeLayout {
  const { nodeW, nodeH, gapX, gapY, pad } = metrics
  const ordered = [...nodes].sort((a, b) => a.sort_key - b.sort_key)
  const byId = new Map(ordered.map((node) => [node.id, node]))

  const children = new Map<number | null, MessageRecord[]>()
  for (const node of ordered) {
    const list = children.get(node.parent_id) ?? []
    list.push(node)
    children.set(node.parent_id, list)
  }

  // 当前分支：从 active leaf 一路往上
  const onPath = new Set<number>()
  let cursor = activeLeaf
  while (cursor !== null) {
    onPath.add(cursor)
    cursor = byId.get(cursor)?.parent_id ?? null
  }

  const leafIds = new Set<number>()
  for (const node of ordered) {
    if ((children.get(node.id) ?? []).length === 0) leafIds.add(node.id)
  }

  const centers = new Map<number, number>()
  const depths = new Map<number, number>()
  let nextLeafX = 0

  /*
    先序遍历定深度、顺手给叶子排横坐标。

    用显式栈而不是递归：一段线性对话的树深就等于它的消息条数，而递归版在 2000 到 4000 条之
    间就会 RangeError——那不是假想的规模，一段用久了的会话就有几千条。爆栈的位置在一个
    computed 里，表现是分支面板整块空白。
  */
  const stack: { id: number; depth: number }[] = []
  for (const root of [...(children.get(null) ?? [])].reverse()) {
    stack.push({ id: root.id, depth: 0 })
  }
  const visited: number[] = []
  while (stack.length > 0) {
    const { id, depth } = stack.pop()!
    // 树本该是树，但脏数据里的父子环会让这个循环永远转下去——原先的递归版是当场爆栈,
    // 都不好，至少这里不挂住整个界面
    if (depths.has(id)) continue
    depths.set(id, depth)
    visited.push(id)

    const kids = children.get(id) ?? []
    if (kids.length === 0) {
      centers.set(id, nextLeafX + nodeW / 2)
      nextLeafX += nodeW + gapX
      continue
    }
    // 倒着压栈，弹出来才是 sort_key 的顺序
    for (let i = kids.length - 1; i >= 0; i -= 1) {
      stack.push({ id: kids[i]!.id, depth: depth + 1 })
    }
  }

  // 内部节点自底向上定心：按深度从深到浅处理，轮到谁时它的孩子必定已经有中心了
  const internal = visited.filter((id) => (children.get(id) ?? []).length > 0)
  internal.sort((a, b) => depths.get(b)! - depths.get(a)!)
  for (const id of internal) {
    const kids = children.get(id)!
    const first = centers.get(kids[0]!.id)
    const last = centers.get(kids.at(-1)!.id)
    if (first === undefined || last === undefined) continue
    centers.set(id, (first + last) / 2)
  }

  const placed: PlacedNode[] = ordered
    .filter((node) => centers.has(node.id))
    .map((node) => ({
      record: node,
      x: centers.get(node.id)! - nodeW / 2 + pad,
      y: pad + depths.get(node.id)! * (nodeH + gapY),
      onPath: onPath.has(node.id),
      isLeaf: leafIds.has(node.id),
    }))

  const placedById = new Map(placed.map((item) => [item.record.id, item]))
  const edges: TreeEdge[] = []
  for (const item of placed) {
    // 往上找最近的已摆放祖先：中间那些被过滤掉了，连线得跨过去
    let parent = item.record.parent_id
    while (parent !== null && !placedById.has(parent)) {
      parent = byId.get(parent)?.parent_id ?? null
    }
    const from = parent === null ? undefined : placedById.get(parent)
    if (!from) continue
    edges.push({
      x1: from.x + nodeW / 2,
      y1: from.y + nodeH,
      x2: item.x + nodeW / 2,
      y2: item.y,
      onPath: item.onPath && from.onPath,
    })
  }

  // 用 reduce 而不是 Math.max(...arr)：展开一个很长的数组会爆栈，
  // 而这里的长度等于消息条数，是会长的
  const right = placed.reduce((max, item) => Math.max(max, item.x + nodeW), nodeW)
  const bottom = placed.reduce((max, item) => Math.max(max, item.y + nodeH), nodeH)

  return { placed, edges, w: right + pad, h: bottom + pad }
}
