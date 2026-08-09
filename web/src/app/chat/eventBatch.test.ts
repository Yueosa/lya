import { describe, expect, it } from 'vitest'

import { applyEvent, emptyState } from '../../store/session'
import type { LyaEvent } from '../../api/wire'
import { createEventBatcher } from './eventBatch'

describe('createEventBatcher', () => {
  it('merges delta events before applying state', () => {
    let state = emptyState()
    state = applyEvent(state, {
      type: 'round_started',
      round: 1,
    })
    const applied: LyaEvent[] = []

    const batcher = createEventBatcher({
      getState: () => state,
      setState: (next) => {
        state = next
      },
      onApplied: (event) => applied.push(event),
    })

    batcher.push({ type: 'message_delta', text: 'a' })
    batcher.push({ type: 'message_delta', text: 'b' })
    batcher.flush()

    expect(state.running?.content).toBe('ab')
    expect(applied).toHaveLength(2)
  })

  it('flushes pending deltas before structural events', () => {
    let state = emptyState()
    state = applyEvent(state, { type: 'round_started', round: 1 })
    const applied: LyaEvent[] = []

    const batcher = createEventBatcher({
      getState: () => state,
      setState: (next) => {
        state = next
      },
      onApplied: (event) => applied.push(event),
    })

    batcher.push({ type: 'message_delta', text: 'x' })
    batcher.push({ type: 'turn_end', reason: { kind: 'completed' } })

    expect(state.running).toBeNull()
    expect(applied.map((event) => event.type)).toEqual(['message_delta', 'turn_end'])
  })
})
