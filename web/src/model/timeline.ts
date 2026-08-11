/**
 * 消息树 → 时间线。
 *
 * # 为什么要分三层
 *
 * 「正文」「思考」「工具调用」是**同一条助手消息内部**的片段——一条助手消息可以
 * 同时带这三样（OpenAI 协议允许 `content` 和 `tool_calls` 同时出现，「说一句话
 * 再继续调工具」就是这么实现的）。而用户消息、HITL 是**独立的树节点**，有 id、
 * 有父子关系。时间分隔**根本不是消息**，后端不存它，是这里按相邻消息的时间差
 * 算出来插进去的。
 *
 * 压成一个扁平的联合类型会立刻卡住：思考和正文该在同一个气泡里，时间分隔没有
 * role 却要混进列表，HITL 居中但它也是有 id 的节点。
 *
 * # 唯一一次实质变换
 *
 * 后端里工具**调用**（`role=assistant, kind=tool_call`）和工具**结果**
 * （`role=tool`）是两条独立消息，因为 OpenAI 协议要求这么发。但界面上它们是
 * 一张折叠卡片，所以这里按 `tool_call_id` 把结果并进调用块，然后把独立的结果
 * 节点从时间线上去掉。除此之外都是直接映射。
 */

import type {
  CallState,
  HitlBlock,
  MessageRecord,
  MessageStatus,
  Mode,
  Role,
  ToolBatchMeta,
  TurnBuffer,
  TurnEndReason,
} from '../api/wire'
import { messageTimeSeparator } from '../utils/dateFormat'

/** 一次工具或动作调用，已经把结果并进来了。 */
export interface ToolCallView {
  callId: string
  name: string
  /** 模型给的原始参数字符串，可能不是合法 JSON。 */
  rawArguments: string
  /** 解析成功才有；失败时为 undefined，界面回退到显示原始串。 */
  arguments?: unknown
  /** 还没拿到结果时为 undefined，界面显示「执行中」。 */
  result?: { ok: boolean; content: string }
  /**
   * 参数暂时取不到（本轮缓冲里只有 call_id 和名字）。
   *
   * 和「模型真的没传参数」是两回事，后者要显眼地报出来。
   */
  argsUnknown?: boolean
}

/** 消息内部的片段。 */
export type Block =
  | { type: 'text'; text: string }
  | { type: 'reasoning'; text: string }
  | { type: 'tool'; call: ToolCallView }
  | {
      type: 'provider_search'
      callId: string
      phase: 'in_progress' | 'searching' | 'completed' | 'failed'
      query?: string
      /** DeepSeek `action.queries`；有多条时 UI 一并展示。 */
      queries?: string[]
    }
  | { type: 'hitl'; hitl: HitlBlock; answer?: unknown }

/** 一条消息，对应树上一个节点。 */
export interface Message {
  id: number
  parentId: number | null
  role: Role
  status: MessageStatus
  /** RFC 3339。 */
  createdAt: string
  blocks: Block[]
  /**
   * 同一个父节点下有几个兄弟、当前是第几个。
   *
   * 只有 `total > 1` 时才会有——那意味着这里分过叉（重新生成过或编辑重发过），
   * 界面要给一个「‹ 2/3 ›」的切换器。**没有它，树就退化成了列表**，后端的
   * `fork_at` / `switch_leaf` 全都用不上。
   */
  branch?: { index: number; total: number; siblingIds: number[] }
  /** 同批 tool_calls 元数据（仅 assistant 且 kind=tool_call 时有）。 */
  toolBatch?: ToolBatchMeta
}

/** 能出现在滚动列表里的东西。 */
export type TimelineItem =
  /** 一条消息。 */
  | { kind: 'message'; message: Message }
  /** 距上一条消息隔得久了，插一行小灰字。 */
  | { kind: 'time-gap'; at: string; text: string }
  /** 系统标记，如「用户把工作模式切换为 agent」。居中小灰字。 */
  | { kind: 'notice'; at: string; text: string }
  /**
   * 出错了。
   *
   * 不渲染这个的话失败就是静默：界面转个圈，然后什么都没发生。配错密钥是第一次
   * 运行最常见的情况，所以这条一定会被用到。
   */
  | { kind: 'error'; reason: TurnEndReason }

