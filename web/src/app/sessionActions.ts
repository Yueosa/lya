/**
 * 对一个会话做点什么：重命名、归档、取消归档、删除。
 *
 * 一个「动作」是三步连在一起的：**问一句、改掉、说一声**。只把中间那步（`rename`、
 * `setArchived`、`removeSession`）抽出来是不够的——前后两步同样是「这个应用怎么对待一个
 * 会话」的一部分，留给每处界面自己拼，就会拼出不一样的。
 *
 * 这不是假想：同样这四个动作原先在右键菜单和会话列表页各写了一遍，而且**已经走偏了八处**
 * ——重命名的对话框一处叫「重命名」一处叫「重命名会话」，一处成功了提示一处不提示；取消
 * 归档一处说「已恢复」一处说「已取消归档」，失败一处说「操作失败」一处说「取消归档失败」；
 * 删除的确认一处只写「不可恢复。」，另一处还额外告诉你「只想收起来的话用归档」。同一个
 * 操作，走按钮和走右键菜单，得到的是两种反馈。
 *
 * 所以入口只留这一处。要加第三种界面（比如命令面板）就调这里，别再抄。
 *
 * 每个函数都自己吞掉失败并弹提示，因此调用方不必 try/catch，也**不该**再补一层提示。
 */

import type { SessionMeta } from '../api/wire'
import { confirm, confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'
import { removeSession, rename, setArchived } from './chat'

/** 会话在界面上的称呼。没标题的那些得有个说法，不然确认框里是一对空引号。 */
function nameOf(session: SessionMeta): string {
  return session.title || '未命名会话'
}

/** 改标题。按了取消、或者只敲了空白，就什么都不做。 */
export async function renameSession(session: SessionMeta): Promise<void> {
  const title = await prompt({ title: '重命名会话', initial: session.title })
  if (title === null || !title.trim()) return
  try {
    await rename(session.id, title.trim())
    toast('已重命名', 'success')
  } catch {
    toast('重命名失败', 'error')
  }
}

/**
 * 收进归档。
 *
 * 要先问一句：归档之后这段对话变成只读，发不了消息也删不了内容（后端也拦着，见
 * `lya-session` 的 `fork_at` / `delete_leaf`）。那是个会让人意外的状态变化。
 */
export async function archiveSession(session: SessionMeta): Promise<void> {
  const ok = await confirm({
    title: '归档这个会话？',
    message: '归档之后它从活跃列表里收起来，仍然可以回看，但不能再发消息。',
  })
  if (!ok) return
  try {
    await setArchived(session.id, true)
    toast('已归档', 'success')
  } catch {
    toast('归档失败', 'error')
  }
}

/** 从归档里放回来。不问——它什么都不丢，点错了再归档回去就行。 */
export async function unarchiveSession(session: SessionMeta): Promise<void> {
  try {
    await setArchived(session.id, false)
    toast('已取消归档', 'success')
  } catch {
    toast('取消归档失败', 'error')
  }
}

/**
 * 连消息一起删掉。
 *
 * 确认框里指一条路：想收起来的人多半是找归档没找到才点到删除的，这时候告诉他比拦住他
 * 有用。走 [`confirmAsync`] 是为了让删除期间的失败显示在框里，而不是框先关掉再弹个提示
 * ——那会让人不确定到底删没删。
 */
export async function deleteSession(session: SessionMeta): Promise<void> {
  await confirmAsync({
    title: `删除「${nameOf(session)}」？`,
    message: '连同全部消息一起从库里去掉，不可恢复。只想收起来的话用「归档」。',
    confirmText: '删除',
    danger: true,
    run: () => removeSession(session.id),
  })
}
