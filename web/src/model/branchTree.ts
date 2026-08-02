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
