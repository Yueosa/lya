import type { Memory } from '../api/client'
import type { ModelInfo } from '../api/client'
import type { SessionMeta } from '../api/wire'

const FALLBACKS = [
  'Also try 对话列表！',
  'Also try 写一条记忆！',
  '喵~',
  '今天也要开心聊天！',
]

/** 从会话 / 记忆 / 模型拼出 MC 风格 splash 候选。 */
export function buildSplashLines(
  sessions: SessionMeta[],
  archived: SessionMeta[],
  memories: Memory[],
  modelList: ModelInfo[],
): string[] {
  const lines: string[] = []

  for (const session of [...sessions, ...archived]) {
    const title = session.title?.trim() || '未命名会话'
    lines.push(`试试「${title}」！`)
    lines.push(`Also try ${title}!`)
  }
  for (const memory of memories) {
    lines.push(`记得「${memory.title}」`)
    lines.push(`Also try ${memory.title}!`)
  }
  for (const model of modelList) {
    lines.push(`Also try ${model.name}!`)
    lines.push(`试试模型 ${model.name}`)
  }

  return lines.length ? lines : FALLBACKS
}

export function pickSplash(lines: string[], seed = Math.random()): string {
  if (!lines.length) return FALLBACKS[0]!
  const index = Math.floor(seed * lines.length) % lines.length
  return lines[index]!
}

/** 主菜单左下角：活跃数据概览。 */
export function menuFootLeft(
  sessions: SessionMeta[],
  archived: SessionMeta[],
  memories: Memory[],
): string {
  return `lya · ${sessions.length} 活跃 · ${archived.length} 归档 · ${memories.length} 记忆`
}

/** 主菜单右下角：模型与版本感。 */
export function menuFootRight(modelList: ModelInfo[]): string {
  const n = modelList.length
  return `${n} 个模型 · Minecraft Edition`
}
