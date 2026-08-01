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
  SessionMeta,
  Snapshot,
  TurnBuffer,
  TurnEndReason,
} from '../api/wire'

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
  /** 正等用户答复的 HITL 节点。 */
  pendingHitlId: number | null
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
        running: { round: event.round, message_id: null, content: '', reasoning: '', calls: [] },
        endReason: null,
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

    case 'message_committed':
      return {
        ...state,
        messages: upsert(state.messages, event.record),
        running: state.running
          ? { ...state.running, message_id: state.running.message_id ?? event.record.id }
          : state.running,
        pendingHitlId: isPendingHitl(event.record) ? event.record.id : state.pendingHitlId,
      }

    case 'message_updated': {
      const messages = upsert(state.messages, event.record)
      return {
        ...state,
        messages,
        // 定稿了就把缓冲里那份丢掉，否则同一段正文会显示两遍
        running: state.running?.message_id === event.record.id ? null : state.running,
        pendingHitlId: isPendingHitl(event.record)
          ? event.record.id
          : state.pendingHitlId === event.record.id
            ? null
            : state.pendingHitlId,
      }
    }

    case 'message_deleted':
      return {
        ...state,
        messages: state.messages.filter((m) => m.id !== event.id),
        running: state.running?.message_id === event.id ? null : state.running,
        pendingHitlId: state.pendingHitlId === event.id ? null : state.pendingHitlId,
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

    case 'await_human':
      return { ...state, pendingHitlId: event.message_id }

    case 'turn_end':
      return {
        ...state,
        endReason: event.reason,
        // 本轮结束，缓冲里该落库的都落了；还留着只会和真消息重影
        running: null,
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

function findPendingHitl(messages: MessageRecord[]): number | null {
  // 只看最后一条：树上同时只可能有一个未决 HITL，后端的 append 会拦住其余的
  const last = messages.at(-1)
  return last && isPendingHitl(last) ? last.id : null
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
