import { describe, expect, it } from 'vitest'

import type { ModelInfo } from '../api/client'
import {
  formatContext,
  gatewayLabel,
  groupModelsByGateway,
  modeStackHint,
} from './gateways'

function model(partial: Partial<ModelInfo> & Pick<ModelInfo, 'id' | 'base_url'>): ModelInfo {
  return {
    name: partial.id,
    api_key_masked: 'sk-…',
    api_key_placeholder: false,
    modes: {},
    ...partial,
  }
}

describe('groupModelsByGateway', () => {
  it('按 base_url 归组且保住首次出现顺序', () => {
    const groups = groupModelsByGateway([
      model({ id: 'a', base_url: 'https://one.example' }),
      model({ id: 'b', base_url: 'https://two.example' }),
      model({ id: 'c', base_url: 'https://one.example' }),
    ])
    expect(groups.map((g) => g.baseUrl)).toEqual([
      'https://one.example',
      'https://two.example',
    ])
    expect(groups[0]!.models.map((m) => m.id)).toEqual(['a', 'c'])
  })
})

describe('gatewayLabel', () => {
  it('普通 host', () => {
    expect(gatewayLabel('https://api.example.com/v1')).toBe('api.example.com/v1')
  })

  it('根 path 只留 host', () => {
    expect(gatewayLabel('https://api.example.com/')).toBe('api.example.com')
  })
})

describe('formatContext', () => {
  it('整 K / 整 M', () => {
    expect(formatContext(128 * 1024)).toBe('128K')
    expect(formatContext(2 * 1_048_576)).toBe('2M')
  })

  it('空值', () => {
    expect(formatContext(null)).toBe('—')
  })
})

describe('modeStackHint', () => {
  it('没配时说人话', () => {
    const m = model({ id: 'x', base_url: 'https://x', modes: {} })
    expect(modeStackHint(m, 'responses')).toContain('未配置')
  })
})
