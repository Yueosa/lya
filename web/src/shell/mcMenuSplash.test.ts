import { describe, expect, it } from 'vitest'

import { buildSplashLines, menuFootLeft, menuFootRight, pickSplash } from './mcMenuSplash'

describe('mcMenuSplash', () => {
  it('有数据时生成 splash', () => {
    const lines = buildSplashLines(
      [{ id: 's1', title: 'Rust 闲聊', updated_at: '', status: 'active', work_mode: 'agent', model_id: null, api_mode: 'completions', persona: null, created_at: '', active_leaf_id: null, enabled_tools: null }],
      [],
      [{ id: 1, title: '偏好', summary: '', body: '', tags: [], source_session_id: null, created_at: '', updated_at: '' }],
      [{ id: 'md1', name: 'gpt-4', base_url: '', api_key_masked: '', api_key_placeholder: false, modes: { completions: { capabilities: ['text'] } } }],
    )
    expect(lines.some((line) => line.includes('Rust 闲聊'))).toBe(true)
    expect(lines.some((line) => line.includes('偏好'))).toBe(true)
    expect(lines.some((line) => line.includes('gpt-4'))).toBe(true)
  })

  it('无数据时用 fallback', () => {
    expect(buildSplashLines([], [], [], [])).toContain('Also try 新的对话！')
  })

  it('pickSplash 可复现', () => {
    const lines = ['a', 'b', 'c']
    expect(pickSplash(lines, 0.1)).toBe('a')
    expect(pickSplash(lines, 0.9)).toBe('c')
  })

  it('角落文案', () => {
    expect(menuFootLeft([], [], [])).toBe('lya · 0 活跃 · 0 归档 · 0 记忆')
    expect(menuFootRight([])).toBe('0 个模型 · Minecraft Edition')
  })
})
