/** 各会话输入框草稿，切页回来不丢。 */
import { reactive } from 'vue'

const KEY_PREFIX = 'lya.composer.draft.'
const drafts = reactive<Record<string, string>>({})

function load(id: string): string {
  try {
    return localStorage.getItem(`${KEY_PREFIX}${id}`) ?? ''
  } catch {
    return ''
  }
}

function persist(id: string, text: string): void {
  try {
    if (text) localStorage.setItem(`${KEY_PREFIX}${id}`, text)
    else localStorage.removeItem(`${KEY_PREFIX}${id}`)
  } catch {
    // localStorage 不可用时仅保留内存草稿
  }
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
