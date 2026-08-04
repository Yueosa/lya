import { describe, expect, it } from 'vitest'

import { formatSearchQueries } from './chatBlockHelpers'

describe('formatSearchQueries', () => {
  it('joins queries array', () => {
    expect(formatSearchQueries({ queries: ['Rust', 'AI'] })).toBe('Rust · AI')
  })

  it('falls back to single query', () => {
    expect(formatSearchQueries({ query: '天气' })).toBe('天气')
  })

  it('returns null when empty', () => {
    expect(formatSearchQueries({ queries: [] })).toBeNull()
  })
})
