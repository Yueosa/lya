import type { LyaEvent } from '../../api/wire'
import { applyEvent, type SessionState } from '../../store/session'

/** 可合并到同一帧的状态事件；结构性事件必须立即 flush。 */
const BATCHABLE = new Set<LyaEvent['type']>([
  'message_delta',
  'reasoning_delta',
  'call_started',
  'call_finished',
  'provider_search',
])

export type EventBatchSink = {
  getState: () => SessionState
  setState: (next: SessionState) => void
  onApplied: (event: LyaEvent) => void
}

/** 把高频 SSE 增量合并为每帧一次 state 更新。 */
export function createEventBatcher(sink: EventBatchSink) {
  let pending: LyaEvent[] = []
  let rafId: number | null = null

  function flush(): void {
    if (rafId !== null) {
      cancelAnimationFrame(rafId)
      rafId = null
    }
    if (pending.length === 0) return
    const batch = pending
    pending = []
    let next = sink.getState()
    for (const event of batch) {
      next = applyEvent(next, event)
    }
    sink.setState(next)
    for (const event of batch) {
      sink.onApplied(event)
    }
  }

  function push(event: LyaEvent): void {
    if (!BATCHABLE.has(event.type)) {
      flush()
      sink.setState(applyEvent(sink.getState(), event))
      sink.onApplied(event)
      return
    }
    pending.push(event)
    if (rafId === null) {
      rafId = requestAnimationFrame(() => {
        rafId = null
        flush()
      })
    }
  }

  function dispose(): void {
    flush()
  }

  return { push, flush, dispose }
}