/** 组装时间线要的东西。 */
export interface TimelineInput {
  /** 当前分支从根到叶。 */
  messages: MessageRecord[]
  /** 整棵树，只用来算分支切换器；没有就不显示切换器。 */
  tree?: MessageRecord[]
  /** 正在跑的那一轮，还没落库的内容。 */
  running?: TurnBuffer | null
  /** 上一轮的结束原因，非正常结束时会渲染成错误项。 */
  endReason?: TurnEndReason | null
}

/** 把后端的消息树拼成可以直接渲染的时间线。 */
export function buildTimeline(input: TimelineInput): TimelineItem[] {
  const results = collectToolResults(input.messages)
  const siblings = indexSiblings(input.tree)

  const items: TimelineItem[] = []
  let previousAt: string | null = null

  for (const record of input.messages) {
    // 工具结果已经并进了对应的调用块，别再单独占一行
    if (record.payload.role === 'tool') continue

    // 系统消息不是对话，是「为什么助手的行为边界变了」的说明
    if (record.payload.role === 'system') {
      items.push({
        kind: 'notice',
        at: record.created_at,
        text: record.payload.openai?.content ?? '',
      })
      continue
    }

    const gap = messageTimeSeparator(previousAt, record.created_at)
    if (gap) items.push({ kind: 'time-gap', at: record.created_at, text: gap })
    previousAt = record.created_at

    const message = toMessage(record, results, siblings)

    // 正在写的那条：占位消息已经落库但正文是空的，真正在长的字在缓冲里。
    // 不覆盖的话界面上就只有一个空气泡——「没有流式输出」就是这么来的。
    // 工具结果可能已经落库，但缓冲里的 call 还是 ok=null——必须把 results 并进来，
    // 否则一直显示「执行中」，刷新（running 清空）后才正常。
    if (input.running && input.running.message_id === record.id) {
      const live = runningBlocks(input.running, results, record)
      if (live.length > 0) message.blocks = live
      message.status = 'streaming'
    }

    items.push({ kind: 'message', message })
  }

  const orphan = orphanRunning(input.running, input.messages)
  if (orphan) items.push({ kind: 'message', message: orphan })

  if (input.endReason && isFailure(input.endReason)) {
    items.push({ kind: 'error', reason: input.endReason })
  }
  return items
}

/** `completed` 和 `awaiting_human` 是正常收尾，其余都要告诉用户。 */
function isFailure(reason: TurnEndReason): boolean {
  return reason.kind !== 'completed' && reason.kind !== 'awaiting_human'
}

/** 按 `tool_call_id` 归拢工具结果。 */
function collectToolResults(
  messages: MessageRecord[],
): Map<string, { ok: boolean; content: string }> {
  const map = new Map<string, { ok: boolean; content: string }>()
  for (const record of messages) {
    if (record.payload.role !== 'tool') continue
    const openai = record.payload.openai
    if (!openai?.tool_call_id) continue
    map.set(openai.tool_call_id, {
      // 工具自己的失败也会照常回灌给模型，所以走到这里的都算「拿到了结果」；
      // 成不成功看内容，不看有没有结果
      ok: record.payload.status !== 'interrupted',
      // 压缩后 openai.content 是占位；界面优先展示 full_content
      content: record.payload.lya.full_content ?? openai.content,
    })
  }
  return map
}

/** 建一张「父节点 → 子节点们」的表，用来算分支切换器。 */
function isBranchNode(record: MessageRecord): boolean {
  const role = record.payload.role
  return role === 'user' || role === 'assistant'
}

function indexSiblings(tree?: MessageRecord[]): Map<number | null, number[]> {
  const map = new Map<number | null, number[]>()
  if (!tree) return map
  const sorted = [...tree].sort((a, b) => a.sort_key - b.sort_key)
  for (const node of sorted) {
    if (!isBranchNode(node)) continue
    const key = node.parent_id
    const list = map.get(key)
    if (list) list.push(node.id)
    else map.set(key, [node.id])
  }
  return map
}

