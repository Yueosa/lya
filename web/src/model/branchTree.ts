import type { MessageRecord } from '../api/wire'

/** 分支树里要画出来的节点：有正文的 user / assistant。 */
export function treeNode(record: MessageRecord): boolean {
  const text = record.payload.openai?.content?.trim()
  if (!text) return false
  return record.payload.role === 'user' || record.payload.role === 'assistant'
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
