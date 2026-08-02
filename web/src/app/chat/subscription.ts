import { computed } from 'vue'

import type { HitlReply } from '../../api/client'
import type { HitlBlock } from '../../api/wire'
import { buildTimeline } from '../../model/timeline'
import { applyEvent, applySnapshot, canSend as canSendTo, emptyState, isRunning } from '../../store/session'
import { toast } from '../../ui/useToast'
import { report } from './errors'
import { loadTools } from './settings'
import { refreshTree } from './snapshot'
import { round, startClock, stopClock } from './turn'
import { client } from './client'
import {
  currentId,
  hydrating,
  loading,
  state,
  tree,
  unsubscribe,
} from './state'

/** 渲染用的时间线。 */
export const timeline = computed(() =>
  buildTimeline({
    messages: state.value.messages,
    ...(tree.value ? { tree: tree.value } : {}),
    running: state.value.running,
    endReason: state.value.endReason,
  }),
)

export const meta = computed(() => state.value.meta)
export const running = computed(() => isRunning(state.value))
export const canSend = computed(() => currentId.value !== null && canSendTo(state.value))
export const pendingHitlId = computed(() => state.value.pendingHitlId)

export const pendingHitl = computed<HitlBlock | null>(() => {
  const id = state.value.pendingHitlId
  if (id === null) return null
  const record = state.value.messages.find((message) => message.id === id)
  return record?.payload.lya.hitl ?? null
})

/** 当前待审工具在调用组里的序号（仅 tool_confirm 批内有效）。 */
export const pendingHitlBatch = computed<{ index: number; total: number } | null>(() => {
  const id = state.value.pendingHitlId
  if (id === null) return null
  const record = state.value.messages.find((message) => message.id === id)
  const meta = record?.payload.lya.meta
  if (!meta) return null
  const index = meta['batch_index']
  const total = meta['batch_total']
  if (typeof index !== 'number' || typeof total !== 'number' || total <= 1) return null
  return { index, total }
})

/** 打开一个会话并订阅 SSE。 */
export async function openSession(id: string): Promise<void> {
  closeSession()
  currentId.value = id
  loading.value = true
  hydrating.value = true

  unsubscribe.value = client.subscribe(id, {
    onSnapshot: (snapshot) => {
      state.value = applySnapshot(state.value, snapshot)
      loading.value = false
      void loadTools()
      void refreshTree()
      queueMicrotask(() => {
        hydrating.value = false
      })
    },
    onEvent: (event) => {
      state.value = applyEvent(state.value, event)
      if (event.type === 'round_started' && round.value === 0) startClock()
      if (event.type === 'turn_end') {
        stopClock()
        void refreshTree()
      }
    },
    onError: () => {
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
  hydrating.value = false
  stopClock()
}

/** 答复当前挂起的 HITL。 */
export async function replyHitl(reply: HitlReply): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.replyHitl(id, reply)
  } catch (error) {
    report(error, '提交')
  }
}
