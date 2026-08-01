/**
 * 把传输层、store 与界面接起来。
 *
 * `store/session.ts` 是纯函数，不认识 Vue 也不认识网络；`api/client.ts` 只管
 * 收发。这一层负责三件事：持有响应式状态、管住 SSE 订阅的生命周期、把用户动作
 * 翻译成请求。
 *
 * 仍然是模块级单例——同一时刻只会看着一个会话，而侧边栏、聊天区、输入框都要
 * 读同一份状态。
 */

import { computed, ref, shallowRef } from 'vue'

import {
  ApiError,
  LyaClient,
  type HitlReply,
  type ModelInfo,
  type ToolInfo,
} from '../api/client'
import type { HitlBlock, MessageRecord, Mode, SessionMeta, SessionTree } from '../api/wire'
import { buildTimeline, type TimelineItem } from '../model/timeline'
import {
  applyEvent,
  applySnapshot,
  canSend as canSendTo,
  emptyState,
  isRunning,
  type SessionState,
} from '../store/session'
import { toast } from '../ui/useToast'

export const client = new LyaClient()

/**
 * 本地图片渲染要的令牌与家目录。
 *
 * 拿不到就为 `null`——那时本地图片会渲染成坏图，比拼一个不带令牌、必然 403 的
 * 地址要诚实。
 */
export const imageContext = ref<{ token: string; home: string } | null>(null)

/** 启动握手。失败不阻塞，只是本地图片显示不出来。 */
export async function bootstrap(): Promise<void> {
  try {
    const info = await client.bootstrap()
    if (info.home) imageContext.value = { token: info.image_token, home: info.home }
  } catch {
    toast('拿不到图片令牌，本地图片将无法显示', 'error')
  }
}

/** 会话列表。 */
export const sessions = ref<SessionMeta[]>([])
/** 当前打开的会话 id。 */
export const currentId = ref<string | null>(null)
/** 当前会话的全部状态。 */
const state = ref<SessionState>(emptyState())
/** 正在加载会话列表或快照。 */
export const loading = ref(false)

/** 断开当前订阅；切换会话或退出时调用。 */
const unsubscribe = shallowRef<(() => void) | null>(null)

/**
 * 整棵树，只用来算分支切换器。
 *
 * 不跟着事件流实时维护：分叉只在重新生成、编辑重发之后产生，那都是明确的用户
 * 操作，拉快照和收轮次结束时各刷一次就够了。
 */
const tree = ref<MessageRecord[] | null>(null)

/** 渲染用的时间线。 */
export const timeline = computed<TimelineItem[]>(() =>
  buildTimeline({
    messages: state.value.messages,
    ...(tree.value ? { tree: tree.value } : {}),
    running: state.value.running,
    endReason: state.value.endReason,
  }),
)

/** 刷新分支信息。 */
async function refreshTree(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    tree.value = (await client.tree(id)).nodes
  } catch {
    // 拿不到就只是不显示切换器，不影响读消息
  }
}

/** 当前会话元数据。 */
export const meta = computed(() => state.value.meta)
/** 本轮是否在跑。 */
export const running = computed(() => isRunning(state.value))
/** 现在能不能发消息。 */
export const canSend = computed(() => currentId.value !== null && canSendTo(state.value))
/** 正等用户答复的 HITL 节点 id。 */
export const pendingHitlId = computed(() => state.value.pendingHitlId)

/** 正等答复的那个 HITL 块；没有则为 `null`。 */
export const pendingHitl = computed<HitlBlock | null>(() => {
  const id = state.value.pendingHitlId
  if (id === null) return null
  const record = state.value.messages.find((message) => message.id === id)
  return record?.payload.lya.hitl ?? null
})

/**
 * 答复当前挂起。
 *
 * 后端结清之后会自己接着跑下一轮，所以这里不用再调一次发送——事件流会继续。
 */
export async function replyHitl(reply: HitlReply): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.replyHitl(id, reply)
  } catch (error) {
    report(error, '提交')
  }
}

/** 把后端报的错说给用户听。 */
function report(error: unknown, what: string): void {
  const detail = error instanceof ApiError ? `${error.status} ${error.message}` : String(error)
  toast(`${what}失败：${detail}`, 'error')
}

/** 拉会话列表。 */
export async function refreshSessions(): Promise<void> {
  loading.value = true
  try {
    sessions.value = await client.listSessions()
  } catch (error) {
    report(error, '读取会话列表')
  } finally {
    loading.value = false
  }
}

/** 建一个新会话并打开它。 */
export async function createSession(): Promise<void> {
  try {
    const created = await client.createSession({})
    sessions.value = [created, ...sessions.value]
    await openSession(created.id)
  } catch (error) {
    report(error, '新建会话')
  }
}

/**
 * 打开一个会话。
 *
 * 订阅时先收一份快照再收增量，所以不需要另外拉一次历史。断线重连、订阅者跟不上
 * 时后端会**再补一份快照**，那时整体替换即可——两条路走的是同一个入口。
 */
export async function openSession(id: string): Promise<void> {
  closeSession()
  currentId.value = id
  loading.value = true

  unsubscribe.value = client.subscribe(id, {
    onSnapshot: (snapshot) => {
      state.value = applySnapshot(state.value, snapshot)
      loading.value = false
      void loadTools()
      void refreshTree()
    },
    onEvent: (event) => {
      state.value = applyEvent(state.value, event)
      // 一轮结束才可能多出分叉，没必要每个增量都去拉整棵树
      if (event.type === 'turn_end') void refreshTree()
    },
    onError: () => {
      // EventSource 自己会重连，重连后收到的快照会把状态对齐，
      // 所以这里只提示一声，不做别的
      toast('与后端的连接断了，正在重试', 'error')
    },
  })
}

