/**
 * HTTP 传输层。
 *
 * 这里**只做解析**：把请求发出去、把响应变成带类型的对象。事件怎么改变状态是
 * `store/session.ts` 的事。两者看起来都叫「处理事件」，其实一个是传输、一个是
 * 领域逻辑，混在一起这层就会长成一个挂了 `fetch` 的 store。
 *
 * 路径和请求体都照着 `crates/lya-api/src/http/mod.rs` 的路由表写，形状照着
 * `cargo run -p lya-api --example wire` 打出的真实 JSON 写。
 */

import type {
  ApiMode,
  Envelope,
  LyaEvent,
  MessageRecord,
  Mode,
  SessionMeta,
  SessionTree,
  Snapshot,
  ToolBatchCall,
  ProviderSearchState,
  TurnEndReason,
} from './wire'

/** 后端返回的错误。 */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

/**
 * 把一个抛出来的东西说成一句能给用户看的话。
 *
 * 全应用只有这一处这么干。原先有两套，各自坏一头：
 *
 * - `String(error)` 会把 `Error: ` 这个前缀带进用户读的那句话（「连接被拒绝」变成
 *   「Error: 连接被拒绝」）。
 * - `error.message` 会把 [`ApiError`] 的状态码丢掉，于是 404 和 500 读起来一模一样——
 *   而那是两件事：一个是这东西不在了，一个是服务端炸了。用户能不能自己解决，全看这个数。
 *
 * 放在这儿是因为它认识的正是这个 client 抛出来的东西，跟 [`ApiError`] 待在一起最顺;
 * 要弹提示而不只是取字的，用 `app/errors.ts` 的 `report`。
 */
export function errorText(error: unknown): string {
  if (error instanceof ApiError) {
    // 状态码留着，正文可能是空的（后端返回空 body 时）
    return error.message ? `${error.status} ${error.message}` : `HTTP ${error.status}`
  }
  if (error instanceof Error && error.message) return error.message
  // 抛出来的不是 Error（字符串、对象、undefined 都见过），只能有什么说什么
  return String(error)
}

/** 前端启动握手。 */
export interface Bootstrap {
  image_token: string
  home: string | null
  default_model_id: string | null
  default_model_name: string | null
}

/** 创建会话时可给的字段。 */
export interface CreateSession {
  title?: string
  work_mode?: Mode
  model_id?: string | null
  /** 未传则 completions。 */
  api_mode?: ApiMode
}

/** 会话可改字段；不给的字段保持不变。 */
export interface PatchSession {
  title?: string
  work_mode?: Mode
  /** 显式给 null 表示回退到默认模型。 */
  model_id?: string | null
  /** 显式给 null 表示启用全部工具。 */
  enabled_tools?: string[] | null
  /** 显式给 null 表示清空。 */
  identity?: string | null
  /** 显式给 null 表示清空。 */
  style?: string | null
  /** 空会话可改；有消息后锁定。 */
  api_mode?: ApiMode
  /** 归档或取回。归档后只能回看，后端会拒绝一切写入。 */
  status?: 'active' | 'archived'
}

/** 一道题的作答。 */
export interface FormAnswerItem {
  question_id: string
  /** 单选一个、多选多个，文本题放内容。 */
  values: string[]
  /** 题目开了 `allow_note` 时才有。 */
  note?: string
}

/** 一次表单作答。 */
export interface FormAnswer {
  form_id: string
  items: FormAnswerItem[]
  /** 表单级补充说明。 */
  freetext?: string
}

/**
 * HITL 答复。
 *
 * 三种打断合用一个端点，因为它们共享「结清当前挂起、让本轮接着跑」这个语义。
 */
export type HitlReply =
  | { kind: 'form'; answer: FormAnswer }
  | { kind: 'confirm'; approved: boolean; note?: string }
  | { kind: 'mode_change'; approved: boolean }

/** 某个 API 栈下的能力摘要。 */
export interface ModelModeInfo {
  capabilities: string[]
}

/** 一个模型（密钥已脱敏）。 */
export interface ModelInfo {
  id: string
  name: string
  base_url: string
  api_key_masked: string
  /** 还是模板里的占位符，说明这个模型不能用。 */
  api_key_placeholder: boolean
  /** 输入上下文上限（token）；lya 元数据，不透传 API。 */
  context_window?: number | null
  /** 按 API 栈划分；前端按会话 api_mode 过滤可选模型。 */
  modes: Partial<Record<ApiMode, ModelModeInfo>>
}

