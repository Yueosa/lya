/** 聊天时间线块相关的纯函数。 */

import type { Block, Message, ToolCallView } from '../../model/timeline'
import type { MessageRecord, ToolBatchMeta } from '../../api/wire'
import { parseFormCall } from '../../utils/parseFormCall'

export function lineCount(text: string): number {
  if (!text) return 0
  return text.split('\n').length
}

export function toolLineCount(block: Block): number {
  if (block.type !== 'tool') return 0
  const args = toolArgsText(block.call)
  return lineCount(block.call.result?.content ?? '') + (args ? lineCount(args) + 1 : 0)
}

/**
 * 模型实际发出的参数，供界面原样展示。
 *
 * `null` = 现在还看不到（流式缓冲里没有参数），不渲染这一段。
 * 空字符串参数会明确写出来——不然「模型漏传参数」在界面上和正常调用长得一样。
 */
export function toolArgsText(call: ToolCallView): string | null {
  if (call.argsUnknown) return null
  const raw = call.rawArguments.trim()
  if (!raw) return '(模型未传参数)'
  if (call.arguments !== undefined) {
    try {
      return JSON.stringify(call.arguments, null, 2)
    } catch {
      // 拿不动就退回原始串，总比整块渲染不出来强
    }
  }
  return raw
}

/** 参数缺失或解析不了：这次调用基本注定失败，标出来。 */
export function toolArgsBroken(call: ToolCallView): boolean {
  if (call.argsUnknown) return false
  return call.rawArguments.trim() === '' || call.arguments === undefined
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

export function reasonLabel(reason: {
  kind: string
  message?: string
  count?: number
  last_tool?: string
}): string {
  switch (reason.kind) {
    case 'failed':
      return `出错了：${reason.message ?? ''}`
    case 'max_rounds':
      return '工具轮数到上限'
    case 'tool_failure_loop':
      return `${reason.last_tool ?? '工具'} 连续失败 ${reason.count ?? 0} 次，已中止`
    case 'cancelled':
      return '已停止'
    case 'empty_response':
      return '空回复'
    default:
      return reason.kind
  }
}

export function errorRetryable(reason: { kind: string }): boolean {
  // 这两种重试大概率还是同样的结果，先让人去改配置或改提法
  return reason.kind !== 'max_rounds' && reason.kind !== 'tool_failure_loop'
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

export function providerSearchLabel(
  block: Extract<Block, { type: 'provider_search' }>,
): string {
  const q = formatSearchQueries(block)
  switch (block.phase) {
    case 'in_progress':
      return '正在准备搜索…'
    case 'searching':
      return q ? `正在搜索：${q}` : '正在搜索…'
    case 'completed':
      return q ? `搜索完成：${q}` : '搜索完成'
    case 'failed':
      return q ? `搜索失败：${q}` : '搜索失败'
  }
}

/** 原生搜索块的可读关键词（支持 `queries` 数组与旧 `query` 字段）。 */
export function formatSearchQueries(
  block: Pick<Extract<Block, { type: 'provider_search' }>, 'query' | 'queries'>,
): string | null {
  if (block.queries?.length) return block.queries.join(' · ')
  if (block.query) return block.query
  return null
}
