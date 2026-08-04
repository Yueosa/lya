import type { MessageRecord } from '../../api/wire'
import { applySnapshot } from '../../store/session'
import { client } from './client'
import { currentId, state, tree } from './state'

/**
 * 时间线只从树里取分支切换器，用到的就是父子、排序和角色这几样。
 * 这些没变就别重新赋值：`tree` 是 `timeline` 这个 computed 的依赖，一赋值整条时间线
 * 都要重算重渲染，而开分支树面板、开会话设置都会顺手拉一次树。
 */
function sameShape(before: MessageRecord[] | null, after: MessageRecord[]): boolean {
  if (!before || before.length !== after.length) return false
  return before.every((node, index) => {
    const next = after[index]
    return (
      next !== undefined &&
      node.id === next.id &&
      node.parent_id === next.parent_id &&
      node.sort_key === next.sort_key &&
      node.payload.role === next.payload.role
    )
  })
}

/** 写入分支树，结构没变则原样保留。 */
export function setTree(nodes: MessageRecord[]): void {
  if (!sameShape(tree.value, nodes)) tree.value = nodes
}

/** 刷新分支信息。 */
export async function refreshTree(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    setTree((await client.tree(id)).nodes)
  } catch {
    // 拿不到就只是不显示切换器
  }
}

/** 主动拉一次快照。删消息等未广播的改动之后用。 */
export async function refreshSnapshot(): Promise<void> {
  const id = currentId.value
  if (!id) return
  state.value = applySnapshot(state.value, await client.snapshot(id))
}