/** 一个工具。`enabled` 只在按会话查询时才有。 */
export interface ToolInfo {
  name: string
  raw_name: string
  description: string
  /** 形如 `-R-`、`-R-W-X-`。 */
  permission: string
  /** 至少要哪个模式才看得到。 */
  min_mode: Mode
  /** 详细用法（喂给模型的那段）。 */
  prompt_hint?: string
  /** JSON Schema。 */
  parameters?: Record<string, unknown>
  enabled?: boolean
}

/** 一个动作。用户不能开关——动作是模型操作自身状态的手段，不是可选能力。 */
export interface ActionInfo {
  name: string
  raw_name: string
  description: string
  /** `continue` 直接接着跑，`await_human` 会挂起等人。 */
  flow: string
  /** 哪些模式下可见。 */
  visible_in: Mode[]
  prompt_hint?: string
  parameters?: Record<string, unknown>
}

/** 提示词各段键。 */
export type PromptSectionKey = 'environment' | 'operations' | 'voice' | 'identity' | 'style'

/** 提示词各段正文（空串表示回退内置默认）。 */
export interface PromptView {
  environment: string
  operations: string
  voice: string
  identity: string
  style: string
}

/** 配置全貌。`core` 只读——改端口这类事需要重启才生效，界面上不给改。 */
export interface ConfigView {
  core: Record<string, unknown>
  runtime: Record<string, unknown>
  models: ModelInfo[]
  prompt: PromptView
  core_readonly: boolean
}

/**
 * 一批文件的占用情况。
 *
 * 三个字节数各有各的意思，别混用：`logical_bytes` 是逐文件相加，`physical_bytes`
 * 是按 inode 去重后真正压在盘上的量，`reclaimable_bytes` 是删掉能腾出来的量。
 * 本地媒体缓存优先硬链接到用户原来的文件，那部分 `reclaimable_bytes` 是 0。
 */
export interface DiskUsage {
  logical_bytes: number
  physical_bytes: number
  reclaimable_bytes: number
  shared_bytes: number
  file_count: number
  linked_file_count: number
}

/** 树形占用节点。 */
export interface UsageSection {
  id: string
  label: string
  usage: DiskUsage
  children?: UsageSection[]
}

/** 主题素材目录里的一个文件。 */
export interface ThemeAsset {
  /** 相对素材目录的路径，可能带一层子目录。 */
  name: string
  /** `image` 走 <img>，`video` 走 <video>——CG 是视频。 */
  media: 'image' | 'video'
  bytes: number
  /** 展示名；创意工坊条目来自 project.json。 */
  title?: string
  /** 预览图的相对路径，视频加载出来之前当封面。 */
  poster?: string
}

/** `GET /api/theme/{id}/assets` 响应。 */
export interface ThemeAssetList {
  /** 素材目录的绝对路径，界面据此告诉用户往哪儿丢文件。 */
  dir: string
  exists: boolean
  assets: ThemeAsset[]
}

/** `GET /api/storage/stats` 响应。 */
export interface UsageReport {
  root: string
  usage: DiskUsage
  sections: UsageSection[]
}

/** 上下文占用的一项分类。 */
export interface ContextUsageCategory {
  id: string
  label: string
  tokens: number
  /** 默认 true；false 表示落库但未进 wire，不计入 total/pct。 */
  in_context?: boolean
}

/** `GET /api/sessions/{id}/context-usage` 响应。 */
export interface ContextUsageReport {
  tokenizer_id: string
  total: number
  limit: number
  pct: number
  categories: ContextUsageCategory[]
}

/** 探测一个模型能不能连通。 */
export interface ProbeResult {
  ok: boolean
  /** 该供应商声明支持的模型 id。 */
  models: string[]
  error?: string
}

/** 一条长期记忆。 */
export interface Memory {
  id: number
  title: string
  /** 一句话概括，进常驻索引。 */
  summary: string
  body: string
  tags: string[]
  /** 写下它的会话，仅溯源用。 */
  source_session_id: string | null
  created_at: string
  updated_at: string
}

/** 搜索命中（与后端 `lya_memory::MemoryHit` 一致；不含正文）。 */
export interface MemoryHit {
  id: number
  title: string
  summary: string
  tags: string[]
  /** 命中字段：title / summary / tag / body */
  matched_in: string
  snippet: string
}

/** 新建记忆。 */
export interface NewMemory {
  title: string
  summary: string
  body: string
  tags: string[]
}

/** 改记忆；不给的字段保持不变，`tags` 是整体替换。 */
export interface MemoryPatch {
  title?: string
  summary?: string
  body?: string
  tags?: string[]
}

