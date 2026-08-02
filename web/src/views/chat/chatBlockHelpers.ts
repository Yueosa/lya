/** 聊天时间线块相关的纯函数。 */

import type { Block, Message } from '../../model/timeline'
import type { MessageRecord, ToolBatchMeta } from '../../api/wire'
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

/** 调用组折叠标题，如「3 个工具 · 2 待确认」。 */
export function toolBatchLabel(batch: ToolBatchMeta, messages: MessageRecord[]): string {
  const total = batch.call_ids.length
  const pending = batch.needs_review.filter((callId) =>
    messages.some(
      (m) =>
        m.payload.role === 'hitl' &&
        m.payload.status === 'pending' &&
        m.payload.lya.meta?.['tool_call_id'] === callId,
    ),
  ).length
  if (pending === 0) return `${total} 个工具`
  return `${total} 个工具 · ${pending} 待确认`
}

export function toolBlocksInMessage(blocks: Block[]): Extract<Block, { type: 'tool' }>[] {
  return blocks.filter((block): block is Extract<Block, { type: 'tool' }> => block.type === 'tool')
}

/** 有调用组时只在第一个 tool 块处渲染折叠外壳。 */
export function isFirstToolBlockInBatch(message: Message, blockIndex: number, blocks: Block[]): boolean {
  if (!message.toolBatch) return false
  for (let i = 0; i < blockIndex; i++) {
    if (blocks[i]?.type === 'tool') return false
  }
  return blocks[blockIndex]?.type === 'tool'
}

export function shouldSkipToolBlock(message: Message, blockIndex: number, blocks: Block[]): boolean {
  if (!message.toolBatch) return false
  if (blocks[blockIndex]?.type !== 'tool') return false
  return !isFirstToolBlockInBatch(message, blockIndex, blocks)
}
