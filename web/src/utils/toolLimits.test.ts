import { describe, expect, it } from 'vitest'

import { buildToolsEnabledPayload, readGlobalToolsMode } from './toolLimits'

describe('readGlobalToolsMode', () => {
  it('missing enabled means all', () => {
    expect(readGlobalToolsMode({}).mode).toBe('all')
  })

  it('empty array means none', () => {
    expect(readGlobalToolsMode({ tools: { enabled: [] } }).mode).toBe('none')
  })

  it('list means custom', () => {
    const { mode, enabled } = readGlobalToolsMode({
      tools: { enabled: ['bash', 'file_read'] },
    })
    expect(mode).toBe('custom')
    expect(enabled.has('bash')).toBe(true)
  })
})

describe('buildToolsEnabledPayload', () => {
  it('all mode removes key', () => {
    expect(buildToolsEnabledPayload('all', new Set(['a'])).enabled).toBeNull()
  })
})
