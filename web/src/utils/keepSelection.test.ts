import { describe, expect, it } from 'vitest'

import { keepSelectedItem, keepSelectedKey } from './keepSelection'

describe('keepSelectedKey', () => {
  it('还在列表里就不动', () => {
    expect(keepSelectedKey(['a', 'b'], 'b')).toBe('b')
  })

  it('不在了就落到第一项', () => {
    expect(keepSelectedKey(['a', 'b'], 'gone')).toBe('a')
  })

  it('不在了但偏好项在，就落到偏好', () => {
    expect(keepSelectedKey(['a', 'b'], 'gone', 'b')).toBe('b')
  })

  it('列表空了就清空', () => {
    expect(keepSelectedKey([], 'a')).toBeNull()
  })
})

describe('keepSelectedItem', () => {
  const items = [
    { name: 'read' },
    { name: 'write' },
  ]

  it('还在就保住整项', () => {
    expect(keepSelectedItem(items, items[1]!, (i) => i.name)).toBe(items[1])
  })

  it('不在了落到第一项', () => {
    expect(keepSelectedItem(items, { name: 'gone' }, (i) => i.name)).toBe(items[0])
  })
})
