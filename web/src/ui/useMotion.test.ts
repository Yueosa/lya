import { describe, expect, it } from 'vitest'

import { messageStaggerDelay } from './useMotion'

describe('useMotion', () => {
  it('messageStaggerDelay 按序递增并封顶', () => {
    expect(messageStaggerDelay(0)).toBe('0ms')
    expect(messageStaggerDelay(1)).toBe('42ms')
    expect(messageStaggerDelay(28)).toBe('1176ms')
    expect(messageStaggerDelay(99)).toBe('1176ms')
  })
})
