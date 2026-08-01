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

import { ApiError, LyaClient, type HitlReply } from '../api/client'
import type { HitlBlock, SessionMeta } from '../api/wire'
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

/** 渲染用的时间线。 */
export const timeline = computed<TimelineItem[]>(() =>
  buildTimeline({
    messages: state.value.messages,
    running: state.value.running,
    endReason: state.value.endReason,
  }),
)

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
    },
    onEvent: (event) => {
      state.value = applyEvent(state.value, event)
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
