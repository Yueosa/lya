/** 各会话输入框草稿，切页回来不丢。 */
import { reactive } from 'vue'

import { readLocal, writeLocal } from '../../utils/storage'

const KEY_PREFIX = 'lya.composer.draft.'
const drafts = reactive<Record<string, string>>({})

function load(id: string): string {
  return readLocal(`${KEY_PREFIX}${id}`) ?? ''
}

function persist(id: string, text: string): void {
  writeLocal(`${KEY_PREFIX}${id}`, text || null)
}

export function readComposerDraft(sessionId: string | null): string {
  if (!sessionId) return ''
  if (!(sessionId in drafts)) drafts[sessionId] = load(sessionId)
  return drafts[sessionId] ?? ''
}

export function writeComposerDraft(sessionId: string | null, text: string): void {
  if (!sessionId) return
  drafts[sessionId] = text
  persist(sessionId, text)
}
