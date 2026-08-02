import { applySnapshot } from '../../store/session'
import { client } from './client'
import { currentId, state, tree } from './state'

/** 刷新分支信息。 */
export async function refreshTree(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    tree.value = (await client.tree(id)).nodes
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
