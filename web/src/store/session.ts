/**
 * 会话状态：快照做种子，事件往前推。
 *
 * # 为什么单独一层，而不是塞进 API 客户端
 *
 * 要累积的**不只是本轮缓冲**——`buildTimeline` 需要 `messages`、`tree`、
 * `running`、`endReason`，它们随同一条事件流一起变，还互相牵连（消息一落库，
 * 既要动消息列表，又要清掉缓冲里对应的部分）。而且这些状态**有两个来源**：
 * 订阅时的 HTTP 快照和之后的 SSE 增量，两条路必须收敛到同一个形状。
 *
 * 让 API 客户端来干这些，它就不再是 API 客户端了，而是一个挂了 `fetch` 的
 * store——层数没少，只是名字起错了。
 *
 * 所以这里只做**解释**（事件 → 状态），传输层只做**解析**（字节 → 事件）。
 * 分开的直接好处是 [`applyEvent`] 是纯函数：整个流式生命周期可以拿一串事件
 * 跑完，不碰网络，也不用伪造 EventSource。
 */

import type {
  CallState,
  LyaEvent,
  MessageRecord,
  ProviderSearchState,
  SessionMeta,
  Snapshot,
  ToolBatchCall,
  TurnBuffer,
  TurnEndReason,
} from '../api/wire'

/** 流式阶段收到的调用组摘要。 */
export interface ActiveToolBatch {
  batchId: string
  messageId: number
  calls: ToolBatchCall[]
}

/** 一个会话在前端的全部状态。 */
export interface SessionState {
  meta: SessionMeta | null
  /** 当前分支从根到叶。 */
  messages: MessageRecord[]
  /** 整棵树；只在需要分支切换器时拉，不跟着事件更新。 */
  tree: MessageRecord[] | null
  /** 本轮还没落库的部分。 */
  running: TurnBuffer | null
  /** 上一轮怎么结束的。 */
  endReason: TurnEndReason | null
  /** 正等用户答复的 HITL 节点（同批里取路径上第一条 pending）。 */
  pendingHitlId: number | null
  /** 本轮 SSE 收到的调用组；落库后以 assistant 上的 meta 为准。 */
  activeToolBatch: ActiveToolBatch | null
}

/** 什么都还没有的初始状态。 */
export function emptyState(): SessionState {
  return {
    meta: null,
    messages: [],
    tree: null,
    running: null,
    endReason: null,
    pendingHitlId: null,
    activeToolBatch: null,
  }
}

/**
 * 用快照重置状态。
 *
 * 首次订阅和断线重连走的是同一条路——快照是幂等的，所以这里直接整体替换，
 * 不需要和已有状态做差量合并。
 */
export function applySnapshot(state: SessionState, snapshot: Snapshot): SessionState {
  return {
    ...state,
    meta: snapshot.session,
    messages: snapshot.messages,
    running: snapshot.running,
    // 快照没带这两个，但它们描述的是「刚刚发生了什么」，重连后本就不该沿用旧的
    endReason: null,
    pendingHitlId: findPendingHitl(snapshot.messages),
    activeToolBatch: null,
  }
}

