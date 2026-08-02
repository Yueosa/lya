/** 聊天时间线块相关的纯函数。 */

import type { Block } from '../../model/timeline'
import { parseFormCall } from '../../utils/parseFormCall'

export function lineCount(text: string): number {
  if (!text) return 0
  return text.split('\n').length
}

export function toolLineCount(block: Block): number {
  const result = block.type === 'tool' ? block.call.result?.content : undefined
  return lineCount(result ?? '')
}

export function formCall(block: Extract<Block, { type: 'tool' }>) {
  if (block.call.name !== 'form') return null
  return parseFormCall(block.call.arguments)
}

export function toolLabel(block: Extract<Block, { type: 'tool' }>): string {
  const form = formCall(block)
  if (form) return `form  ${form.title}`
  const args = block.call.arguments
  if (args && typeof args === 'object') {
    const first = Object.values(args as Record<string, unknown>)[0]
    if (typeof first === 'string' && first) return `${block.call.name}  ${first.slice(0, 60)}`
  }
  return block.call.name
}

export function reasonLabel(reason: { kind: string; message?: string }): string {
  switch (reason.kind) {
    case 'failed':
      return `出错了：${reason.message ?? ''}`
    case 'max_rounds':
      return '工具轮数到上限'
    case 'cancelled':
      return '已停止'
    case 'empty_response':
      return '空回复'
    default:
      return reason.kind
  }
}

export function errorRetryable(reason: { kind: string }): boolean {
  return reason.kind !== 'max_rounds'
}

export function visibleBlocks(blocks: Block[], prefs: {
  hideReasoning: boolean
  hideTools: boolean
  hideResolvedHitl: boolean
}): Block[] {
  return blocks.filter((block) => {
    if (block.type === 'reasoning') return !prefs.hideReasoning
    if (block.type === 'tool') return !prefs.hideTools
    if (block.type === 'hitl') return !(prefs.hideResolvedHitl && block.answer !== undefined)
    return true
  })
}

export function hasText(blocks: Block[]): boolean {
  return blocks.some((block) => block.type === 'text')
}

export function lastTextBlockIndex(blocks: Block[], prefs: {
  hideReasoning: boolean
  hideTools: boolean
  hideResolvedHitl: boolean
}): number {
  const vis = visibleBlocks(blocks, prefs)
  for (let i = vis.length - 1; i >= 0; i--) {
    if (vis[i]?.type === 'text') return i
  }
  return -1
}
