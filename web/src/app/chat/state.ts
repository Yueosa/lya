/** 聊天层共享响应式状态（单会话单例）。 */
import { ref, shallowRef } from 'vue'

import type { ModelInfo, ToolInfo } from '../../api/client'
import type { ApiMode, Mode, SessionMeta } from '../../api/wire'
import type { MessageRecord } from '../../api/wire'
import { emptyState, type SessionState } from '../../store/session'

/** bootstrap 下发的图片令牌与家目录。 */
export const imageBootstrap = ref<{ token: string; home: string } | null>(null)

/** 首屏快照到达前为 true。 */
export const hydrating = ref(false)

/**
 * 会话列表在拉取中。
 *
 * 和 `hydrating` 分开：列表刷新是全局的（配置一变就会刷），会话首屏加载是单个会话的。
 * 混成一个 ref 的话，后台刷一下列表就会让聊天页以为自己在加载。
 */
export const sessionsLoading = ref(false)

/** 配置里的默认模型。 */
export const defaultModel = ref<{ id: string; name: string } | null>(null)

/** 活跃 / 已归档会话列表。 */
export const sessions = ref<SessionMeta[]>([])
export const archivedSessions = ref<SessionMeta[]>([])

/** 当前打开的会话。 */
export const currentId = ref<string | null>(null)
export const state = ref<SessionState>(emptyState())

/** SSE 取消句柄。 */
export const unsubscribe = shallowRef<(() => void) | null>(null)

/** 分支树缓存。 */
export const tree = ref<MessageRecord[] | null>(null)

/** 运行时默认工作模式。 */
export const defaultWorkMode = ref<Mode>('agent')

/** 运行时默认 API 栈（新会话继承；空会话可在设置里改）。 */
export const defaultApiMode = ref<ApiMode>('completions')

/** 模型与工具清单。 */
export const models = ref<ModelInfo[]>([])
export const tools = ref<ToolInfo[]>([])

/** 本轮计时。 */
export const turnStartedAt = ref<number | null>(null)
export const elapsed = ref(0)
export let ticker: ReturnType<typeof setInterval> | null = null

/** 工具批 HITL 托盘当前查看的节点（可前后浏览，提交仍针对 pendingHitlId）。 */
export const focusedHitlId = ref<number | null>(null)

export function setTicker(next: ReturnType<typeof setInterval> | null): void {
  ticker = next
}
