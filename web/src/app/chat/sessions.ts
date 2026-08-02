import { report } from './errors'
import { client } from './client'
import { archivedSessions, defaultWorkMode, loading, sessions } from './state'
import { openSession } from './subscription'

/** 拉会话列表。 */
export async function refreshSessions(): Promise<void> {
  loading.value = true
  try {
    const [active, archived] = await Promise.all([
      client.listSessions(),
      client.listArchived(),
    ])
    sessions.value = active
    archivedSessions.value = archived
  } catch (error) {
    report(error, '读取会话列表')
  } finally {
    loading.value = false
  }
}

/** 建一个新会话并打开它。 */
export async function createSession(): Promise<void> {
  try {
    const created = await client.createSession({ work_mode: defaultWorkMode.value })
    sessions.value = [created, ...sessions.value]
    await openSession(created.id)
  } catch (error) {
    report(error, '新建会话')
  }
}
