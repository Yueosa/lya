import { describe, expect, it } from 'vitest'

import { bubbleSeparator, messageTimeSeparator } from './dateFormat'

describe('messageTimeSeparator', () => {
  const now = new Date(2026, 7, 2, 12, 0, 0)

  it('会话第一条精确到分钟', () => {
    const ts = new Date(2026, 6, 12, 14, 23, 0).toISOString()
    expect(messageTimeSeparator(null, ts, now)).toBe('7月12日 周日 14:23')
  })

  it('昨天的消息显示昨天而非今天', () => {
    const ts = new Date(2026, 7, 1, 14, 23, 0).toISOString()
    expect(messageTimeSeparator(null, ts, now)).toBe('昨天 14:23')
  })

  it('同日间隔超过 10 分钟带分钟', () => {
    const prev = new Date(2026, 7, 2, 10, 0, 0).toISOString()
    const curr = new Date(2026, 7, 2, 10, 15, 0).toISOString()
    expect(bubbleSeparator(prev, curr, now)).toBe('今天 10:15')
  })

  it('同日间隔不超过 10 分钟不显示', () => {
    const prev = new Date(2026, 7, 2, 10, 0, 0).toISOString()
    const curr = new Date(2026, 7, 2, 10, 9, 0).toISOString()
    expect(bubbleSeparator(prev, curr, now)).toBe('')
  })

  it('跨日分隔带分钟', () => {
    const prev = new Date(2026, 6, 11, 22, 0, 0).toISOString()
    const curr = new Date(2026, 6, 12, 9, 5, 0).toISOString()
    expect(bubbleSeparator(prev, curr, now)).toBe('7月12日 周日 09:05')
  })
})