/** 全局事件类型。 */
const GLOBAL_EVENTS = ['config_changed', 'sessions_changed'] as const

export class LyaClient {
  constructor(private readonly base = '') {}

  // ── 启动 ──────────────────────────────────────────────────────

  /**
   * 拿图片令牌与家目录。
   *
   * 令牌只能从这类 JSON 端点拿：跨域 `fetch` 一定带 `Origin`，会被后端的跨站
   * 守卫挡掉，所以恶意页面偷不走。
   */
  bootstrap(): Promise<Bootstrap> {
    return this.request('GET', '/api/bootstrap')
  }

  // ── 会话 ──────────────────────────────────────────────────────

  listSessions(): Promise<SessionMeta[]> {
    return this.request('GET', '/api/sessions')
  }

  createSession(body: CreateSession = {}): Promise<SessionMeta> {
    return this.request('POST', '/api/sessions', body)
  }

  /** 当前分支 + 正在跑的那一轮。 */
  snapshot(id: string): Promise<Snapshot> {
    return this.request('GET', `/api/sessions/${id}`)
  }

  patchSession(id: string, body: PatchSession): Promise<SessionMeta> {
    return this.request('PATCH', `/api/sessions/${id}`, body)
  }

  /** 已归档的会话。 */
  listArchived(): Promise<SessionMeta[]> {
    return this.request('GET', '/api/sessions/archived')
  }

  /**
   * 真删一个会话，连同它的全部消息，不可恢复。
   *
   * 和归档是两回事：归档只是收起来、仍能回看。调用前必须问一句。
   */
  deleteSession(id: string): Promise<void> {
    return this.request('DELETE', `/api/sessions/${id}`)
  }

  /**
   * 整棵树。
   *
   * 只在要画分叉图或算分支切换器时拉——分支只在重新生成、编辑重发之后才出现，
   * 那都是明确的用户操作，不需要跟着事件流实时更新。
   */
  tree(id: string): Promise<SessionTree> {
    return this.request('GET', `/api/sessions/${id}/tree`)
  }

  /** 估算当前活跃分支的上下文占用（只读）。 */
  contextUsage(id: string): Promise<ContextUsageReport> {
    return this.request('GET', `/api/sessions/${id}/context-usage`)
  }

  /** 手动压缩：裁掉较旧约一半工具结果（界面仍可见原文）。 */
  compactSession(id: string): Promise<{ pruned: number; saved_tokens: number }> {
    return this.request('POST', `/api/sessions/${id}/compact`)
  }

  /**
   * 发一条用户消息。
   *
   * **返回 202 就走了，正文从订阅流出来**——这样同一个会话在网页和手机上看到的
   * 是同一条流，而不是「谁发的谁才看得到响应」。
   */
  sendMessage(id: string, text: string): Promise<void> {
    return this.request('POST', `/api/sessions/${id}/messages`, { text })
  }

  /** 停掉正在跑的这一轮。 */
  stop(id: string): Promise<void> {
    return this.request('POST', `/api/sessions/${id}/stop`)
  }

  // ── 分支 ──────────────────────────────────────────────────────

  regenerate(id: string): Promise<void> {
    return this.request('POST', `/api/sessions/${id}/regenerate`)
  }

  /** 改掉某条消息并从那里重新开跑，旧分支原样保留。 */
  editAndResend(id: string, messageId: number, text: string): Promise<void> {
    return this.request('POST', `/api/sessions/${id}/messages/${messageId}`, { text })
  }

  deleteMessage(id: string, messageId: number): Promise<void> {
    return this.request('DELETE', `/api/sessions/${id}/messages/${messageId}`)
  }

  /** 切到另一个分支，返回切换后的快照。 */
  switchBranch(id: string, leafId: number): Promise<Snapshot> {
    return this.request('POST', `/api/sessions/${id}/branches`, { leaf_id: leafId })
  }

  // ── 白盒 ──────────────────────────────────────────────────────

  /** 模型清单。密钥已脱敏，不会把真钥匙发到界面上。 */
  models(): Promise<ModelInfo[]> {
    return this.request('GET', '/api/models')
  }

  /** 工具清单；给了会话 id 就顺带标出它当前生效哪些。 */
  tools(sessionId?: string): Promise<ToolInfo[]> {
    const query = sessionId ? `?session=${encodeURIComponent(sessionId)}` : ''
    return this.request('GET', `/api/tools${query}`)
  }

  /** 动作清单。只读展示——动作由模型自己调，用户关不掉。 */
  actions(): Promise<ActionInfo[]> {
    return this.request('GET', '/api/actions')
  }