function toMessage(
  record: MessageRecord,
  results: Map<string, { ok: boolean; content: string }>,
  siblings: Map<number | null, number[]>,
): Message {
  const message: Message = {
    id: record.id,
    parentId: record.parent_id,
    role: record.payload.role,
    status: record.payload.status,
    createdAt: record.created_at,
    blocks: toBlocks(record, results),
  }

  const batchRaw = record.payload.lya.meta?.['tool_batch']
  if (batchRaw && typeof batchRaw === 'object') {
    message.toolBatch = batchRaw as ToolBatchMeta
  }

  const group = siblings.get(record.parent_id)
  if (group && group.length > 1) {
    message.branch = {
      index: group.indexOf(record.id),
      total: group.length,
      siblingIds: group,
    }
  }
  return message
}

function toBlocks(
  record: MessageRecord,
  results: Map<string, { ok: boolean; content: string }>,
): Block[] {
  const { payload } = record
  const blocks: Block[] = []

  // 思考排在正文前面：模型就是先想后说的，倒过来读着别扭
  if (payload.lya.reasoning) {
    blocks.push({ type: 'reasoning', text: payload.lya.reasoning })
  }

  for (const item of payload.lya.responses_items ?? []) {
    const block = responsesItemToBlock(item)
    if (block) blocks.push(block)
  }

  const content = payload.openai?.content
  if (content) blocks.push({ type: 'text', text: content })

  for (const call of payload.openai?.tool_calls ?? []) {
    blocks.push({ type: 'tool', call: toCallView(call.id, call.function.name, call.function.arguments, results.get(call.id)) })
  }

  if (payload.lya.hitl) {
    const block: Block = { type: 'hitl', hitl: payload.lya.hitl }
    const answer = payload.lya.meta?.['answer']
    // 已答复的 HITL 要能原样回显当时勾了什么，而不是从渲染后的文本里反解
    if (answer !== undefined) Object.assign(block, { answer })
    blocks.push(block)
  }
  return blocks
}

function responsesItemToBlock(item: unknown): Block | null {
  if (!item || typeof item !== 'object') return null
  const rec = item as Record<string, unknown>
  if (rec.type !== 'web_search_call') return null
  const callId =
    typeof rec.id === 'string'
      ? rec.id
      : typeof rec.call_id === 'string'
        ? rec.call_id
        : 'native'
  const status = typeof rec.status === 'string' ? rec.status : 'completed'
  const action = rec.action
  let queries: string[] | undefined
  if (action && typeof action === 'object') {
    const act = action as { query?: unknown; queries?: unknown }
    if (Array.isArray(act.queries)) {
      const list = act.queries.filter((q): q is string => typeof q === 'string' && q.length > 0)
      if (list.length > 0) queries = list
    }
    if (!queries?.length && typeof act.query === 'string' && act.query) {
      queries = [act.query]
    }
  }
  return providerSearchBlock(callId, webSearchPhase(status), queries)
}

function providerSearchBlock(
  callId: string,
  phase: 'in_progress' | 'searching' | 'completed' | 'failed',
  queries?: string[],
): Extract<Block, { type: 'provider_search' }> {
  const block: Extract<Block, { type: 'provider_search' }> = {
    type: 'provider_search',
    callId,
    phase,
  }
  if (queries?.length) {
    block.queries = queries
    const first = queries[0]
    if (first) block.query = first
  }
  return block
}

function webSearchPhase(
  status: string,
): 'in_progress' | 'searching' | 'completed' | 'failed' {
  switch (status) {
    case 'in_progress':
      return 'in_progress'
    case 'searching':
      return 'searching'
    case 'failed':
      return 'failed'
    default:
      return 'completed'
  }
}

function toCallView(
  callId: string,
  name: string,
  rawArguments: string,
  result?: { ok: boolean; content: string },
): ToolCallView {
  const view: ToolCallView = { callId, name, rawArguments }
  try {
    view.arguments = JSON.parse(rawArguments)
  } catch {
    // 模型偶尔会给出不合法的 JSON，那就让界面显示原始串，不要整条消息渲染失败
  }
  if (result) view.result = result
  return view
}

