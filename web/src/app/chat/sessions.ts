import type { CreateSession } from '../../api/client'
import { toast } from '../../ui/useToast'
import { client } from './client'
import { report } from '../errors'
import {
  archivedSessions,
  defaultWorkMode,
  defaultApiMode,
  models,
  sessions,
  sessionsLoading,
} from './state'
import { openSession } from './subscription'
import { modelIdForNewSession } from './modelPick'

/** 拉会话列表。 */
export async function refreshSessions(): Promise<void> {
  sessionsLoading.value = true
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
    sessionsLoading.value = false
  }
}

/** 建一个新会话并打开它。 */
export async function createSession(): Promise<void> {
  if (models.value.length === 0) {
    const { loadModels } = await import('./settings')
    await loadModels()
  }
  const api_mode = defaultApiMode.value
  const model_id = modelIdForNewSession(api_mode)
  if (api_mode === 'responses' && model_id === null) {
    toast('没有可用的 Responses 模型，请检查 models.toml 的 modes.responses', 'error')
    return
  }
  try {
    const body: CreateSession = {
      work_mode: defaultWorkMode.value,
    }
    if (model_id !== null) body.model_id = model_id
    const created = await client.createSession(body)
    sessions.value = [created, ...sessions.value]
    await openSession(created.id)
  } catch (error) {
    report(error, '新建会话')
  }
}