/** 关掉当前会话的订阅。 */
export function closeSession(): void {
  unsubscribe.value?.()
  unsubscribe.value = null
  currentId.value = null
  state.value = emptyState()
  tree.value = null
}

/**
 * 发一条消息。
 *
 * 后端返回 202 就结束，**正文从订阅流里出来**——所以这里不等响应体，
 * 界面靠事件更新。
 */
export async function send(text: string): Promise<void> {
  const id = currentId.value
  if (!id || !text.trim()) return
  try {
    await client.sendMessage(id, text)
  } catch (error) {
    report(error, '发送')
  }
}

/** 停掉正在跑的这一轮。 */
export async function stop(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.stop(id)
  } catch (error) {
    report(error, '停止')
  }
}

// ── 分支操作 ──────────────────────────────────────────────────
//
// 这几个都会改动消息树，后端会在改完之后推一份新快照——分叉换掉的是整条可见
// 路径，增量说不清。所以这里都不用手动刷新。

/** 换个答法重答上一轮。旧分支留着，随时能切回去。 */
export async function regenerate(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.regenerate(id)
  } catch (error) {
    report(error, '重新生成')
  }
}

/** 改掉某条自己发的消息并从那里重开。旧问法与旧回答成为并列分支。 */
export async function editAndResend(messageId: number, text: string): Promise<void> {
  const id = currentId.value
  if (!id || !text.trim()) return
  try {
    await client.editAndResend(id, messageId, text)
  } catch (error) {
    report(error, '编辑重发')
  }
}

/** 删掉一个叶节点。 */
export async function deleteMessage(messageId: number): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.deleteMessage(id, messageId)
    await refreshSnapshot()
    await refreshTree()
  } catch (error) {
    report(error, '删除消息')
  }
}

/** 切到另一条分支。 */
export async function switchBranch(leafId: number): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    state.value = applySnapshot(state.value, await client.switchBranch(id, leafId))
    await refreshTree()
  } catch (error) {
    report(error, '切换分支')
  }
}

/** 主动拉一次快照。删消息这类后端没广播的改动之后用。 */
async function refreshSnapshot(): Promise<void> {
  const id = currentId.value
  if (!id) return
  state.value = applySnapshot(state.value, await client.snapshot(id))
}

/** 整棵树，画分支图用。每次现拉，不跟着事件流维护。 */
export async function loadTree(): Promise<SessionTree | null> {
  const id = currentId.value
  if (!id) return null
  try {
    return await client.tree(id)
  } catch (error) {
    report(error, '读取分支树')
    return null
  }
}

// ── 会话设置 ──────────────────────────────────────────────────

/** 可选模型清单。 */
export const models = ref<ModelInfo[]>([])
/** 当前会话的工具清单，带生效状态。 */
export const tools = ref<ToolInfo[]>([])

/** 拉模型清单，启动时一次就够。 */
export async function loadModels(): Promise<void> {
  try {
    models.value = await client.models()
  } catch {
    // 拿不到就只是选不了模型，不影响聊天
  }
}

/** 拉当前会话的工具清单。 */
export async function loadTools(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    tools.value = await client.tools(id)
  } catch (error) {
    report(error, '读取工具清单')
  }
}

/** 开关某个工具。 */
export async function toggleTool(name: string, enabled: boolean): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.toggleTool(id, name, enabled)
    await loadTools()
  } catch (error) {
    report(error, '切换工具')
  }
}

/** 换工作模式。走 agent，会在树上留一条说明。 */
export async function setMode(mode: Mode): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    const updated = await client.patchSession(id, { work_mode: mode })
    state.value = { ...state.value, meta: updated }
    // 模式变了，能用的工具跟着变
    await Promise.all([loadTools(), refreshSnapshot()])
  } catch (error) {
    report(error, '切换模式')
  }
}

/** 换模型；`null` 表示回退到配置里的默认。 */
export async function setModel(modelId: string | null): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    state.value = {
      ...state.value,
      meta: await client.patchSession(id, { model_id: modelId }),
    }
  } catch (error) {
    report(error, '切换模型')
  }
}

/** 这个会话是不是只读的。 */
export const readOnly = computed(() => state.value.meta?.status === 'archived')

/** 归档或取回。归档后后端会拒绝一切写入，界面也收掉输入框。 */
export async function setArchived(id: string, archived: boolean): Promise<void> {
  const updated = await client.patchSession(id, { status: archived ? 'archived' : 'active' })
  sessions.value = archived
    ? sessions.value.filter((item) => item.id !== id)
    : [updated, ...sessions.value]
  if (state.value.meta?.id === id) {
    state.value = { ...state.value, meta: updated }
  }
}

/** 真删，不可恢复。调用方必须先问过用户。 */
export async function removeSession(id: string): Promise<void> {
  await client.deleteSession(id)
  sessions.value = sessions.value.filter((item) => item.id !== id)
  if (currentId.value === id) closeSession()
}

/** 改标题。 */
export async function rename(id: string, title: string): Promise<void> {
  const updated = await client.patchSession(id, { title })
  sessions.value = sessions.value.map((item) => (item.id === id ? updated : item))
  if (state.value.meta?.id === id) {
    state.value = { ...state.value, meta: updated }
  }
}
