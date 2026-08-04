import { computed } from 'vue'

import { elapsed, setTicker, state, ticker, turnStartedAt } from './state'

export function startClock(): void {
  turnStartedAt.value = Date.now()
  elapsed.value = 0
  if (ticker) return
  setTicker(
    setInterval(() => {
      if (turnStartedAt.value) elapsed.value = Date.now() - turnStartedAt.value
    }, 100),
  )
}

export function stopClock(): void {
  if (ticker) clearInterval(ticker)
  setTicker(null)
  turnStartedAt.value = null
}

export { elapsed }

export const phase = computed<{ text: string } | null>(() => {
  const buffer = state.value.running
  if (!buffer) return null

  if (state.value.pendingHitlId) return { text: '等待确认' }

  const active = buffer.calls.find((call) => call.ok === null)
  if (active) return { text: `${active.name} 执行中` }

  const searching = buffer.provider_searches?.find((s) => s.phase === 'searching')
  if (searching) {
    return {
      text: searching.query ? `正在搜索：${searching.query}` : '正在搜索…',
    }
  }
  const preparing = buffer.provider_searches?.find((s) => s.phase === 'in_progress')
  if (preparing) return { text: '正在准备搜索…' }

  if (buffer.content) return { text: '正在回复' }
  if (buffer.reasoning) return { text: '正在思考' }
  return { text: '等待模型' }
})

export const round = computed(() => state.value.running?.round ?? 0)
