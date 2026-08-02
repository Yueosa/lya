import { describe, expect, it } from 'vitest'

import { bytesToMegabytes, formatBytes, megabytesToBytes } from './formatBytes'

describe('formatBytes', () => {
  it('formats small and large values', () => {
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(2048)).toBe('2.0 KB')
    expect(formatBytes(32 * 1024 * 1024)).toBe('32.0 MB')
  })

  it('converts megabytes for the config form', () => {
    expect(megabytesToBytes(32)).toBe(32 * 1024 * 1024)
    expect(bytesToMegabytes(32 * 1024 * 1024)).toBe(32)
  })
})
