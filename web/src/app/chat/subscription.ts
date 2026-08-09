import { computed, watch } from 'vue'

import type { HitlReply } from '../../api/client'
import type { HitlBlock, LyaEvent } from '../../api/wire'
import { buildTimeline } from '../../model/timeline'
import { applySnapshot, canSend as canSendTo, emptyState, isRunning } from '../../store/session'
import { toast } from '../../ui/useToast'
import { bindSessionPrefs } from '../usePrefs'
import { report } from '../errors'
import { loadTools } from './settings'
import { refreshTree } from './snapshot'
import { round, startClock, stopClock } from './turn'
import { createEventBatcher } from './eventBatch'
import { client } from '../client'
import {
  currentId,
  focusedHitlId,
  hydrating,
  state,
  tree,
  unsubscribe,
} from './state'

function hitlIdsInBatch(anchorId: number): number[] {
  const current = state.value.messages.find((message) => message.id === anchorId)
  const batchId = current?.payload.lya.meta?.['batch_id']
  const pending = state.value.messages.filter(
    (message) => message.payload.role === 'hitl' && message.payload.status === 'pending',
  )
  if (typeof batchId !== 'string') {
    return pending.some((message) => message.id === anchorId) ? [anchorId] : []
  }
  return pending
    .filter((message) => message.payload.lya.meta?.['batch_id'] === batchId)
    .sort(
      (a, b) =>
        Number(a.payload.lya.meta?.['batch_index'] ?? 0) -
        Number(b.payload.lya.meta?.['batch_index'] ?? 0),
    )
    .map((message) => message.id)
}

watch(
  () => state.value.pendingHitlId,
  (id) => {
    focusedHitlId.value = id
  },
  { immediate: true },
)

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

export const batchPendingHitlIds = computed(() => {
  const id = state.value.pendingHitlId
  if (id === null) return []
  return hitlIdsInBatch(id)
})

export const canSubmitFocusedHitl = computed(
  () =>
    focusedHitlId.value !== null &&
    focusedHitlId.value === state.value.pendingHitlId,
)

export const canNavHitlPrev = computed(() => {
  const ids = batchPendingHitlIds.value
  const focused = focusedHitlId.value ?? state.value.pendingHitlId
  if (focused === null) return false
  return ids.indexOf(focused) > 0
})

export const canNavHitlNext = computed(() => {
  const ids = batchPendingHitlIds.value
  const focused = focusedHitlId.value ?? state.value.pendingHitlId
  const pending = state.value.pendingHitlId
  if (focused === null || pending === null) return false
  const at = ids.indexOf(focused)
  const pendingAt = ids.indexOf(pending)
  if (at < 0 || pendingAt < 0) return false
  // 只能预览当前待确认项及之前的，不能跳过还没轮到的
  return at < pendingAt
})

/** 正在预览批内其他项，还不能提交当前 pending。 */
export const hitlFocusBlocksSubmit = computed(
  () =>
    state.value.pendingHitlId !== null &&
    focusedHitlId.value !== null &&
    focusedHitlId.value !== state.value.pendingHitlId,
)

export function navigateHitlBatch(delta: -1 | 1): void {
  const ids = batchPendingHitlIds.value
  const focused = focusedHitlId.value ?? state.value.pendingHitlId
  if (focused === null || ids.length <= 1) return
  const at = ids.indexOf(focused)
  if (at < 0) return
  const next = ids[at + delta]
  if (next !== undefined) focusedHitlId.value = next
}

export const pendingHitl = computed<HitlBlock | null>(() => {
  const id = focusedHitlId.value ?? state.value.pendingHitlId
  if (id === null) return null
  const record = state.value.messages.find((message) => message.id === id)
  return record?.payload.lya.hitl ?? null
})

/** 当前待审工具在调用组里的序号（仅 tool_confirm 批内有效）。 */
export const pendingHitlBatch = computed<{ index: number; total: number } | null>(() => {
  const id = focusedHitlId.value ?? state.value.pendingHitlId
  if (id === null) return null
  const record = state.value.messages.find((message) => message.id === id)
  const meta = record?.payload.lya.meta
  if (!meta) return null
  const index = meta['batch_index']
  const total = meta['batch_total']
  if (typeof index !== 'number' || typeof total !== 'number' || total <= 1) return null
  return { index, total }
})

/** 关掉当前会话的订阅。 */
export function closeSession(): void {
  unsubscribe.value?.()
  unsubscribe.value = null
  currentId.value = null
  bindSessionPrefs(null)
  state.value = emptyState()
  tree.value = null
  focusedHitlId.value = null
  hydrating.value = false
  stopClock()
}

function handleEventSideEffects(event: LyaEvent): void {
  if (event.type === 'round_started' && round.value === 0) startClock()
  if (event.type === 'turn_end') {
    stopClock()
    void refreshTree()
  }
}

/** 打开一个会话并订阅 SSE。 */
export async function openSession(id: string): Promise<void> {
  closeSession()
  currentId.value = id
  bindSessionPrefs(id)
  hydrating.value = true

  let snapshotReady = false
  let preSnapshotEvents: LyaEvent[] = []

  const batcher = createEventBatcher({
    getState: () => state.value,
    setState: (next) => {
      state.value = next
    },
    onApplied: handleEventSideEffects,
  })

  unsubscribe.value = client.subscribe(id, {
    onSnapshot: (snapshot) => {
      batcher.flush()
      state.value = applySnapshot(state.value, snapshot)
      snapshotReady = true
      for (const event of preSnapshotEvents) batcher.push(event)
      preSnapshotEvents = []
      void loadTools()
      void refreshTree()
      queueMicrotask(() => {
        hydrating.value = false
      })
    },
    onEvent: (event) => {
      if (!snapshotReady) {
        preSnapshotEvents.push(event)
        return
      }
      batcher.push(event)
    },
    onError: () => {
      toast('与后端的连接断了，正在重试', 'error')
    },
  })

  const priorClose = unsubscribe.value
  unsubscribe.value = () => {
    batcher.dispose()
    priorClose()
  }
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