/** 事件推进一步。纯函数，不改入参。 */
export function applyEvent(state: SessionState, event: LyaEvent): SessionState {
  switch (event.type) {
    case 'round_started':
      return {
        ...state,
        // 新一轮的正文从头开始，上一轮的已经落库了；调用列表也跟着重置，
        // 否则上一轮的工具会一直挂在界面上
        running: {
          round: event.round,
          message_id: null,
          content: '',
          reasoning: '',
          calls: [],
          provider_searches: [],
        },
        endReason: null,
        activeToolBatch: null,
      }

    case 'message_delta':
      return withRunning(state, (running) => ({
        ...running,
        content: running.content + event.text,
      }))

    case 'reasoning_delta':
      return withRunning(state, (running) => ({
        ...running,
        reasoning: running.reasoning + event.text,
      }))

    case 'message_committed': {
      const messages = upsert(state.messages, event.record)
      return {
        ...state,
        messages,
        running: state.running
          ? { ...state.running, message_id: state.running.message_id ?? event.record.id }
          : state.running,
        pendingHitlId: findPendingHitl(messages),
      }
    }

    case 'message_updated': {
      const messages = upsert(state.messages, event.record)
      return {
        ...state,
        messages,
        // 定稿了就把缓冲里那份丢掉，否则同一段正文会显示两遍
        running: state.running?.message_id === event.record.id ? null : state.running,
        pendingHitlId: findPendingHitl(messages),
      }
    }

    case 'message_deleted': {
      const messages = state.messages.filter((m) => m.id !== event.id)
      return {
        ...state,
        messages,
        running: state.running?.message_id === event.id ? null : state.running,
        pendingHitlId: findPendingHitl(messages),
      }
    }

    case 'call_started':
      return withRunning(state, (running) => ({
        ...running,
        calls: [
          ...running.calls,
          { call_id: event.call_id, name: event.name, kind: event.kind, ok: null },
        ],
      }))

    case 'call_finished':
      return withRunning(state, (running) => ({
        ...running,
        calls: running.calls.map((call) =>
          call.call_id === event.call_id ? { ...call, ok: event.success } : call,
        ),
      }))

    case 'tool_batch_started':
      return {
        ...state,
        activeToolBatch: {
          batchId: event.batch_id,
          messageId: event.message_id,
          calls: event.calls,
        },
      }

    case 'await_human': {
      const pendingHitlId = findPendingHitl(state.messages) ?? event.message_id
      return { ...state, pendingHitlId }
    }

    case 'provider_search':
      return withRunning(state, (running) => {
        const searches = [...(running.provider_searches ?? [])]
        const index = searches.findIndex((s) => s.call_id === event.call_id)
        const next: ProviderSearchState = {
          call_id: event.call_id,
          phase: event.phase,
        }
        if (event.query !== undefined) next.query = event.query
        if (index >= 0) searches[index] = next
        else searches.push(next)
        return { ...running, provider_searches: searches }
      })

    case 'turn_end':
      return {
        ...state,
        endReason: event.reason,
        // 本轮结束，缓冲里该落库的都落了；还留着只会和真消息重影
        running: null,
        activeToolBatch: null,
      }
  }
}

/** 一次性喂一串事件，测试和重放都用它。 */
export function applyEvents(state: SessionState, events: LyaEvent[]): SessionState {
  return events.reduce(applyEvent, state)
}

/**
 * 改本轮缓冲。
 *
 * 没有缓冲说明没有轮次在跑——那多半是乱序到达的残留事件，忽略比凭空造一个
 * 缓冲要好，后者会让界面冒出一条来历不明的消息。
 */
function withRunning(
  state: SessionState,
  update: (running: TurnBuffer) => TurnBuffer,
): SessionState {
  if (!state.running) return state
  return { ...state, running: update(state.running) }
}

/** 有则替换、无则按 `sort_key` 插入。 */
function upsert(messages: MessageRecord[], record: MessageRecord): MessageRecord[] {
  const index = messages.findIndex((m) => m.id === record.id)
  if (index >= 0) {
    const next = [...messages]
    next[index] = record
    return next
  }
  const next = [...messages, record]
  next.sort((a, b) => a.sort_key - b.sort_key)
  return next
}

function isPendingHitl(record: MessageRecord): boolean {
  return record.payload.role === 'hitl' && record.payload.status === 'pending'
}

/** 从根往叶找第一条 pending HITL，与同批审阅顺序一致。 */
function findPendingHitl(messages: MessageRecord[]): number | null {
  for (const record of messages) {
    if (isPendingHitl(record)) return record.id
  }
  return null
}

/** 本轮是不是还在跑。 */
export function isRunning(state: SessionState): boolean {
  return state.running !== null
}

/** 现在能不能发消息。 */
export function canSend(state: SessionState): boolean {
  return !isRunning(state) && state.pendingHitlId === null
}

export type { CallState }
