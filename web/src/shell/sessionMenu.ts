/**
 * 会话行的右键菜单：重命名 / 归档 / 删除。
 *
 * 抽出来是因为不止一套外壳有会话列表——侧栏一份、Momotalk 的联系人栏一份。菜单项的
 * 措辞、确认文案、失败提示都属于「这个应用怎么对待一个会话」，不属于某套皮，抄第二份
 * 就一定会两边走偏。
 */

import type { SessionMeta } from '../api/wire'
import { removeSession, rename, setArchived } from '../app/useChat'
import { openContextMenu } from '../ui/useContextMenu'
import { confirm, confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'

export function openSessionMenu(event: MouseEvent, session: SessionMeta): void {
  const archived = session.status === 'archived'
  openContextMenu(event, [
    {
      label: '重命名',
      icon: 'edit',
      onSelect: async () => {
        const title = await prompt({ title: '重命名', initial: session.title })
        if (title === null || !title.trim()) return
        try {
          await rename(session.id, title.trim())
        } catch {
          toast('重命名失败', 'error')
        }
      },
    },
    archived
      ? {
          label: '取消归档',
          icon: 'unarchive',
          onSelect: async () => {
            try {
              await setArchived(session.id, false)
              toast('已恢复', 'success')
            } catch {
              toast('操作失败', 'error')
            }
          },
        }
      : {
          label: '归档',
          icon: 'archive',
          onSelect: async () => {
            const ok = await confirm({ title: '归档此会话？', message: '归档后只读，可随时恢复。' })
            if (!ok) return
            try {
              await setArchived(session.id, true)
              toast('已归档', 'success')
            } catch {
              toast('归档失败', 'error')
            }
          },
        },
    { separator: true },
    {
      label: '删除',
      icon: 'delete',
      danger: true,
      onSelect: async () => {
        await confirmAsync({
          title: `删除「${session.title || '未命名'}」？`,
          message: '不可恢复。',
          confirmText: '删除',
          danger: true,
          run: () => removeSession(session.id),
        })
      },
    },
  ])
}
