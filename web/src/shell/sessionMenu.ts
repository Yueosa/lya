/**
 * 会话行的右键菜单：重命名 / 归档 / 删除。
 *
 * 这里只管**菜单长什么样**——有哪几项、什么图标、什么顺序。每一项做什么在
 * `app/sessionActions.ts`，那边和会话列表页共用一份，理由见那个文件开头。
 */

import type { SessionMeta } from '../api/wire'
import {
  archiveSession,
  deleteSession,
  renameSession,
  unarchiveSession,
} from '../app/sessionActions'
import { openContextMenu } from '../ui/useContextMenu'

export function openSessionMenu(event: MouseEvent, session: SessionMeta): void {
  const archived = session.status === 'archived'
  openContextMenu(event, [
    {
      label: '重命名',
      icon: 'edit',
      onSelect: () => renameSession(session),
    },
    archived
      ? {
          label: '取消归档',
          icon: 'unarchive',
          onSelect: () => unarchiveSession(session),
        }
      : {
          label: '归档',
          icon: 'archive',
          onSelect: () => archiveSession(session),
        },
    { separator: true },
    {
      label: '删除',
      icon: 'delete',
      danger: true,
      onSelect: () => deleteSession(session),
    },
  ])
}
