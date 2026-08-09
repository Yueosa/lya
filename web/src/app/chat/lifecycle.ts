import { forgetScrollPosition } from '../../views/chat/useChatScroll'
import { forgetSessionPrefs } from '../usePrefs'
import { report } from '../errors'
import { closeSession } from './subscription'
import { client } from '../client'
import { archivedSessions, currentId, sessions, state } from './state'

/** 归档或取回。 */
export async function setArchived(id: string, archived: boolean): Promise<void> {
  const updated = await client.patchSession(id, { status: archived ? 'archived' : 'active' })
  if (archived) {
    sessions.value = sessions.value.filter((item) => item.id !== id)
    archivedSessions.value = [updated, ...archivedSessions.value.filter((item) => item.id !== id)]
  } else {
    archivedSessions.value = archivedSessions.value.filter((item) => item.id !== id)
    sessions.value = [updated, ...sessions.value.filter((item) => item.id !== id)]
  }
  if (state.value.meta?.id === id) {
    state.value = { ...state.value, meta: updated }
  }
}

/** 真删，不可恢复。 */
export async function removeSession(id: string): Promise<void> {
  await client.deleteSession(id)
  sessions.value = sessions.value.filter((item) => item.id !== id)
  archivedSessions.value = archivedSessions.value.filter((item) => item.id !== id)
  if (currentId.value === id) closeSession()
  forgetSessionPrefs(id)
  forgetScrollPosition(id)
}

/** 改标题。 */
export async function rename(id: string, title: string): Promise<void> {
  const updated = await client.patchSession(id, { title })
  sessions.value = sessions.value.map((item) => (item.id === id ? updated : item))
  archivedSessions.value = archivedSessions.value.map((item) => (item.id === id ? updated : item))
  if (state.value.meta?.id === id) {
    state.value = { ...state.value, meta: updated }
  }
}

/** 改会话身份。 */
export async function setIdentity(identity: string | null): Promise<boolean> {
  const id = currentId.value
  if (!id) return false
  try {
    const updated = await client.patchSession(id, { identity })
    if (state.value.meta?.id === id) {
      state.value = { ...state.value, meta: updated }
    }
    return true
  } catch (error) {
    report(error, '保存身份')
    return false
  }
}

/** 改会话口吻。 */
export async function setStyle(style: string | null): Promise<boolean> {
  const id = currentId.value
  if (!id) return false
  try {
    const updated = await client.patchSession(id, { style })
    if (state.value.meta?.id === id) {
      state.value = { ...state.value, meta: updated }
    }
    return true
  } catch (error) {
    report(error, '保存口吻')
    return false
  }
}
