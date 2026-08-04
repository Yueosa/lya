import type { ApiMode } from '../../api/wire'
import { defaultModel, models } from './state'

/** 为指定 API 栈挑一个可用模型 id。 */
export function modelIdForNewSession(apiMode: ApiMode): string | null {
  const defaultId = defaultModel.value?.id
  if (defaultId) {
    const entry = models.value.find((m) => m.id === defaultId)
    if (entry?.modes[apiMode] && !entry.api_key_placeholder) return defaultId
  }
  const pick = models.value.find((m) => m.modes[apiMode] && !m.api_key_placeholder)
  return pick?.id ?? null
}