  // ── 配置 ──────────────────────────────────────────────────────

  config(): Promise<ConfigView> {
    return this.request('GET', '/api/config')
  }

  /**
   * 写 runtime 配置。
   *
   * 后端用 `toml_edit` 落盘，注释和排版都保得住；写完会回读一次验证，
   * 所以返回的是**生效后**的值，不是你发过去那份。
   */
  writeRuntime(tables: Record<string, unknown>): Promise<unknown> {
    // 后端用 #[serde(flatten)] 收顶层表名，不能再包一层 tables——
    // 否则会写出 [tables.agent] 整段 runtime.toml 解析失败。
    return this.request('PUT', '/api/config/runtime', tables)
  }

  writePromptSection(section: PromptSectionKey, text: string): Promise<void> {
    return this.request('PUT', `/api/config/prompt/${section}`, { text })
  }

  /** 某个配置文件的原文，供高级编辑直接看 TOML。 */
  rawConfig(file: 'core' | 'runtime' | 'models' | 'prompt'): Promise<string> {
    return this.requestText('GET', `/api/config/raw/${file}`)
  }

  /** 数据目录占用（只读）。 */
  storageStats(): Promise<UsageReport> {
    return this.request('GET', '/api/storage/stats')
  }

  /**
   * 列一套主题的本地素材。
   *
   * 目录不存在也会正常返回（`exists: false`）——主题在没有素材时该照常能用，
   * 界面拿 `dir` 提示用户往哪儿放就行。
   */
  themeAssets(theme: string, kind: 'home' | 'cg'): Promise<ThemeAssetList> {
    return this.request('GET', `/api/theme/${theme}/assets?kind=${kind}`)
  }

  /**
   * 测一个**已配置**的模型通不通。
   *
   * 只传 id：真密钥留在服务器上取，界面手里只有脱敏的那串，也不该为了测一下
   * 就把钥匙在浏览器里转一圈。
   */
  probeModel(modelId: string): Promise<ProbeResult> {
    return this.request('POST', '/api/models/probe', { model_id: modelId })
  }

  // ── 记忆 ──────────────────────────────────────────────────────

  memories(): Promise<Memory[]> {
    return this.request('GET', '/api/memories')
  }

  memory(id: number): Promise<Memory> {
    return this.request('GET', `/api/memories/${id}`)
  }

  searchMemories(q: string, limit = 20): Promise<MemoryHit[]> {
    const query = new URLSearchParams({ q, limit: String(limit) })
    return this.request('GET', `/api/memories/search?${query}`)
  }

  createMemory(body: NewMemory): Promise<Memory> {
    return this.request('POST', '/api/memories', body)
  }

  updateMemory(id: number, patch: MemoryPatch): Promise<Memory> {
    return this.request('PATCH', `/api/memories/${id}`, patch)
  }

  /** 删除。模型只能读写记忆，删只走界面。 */
  deleteMemory(id: number): Promise<void> {
    return this.request('DELETE', `/api/memories/${id}`)
  }

  // ── 全局事件 ──────────────────────────────────────────────────

  /**
   * 订阅全局事件：配置变更、会话列表变化。
   *
   * 和会话流分开，因为它们与「当前打开哪个会话」无关——换会话不该断掉它。
   */
  subscribeGlobal(onEvent: (kind: string, payload: Record<string, unknown>) => void): () => void {
    const source = new EventSource(`${this.base}/api/events`)
    for (const kind of GLOBAL_EVENTS) {
      source.addEventListener(kind, (message) => {
        const envelope = JSON.parse((message as MessageEvent<string>).data) as Envelope
        onEvent(kind, envelope.payload)
      })
    }
    return () => source.close()
  }

  // ── HITL ─────────────────────────────────────────────────────

  /** 答复当前挂起，后端会自动接着跑下一轮。 */
  replyHitl(id: string, reply: HitlReply): Promise<void> {
    return this.request('POST', `/api/sessions/${id}/hitl`, reply)
  }

  // ── 工具开关 ──────────────────────────────────────────────────

  toggleTool(id: string, tool: string, enabled: boolean): Promise<void> {
    return this.request('PUT', `/api/sessions/${id}/tools/${tool}`, { enabled })
  }

  // ── 订阅 ──────────────────────────────────────────────────────

