import type { LyaEvent } from '../../api/wire'
import { applyEvent, type SessionState } from '../../store/session'

/** 可合并到同一帧的状态事件；结构性事件必须立即 flush。 */
const BATCHABLE = new Set<LyaEvent['type']>([
  'message_delta',
  'reasoning_delta',
  'provider_search',
])

export type EventBatchSink = {
  getState: () => SessionState
  setState: (next: SessionState) => void
  onApplied: (event: LyaEvent) => void
}

function scheduleFlush(run: () => void): () => void {
  if (typeof document !== 'undefined' && document.hidden) {
    const id = window.setTimeout(run, 0)
    return () => window.clearTimeout(id)
  }
  const id = requestAnimationFrame(run)
  return () => cancelAnimationFrame(id)
}

/** 把高频 SSE 增量合并为每帧一次 state 更新。 */
export function createEventBatcher(sink: EventBatchSink) {
  let pending: LyaEvent[] = []
  let cancelScheduled: (() => void) | null = null

  function flush(): void {
    if (cancelScheduled) {
      cancelScheduled()
      cancelScheduled = null
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
    if (cancelScheduled === null) {
      cancelScheduled = scheduleFlush(() => {
        cancelScheduled = null
        flush()
      })
    }
  }

  function dispose(): void {
    flush()
  }

  return { push, flush, dispose }
}
