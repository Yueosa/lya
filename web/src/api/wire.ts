/**
 * 后端原样发过来的形状。
 *
 * 这些类型必须和 Rust 侧逐字段对齐——它们是照着
 * `cargo run -p lya-core --example wire` 打出的真实 JSON 写的，不是照结构体手抄
 * 的。serde 的重命名、`skip_serializing_if`、枚举 tag 形式手抄很容易错，而这类
 * 偏差要到联调时才炸。改后端消息结构后请重跑那个 example 核对。
 *
 * 这一层**不做任何加工**，加工在 `model/timeline.ts`。分开是因为线上格式由后端
 * 决定，而界面要的形状由界面决定，两者的演进节奏不一样。
 */

/** 消息在树里的角色。 */
export type Role = 'user' | 'assistant' | 'system' | 'tool' | 'hitl'

/** 消息细类。 */
export type MessageKind =
  | 'chat'
  | 'tool_call'
  | 'tool_result'
  | 'form'
  | 'tool_confirm'
  | 'mode_change'
  | 'hitl_response'

/** 消息生命周期。 */
export type MessageStatus =
  | 'streaming'
  | 'pending'
  | 'complete'
  | 'interrupted'
  | 'resolved'

/** 工作模式。 */
export type Mode = 'ask' | 'edit' | 'agent'

/** OpenAI 协议里的一次函数调用。 */
export interface OpenAiToolCall {
  id: string
  type: 'function'
  function: {
    name: string
    /** 未解析的 JSON 字符串——模型可能给出不合法的 JSON，所以保持原样。 */
    arguments: string
  }
}

/** 内嵌的 OpenAI 兼容消息体。HITL 节点没有这一段。 */
export interface OpenAiMessage {
  role: string
  content: string
  tool_calls?: OpenAiToolCall[]
  tool_call_id?: string
}

/** 表单选项。 */
export interface FormOption {
  /** 提交时回传的值。 */
  key: string
  label: string
}

/** 表单里的一道题。 */
export interface FormQuestion {
  id: string
  text: string
  kind: 'single' | 'multi' | 'text'
  /** 文本题为空。 */
  options?: FormOption[]
  /** 是否额外给一个备注输入框。 */
  allow_note?: boolean
}

/** 工具确认里拆出的一步。 */
export interface ConfirmStep {
  raw: string
  explain: string
  /** 没有风险时字段不出现。 */
  risk?: string
  /** 与上一段的关系，如「成功后」。 */
  connector: string
}

/** 需要用户介入的三种情形。 */
export type HitlBlock =
  | { type: 'form'; form_id: string; title: string; questions: FormQuestion[] }
  | {
      type: 'tool_confirm'
      tool_call_id: string
      tool_name: string
      /** 已解析的参数对象——放行后照它执行，所以后端存的是结构化的。 */
      arguments: unknown
      summary: string
      steps?: ConfirmStep[]
      reasons?: string[]
    }
  | { type: 'mode_change'; to_mode: Mode; reason: string }

/** lya 自己的扩展字段。 */
export interface LyaExtras {
  /** 思考全文。落库但不回灌给模型，只用于展示。 */
  reasoning?: string
  /**
   * 后端预留的块列表。
   *
   * **目前它只是 `openai.content` 的镜像**，两边内容完全一样。前端一律以
   * `openai.content` 为准，不读这里——同时读两个会说不清哪个是权威。等后端真的
   * 往里放 content 表达不了的东西（比如富文本卡片）再接。
   */
  blocks?: unknown[]
  hitl?: HitlBlock
  /** 杂项。HITL 解决后用户的原始作答存在 `meta.answer`。 */
  meta?: Record<string, unknown>
}

/** 一条消息的完整载荷。 */
export interface MessagePayload {
  /** schema 版本。 */
  v: number
  role: Role
  kind: MessageKind
  status: MessageStatus
  openai?: OpenAiMessage
  lya: LyaExtras
}

/** 树上的一个节点。 */
export interface MessageRecord {
  id: number
  session_id: string
  /** 根节点为 null。 */
  parent_id: number | null
  /** 会话内递增，给整棵树一个稳定的时间序。 */
  sort_key: number
  payload: MessagePayload
  /** RFC 3339。 */
  created_at: string
}

/** 会话元数据。 */
export interface SessionMeta {
  id: string
  title: string
  status: 'active' | 'archived'
  active_leaf_id: number | null
  work_mode: Mode
  persona: string | null
  /** null 表示用配置里的默认模型。 */
  model_id: string | null
  /** null 表示启用全部工具。 */
  enabled_tools: string[] | null
  created_at: string
  updated_at: string
}

/** 正在跑的那一轮里，一次调用的状态。 */
export interface CallState {
  call_id: string
  name: string
  kind: 'tool' | 'action'
  /** 还在跑时为 null。 */
  ok: boolean | null
}

/**
 * 正在跑的那一轮的实时缓冲。
 *
 * 这些内容还没落库，靠它，刷新页面或换台设备也能接着看当前这轮。
 */
export interface TurnBuffer {
  round: number
  /** 正在写的那条消息节点 id；刚开始时可能还没有。 */
  message_id: number | null
  content: string
  reasoning: string
  calls: CallState[]
}

/** 订阅会话时先收到的快照。 */
export interface Snapshot {
  session: SessionMeta
  /** 当前分支从根到叶，不是整棵树。 */
  messages: MessageRecord[]
  running: TurnBuffer | null
}

/** 整棵树，画分叉图和算分支切换器要用。 */
export interface SessionTree {
  active_leaf_id: number | null
  /** 所有分支端点。 */
  leaves: number[]
  /** 全部节点，按 sort_key 正序。 */
  nodes: MessageRecord[]
}

/** 一轮结束的原因。 */
export type TurnEndReason =
  | { kind: 'completed' }
  | { kind: 'awaiting_human' }
  | { kind: 'max_rounds' }
  | { kind: 'cancelled' }
  | { kind: 'empty_response' }
  | { kind: 'failed'; message: string }

/**
 * SSE 推过来的事件。
 *
 * `message_committed` / `message_updated` / `message_deleted` 三个带着库里发生的
 * 事实，其余是本轮的增量。有了这三个，拿快照起步之后光靠事件流就能维护完整
 * 状态，不必回拉——回拉除了多一次往返，还会让「拉取在路上时新增量到达」变成
 * 一个要处理的竞态。
 */
export type LyaEvent =
  | { type: 'round_started'; round: number }
  | { type: 'reasoning_delta'; text: string }
  | { type: 'message_delta'; text: string }
  | { type: 'message_committed'; record: MessageRecord }
  | { type: 'message_updated'; record: MessageRecord }
  | { type: 'message_deleted'; id: number }
  | { type: 'call_started'; call_id: string; name: string; kind: 'tool' | 'action' }
  | { type: 'call_finished'; call_id: string; name: string; success: boolean }
  | { type: 'await_human'; message_id: number }
  | { type: 'turn_end'; reason: TurnEndReason }

/** 事件信封。 */
export interface Envelope {
  /** `session:<id>` 或 `global`。 */
  scope: string
  type: LyaEvent['type']
  /**
   * 递增序号。
   *
   * 只用于排查问题，**不要依赖它对齐**——快照本身幂等，重连和首次连接走同一
   * 条路，不需要事件重放。
   */
  seq: number
  payload: Record<string, unknown>
}