/**
 * 把本轮缓冲拼成块。
 *
 * 缓冲里是**还没落库**的内容。流式开始时后端先落一条空占位好让界面有 id 可挂，
 * 所以那条消息虽然在树上，正文却是空的——真正在长的字在缓冲里。谁更新就用谁。
 */
/** 快照里后端写 `success`，SSE 增量写 `ok`，统一成一种读法。 */
function callOk(call: CallState & { success?: boolean | null }): boolean | null {
  if (call.ok === true || call.ok === false) return call.ok
  if (call.success === true || call.success === false) return call.success
  return null
}

function persistedToolCalls(
  record: MessageRecord | undefined,
  results: Map<string, { ok: boolean; content: string }>,
): Map<string, ToolCallView> {
  const map = new Map<string, ToolCallView>()
  if (!record) return map
  for (const call of record.payload.openai?.tool_calls ?? []) {
    map.set(
      call.id,
      toCallView(call.id, call.function.name, call.function.arguments, results.get(call.id)),
    )
  }
  return map
}

function runningBlocks(
  running: TurnBuffer,
  results: Map<string, { ok: boolean; content: string }>,
  record?: MessageRecord,
): Block[] {
  const blocks: Block[] = []
  if (running.reasoning) blocks.push({ type: 'reasoning', text: running.reasoning })
  for (const search of running.provider_searches ?? []) {
    blocks.push(
      providerSearchBlock(
        search.call_id,
        search.phase,
        search.query ? [search.query] : undefined,
      ),
    )
  }
  if (running.content) blocks.push({ type: 'text', text: running.content })

  const persisted = persistedToolCalls(record, results)
  const seen = new Set<string>()
  for (const call of running.calls) {
    seen.add(call.call_id)
    blocks.push({
      type: 'tool',
      call: runningCall(call, results, persisted.get(call.call_id)),
    })
  }
  for (const [callId, view] of persisted) {
    if (seen.has(callId)) continue
    blocks.push({ type: 'tool', call: view })
  }
  return blocks
}

/**
 * 缓冲对不上任何已有消息时，拼一条临时的挂在末尾。
 *
 * 正常流程里占位消息总是先落库，所以走不到这里；但事件乱序或漏掉时，
 * 有内容总比什么都不显示强。
 */
function orphanRunning(
  running: TurnBuffer | null | undefined,
  messages: MessageRecord[],
): Message | null {
  if (!running) return null
  if (running.message_id !== null && messages.some((m) => m.id === running.message_id)) {
    return null
  }
  const blocks = runningBlocks(running, collectToolResults(messages))
  if (blocks.length === 0) return null

  return {
    // 还没入库就没有真 id，用负数占位，界面按 key 渲染时不会和真节点撞上
    id: -1,
    parentId: messages.at(-1)?.id ?? null,
    role: 'assistant',
    status: 'streaming',
    createdAt: new Date().toISOString(),
    blocks,
  }
}

function runningCall(
  call: CallState & { success?: boolean | null },
  results: Map<string, { ok: boolean; content: string }>,
  persisted?: ToolCallView,
): ToolCallView {
  const fromDb = results.get(call.call_id)
  if (fromDb) {
    return toCallView(
      call.call_id,
      call.name,
      persisted?.rawArguments ?? '',
      fromDb,
    )
  }

  const view: ToolCallView = {
    callId: call.call_id,
    name: call.name,
    rawArguments: persisted?.rawArguments ?? '',
    argsUnknown: persisted ? !persisted.rawArguments : true,
  }
  if (persisted?.arguments !== undefined) view.arguments = persisted.arguments

  // ok 为 null 表示还在跑，这时不给 result，界面显示「执行中」
  const ok = callOk(call)
  if (ok !== null) view.result = { ok, content: '' }
  return view
}

/** 会话头部要显示的东西。 */
export interface SessionHeader {
  title: string
  mode: Mode
  modelId: string | null
}
