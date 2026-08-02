import { describe, expect, it } from 'vitest'

import type { MessageRecord } from '../api/wire'
import {
  defaultTreeFilters,
  isModeChangeNode,
  nodePreview,
  projectVisibleTree,
  treeNode,
} from './branchTree'

function chat(
  id: number,
  role: 'user' | 'assistant',
  content: string,
  parent: number | null,
  extra?: Partial<MessageRecord['payload']['openai']>,
): MessageRecord {
  return {
    id,
    session_id: 's',
    parent_id: parent,
    sort_key: id,
    created_at: '2026-01-01T00:00:00.000Z',
    payload: {
      v: 1,
      role,
      kind: 'chat',
      status: 'complete',
      openai: { role, content, ...extra },
      lya: {},
    },
  }
}

function tool(parent: number, content = 'ok'): MessageRecord {
  return {
    id: 99,
    session_id: 's',
    parent_id: parent,
    sort_key: 99,
    created_at: '2026-01-01T00:00:00.000Z',
    payload: {
      v: 1,
      role: 'tool',
      kind: 'tool_result',
      status: 'complete',
      openai: { role: 'tool', content, tool_call_id: 'c1' },
      lya: {},
    },
  }
}

describe('treeNode', () => {
  it('assistant 有正文时保留，即使带了 tool_calls', () => {
    const node = chat(2, 'assistant', '喵~', 1, {
      tool_calls: [{ id: 'c1', type: 'function', function: { name: 'x', arguments: '{}' } }],
    })
    expect(treeNode(node)).toBe(true)
  })

  it('只有 tool_calls、没有正文时也保留', () => {
    const node = chat(2, 'assistant', '', 1, {
      tool_calls: [{ id: 'c1', type: 'function', function: { name: 'x', arguments: '{}' } }],
    })
    expect(treeNode(node)).toBe(true)
  })

  it('没有正文的不显示', () => {
    expect(treeNode(chat(1, 'user', '   ', null))).toBe(false)
  })

  it('工具结果默认隐藏，取消勾选后显示', () => {
    const node = tool(1)
    expect(treeNode(node)).toBe(false)
    expect(treeNode(node, { ...defaultTreeFilters, hideTools: false })).toBe(true)
  })

  it('识别模式变更 system 节点', () => {
    const node: MessageRecord = {
      id: 5,
      session_id: 's',
      parent_id: 1,
      sort_key: 5,
      created_at: '2026-01-01T00:00:00.000Z',
      payload: {
        v: 1,
        role: 'system',
        kind: 'chat',
        status: 'complete',
        openai: { role: 'system', content: '[模式变更] ask → agent' },
        lya: {},
      },
    }
    expect(isModeChangeNode(node)).toBe(true)
    expect(treeNode(node, { ...defaultTreeFilters, hideModeChange: true })).toBe(false)
  })
})

describe('nodePreview', () => {
  it('tool-only assistant 显示工具名', () => {
    const node = chat(2, 'assistant', '', 1, {
      tool_calls: [{ id: 'c1', type: 'function', function: { name: 'bash', arguments: '{}' } }],
    })
    expect(nodePreview(node)).toBe('调用 bash')
  })
})

describe('projectVisibleTree', () => {
  it('工具可见时保留真实 parent', () => {
    const u1 = chat(1, 'user', '用一小段 Rust 代码', null)
    const a1 = chat(2, 'assistant', '喵~当然可以啦', 1)
    const t1 = tool(2)
    const u2 = chat(3, 'user', '再给我讲讲 Rc 吧', 99)
    const a2 = chat(4, 'assistant', 'Rc 就是…', 3)

    const raw = [u1, a1, t1, u2, a2]
    const filters = { ...defaultTreeFilters, hideTools: false }
    const visible = raw.filter((node) => treeNode(node, filters))
    const { nodes } = projectVisibleTree(raw, visible, 4)

    expect(nodes.map((n) => n.id)).toEqual([1, 2, 3, 4, 99])
    expect(nodes.find((n) => n.id === 3)?.parent_id).toBe(99)
  })

  it('隐藏工具时把子节点挂到可见祖先上', () => {
    const u1 = chat(1, 'user', '用一小段 Rust 代码', null)
    const a1 = chat(2, 'assistant', '喵~当然可以啦', 1)
    const hidden = tool(2)
    const u2 = chat(3, 'user', '再给我讲讲 Rc 吧', 99)
    const a2 = chat(4, 'assistant', 'Rc 就是…', 3)

    const raw = [u1, a1, hidden, u2, a2]
    const filters = { ...defaultTreeFilters, hideTools: true }
    const visible = raw.filter((node) => treeNode(node, filters))
    const { nodes } = projectVisibleTree(raw, visible, 4)

    expect(nodes.map((n) => n.id)).toEqual([1, 2, 3, 4])
    expect(nodes.find((n) => n.id === 3)?.parent_id).toBe(2)
  })

  it('active leaf 落在不可见节点上时回溯到可见祖先', () => {
    const u1 = chat(1, 'user', '你好', null)
    const hidden = tool(1)
    const visible = [u1]
    const { activeLeaf } = projectVisibleTree([u1, hidden], visible, 99)
    expect(activeLeaf).toBe(1)
  })
})
