import type { SessionTree } from '../../api/wire'
import { applySnapshot } from '../../store/session'
import { report } from '../errors'
import { refreshSnapshot, refreshTree, setTree } from './snapshot'
import { client } from '../client'
import { currentId, state, tree } from './state'

/** 换个答法重答上一轮。 */
export async function regenerate(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.regenerate(id)
    await refreshTree()
  } catch (error) {
    report(error, '重新生成')
  }
}

/** 改掉某条用户消息并从那里重开。 */
export async function editAndResend(messageId: number, text: string): Promise<void> {
  const id = currentId.value
  if (!id || !text.trim()) return
  try {
    await client.editAndResend(id, messageId, text)
    await refreshTree()
  } catch (error) {
    report(error, '编辑重发')
  }
}

/** 删掉一个叶节点。 */
export async function deleteMessage(messageId: number): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.deleteMessage(id, messageId)
    await refreshSnapshot()
    await refreshTree()
  } catch (error) {
    report(error, '删除消息')
  }
}

function leafUnder(from: number): number | null {
  const nodes = tree.value
  if (!nodes) return null
  let cursor = from
  for (;;) {
    const kids = nodes
      .filter((node) => node.parent_id === cursor)
      .sort((a, b) => a.sort_key - b.sort_key)
    if (kids.length === 0) return cursor
    cursor = kids.at(-1)!.id
  }
}

/** 切到某个兄弟节点所在分支。 */
export async function switchToBranch(siblingId: number): Promise<void> {
  if (!tree.value) await refreshTree()
  await switchBranch(leafUnder(siblingId) ?? siblingId)
}

/** 切到某个叶节点。 */
export async function switchBranch(leafId: number): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    state.value = applySnapshot(state.value, await client.switchBranch(id, leafId))
    await refreshTree()
  } catch (error) {
    report(error, '切换分支')
  }
}

/** 整棵树，画分支图用。 */
export async function loadTree(): Promise<SessionTree | null> {
  const id = currentId.value
  if (!id) return null
  try {
    const data = await client.tree(id)
    setTree(data.nodes)
    return data
  } catch (error) {
    report(error, '读取分支树')
    return null
  }
}