  /**
   * 订阅一个会话。
   *
   * 连上先收一份 `snapshot`，之后是增量。**订阅者跟不上时后端会再补一份快照**，
   * 所以 `onSnapshot` 可能被调用多次，每次都应当整体替换而不是合并。
   *
   * 返回一个断开函数。
   */
  subscribe(
    id: string,
    handlers: {
      onSnapshot: (snapshot: Snapshot) => void
      onEvent: (event: LyaEvent) => void
      onError?: (error: Event) => void
    },
  ): () => void {
    const source = new EventSource(`${this.base}/api/sessions/${id}/subscribe`)

    source.addEventListener('snapshot', (message) => {
      const envelope = JSON.parse((message as MessageEvent<string>).data) as Envelope
      handlers.onSnapshot(envelope.payload as unknown as Snapshot)
    })

    // 每种事件都是独立的 SSE event 类型，逐个挂上
    for (const type of EVENT_TYPES) {
      source.addEventListener(type, (message) => {
        const envelope = JSON.parse((message as MessageEvent<string>).data) as Envelope
        const event = toEvent(envelope)
        if (event) handlers.onEvent(event)
      })
    }

    if (handlers.onError) source.onerror = handlers.onError
    return () => source.close()
  }

  // ── 底层 ──────────────────────────────────────────────────────

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const text = await this.requestText(method, path, body)
    // 202 / 204 没有正文
    return (text ? JSON.parse(text) : undefined) as T
  }

  /** 拿原始正文，配置文件的 TOML 原文要用。 */
  private async requestText(method: string, path: string, body?: unknown): Promise<string> {
    const response = await fetch(`${this.base}${path}`, {
      method,
      headers: body === undefined ? {} : { 'content-type': 'application/json' },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    })
    if (!response.ok) {
      throw new ApiError(response.status, await response.text())
    }
    return response.text()
  }
}

/** 所有会话级事件类型，用来逐个挂监听。 */
const EVENT_TYPES = [
  'round_started',
  'reasoning_delta',
  'message_delta',
  'message_committed',
  'message_updated',
  'message_deleted',
  'call_started',
  'call_finished',
  'tool_batch_started',
  'await_human',
  'provider_search',
  'turn_end',
] as const satisfies readonly LyaEvent['type'][]

/**
 * 把信封摊平成带类型的事件。
 *
 * 线上格式是「`type` + 松散的 `payload`」，好让后端加字段不破坏老客户端；界面
 * 要的是可辨识联合，这里做这一次转换。认不出的类型返回 `null` 而不是抛错——
 * 后端加了新事件，老页面应该忽略它继续跑，而不是整条流断掉。
 */
export function toEvent(envelope: Envelope): LyaEvent | null {
  const p = envelope.payload
  switch (envelope.type) {
    case 'round_started':
      return { type: 'round_started', round: p['round'] as number }
    case 'message_delta':
      return { type: 'message_delta', text: p['text'] as string }
    case 'reasoning_delta':
      return { type: 'reasoning_delta', text: p['text'] as string }
    case 'message_committed':
      return { type: 'message_committed', record: p['record'] as MessageRecord }
    case 'message_updated':
      return { type: 'message_updated', record: p['record'] as MessageRecord }
    case 'message_deleted':
      return { type: 'message_deleted', id: p['id'] as number }
    case 'call_started':
      return {
        type: 'call_started',
        call_id: p['call_id'] as string,
        name: p['name'] as string,
        kind: p['kind'] as 'tool' | 'action',
      }
    case 'call_finished':
      return {
        type: 'call_finished',
        call_id: p['call_id'] as string,
        name: p['name'] as string,
        success: p['success'] as boolean,
      }
    case 'tool_batch_started':
      return {
        type: 'tool_batch_started',
        batch_id: p['batch_id'] as string,
        message_id: p['message_id'] as number,
        calls: p['calls'] as ToolBatchCall[],
      }
    case 'await_human': {
      const event: Extract<LyaEvent, { type: 'await_human' }> = {
        type: 'await_human',
        message_id: p['message_id'] as number,
      }
      if (p['batch_id'] != null) event.batch_id = p['batch_id'] as string
      if (p['review_index'] != null) event.review_index = p['review_index'] as number
      if (p['review_total'] != null) event.review_total = p['review_total'] as number
      return event
    }
    case 'provider_search': {
      const event: Extract<LyaEvent, { type: 'provider_search' }> = {
        type: 'provider_search',
        call_id: p['call_id'] as string,
        phase: p['phase'] as ProviderSearchState['phase'],
      }
      if (p['query'] != null) event.query = p['query'] as string
      return event
    }
    case 'turn_end':
      return { type: 'turn_end', reason: p['reason'] as TurnEndReason }
    default:
      return null
  }
}
