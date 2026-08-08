import { describe, expect, it } from 'vitest'

import type { MessageRecord } from '../api/wire'
import {
  defaultTreeFilters,
  isModeChangeNode,
  layoutTree,
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

/**
 * 布局。
 *
 * 这段几何计算原先长在 `BranchTree.vue` 的一个 computed 里，于是最该有回归网的部分必须挂起
 * 整个组件才碰得到。用好读的小数字：节点 10×4、间距 2，留白 1。这样每个坐标都能手算核对。
 */
const M = { nodeW: 10, nodeH: 4, gapX: 2, gapY: 2, pad: 1 }

describe('layoutTree', () => {
  it('单链一路往下，横坐标不动', () => {
    const a = chat(1, 'user', '一', null)
    const b = chat(2, 'assistant', '二', 1)
    const c = chat(3, 'user', '三', 2)

    const { placed, edges, w, h } = layoutTree([a, b, c], 3, M)

    expect(placed.map((p) => [p.record.id, p.x, p.y])).toEqual([
      [1, 1, 1],
      [2, 1, 7],
      [3, 1, 13],
    ])
    // 每层往下一个 nodeH + gapY = 6
    expect(h).toBe(13 + 4 + 1)
    expect(w).toBe(1 + 10 + 1)
    expect(edges).toHaveLength(2)
    expect(edges.every((e) => e.onPath), '整条链都在当前分支上').toBe(true)
  })

  it('一处分叉：两个孩子分开排，父节点落在正中', () => {
    const root = chat(1, 'user', '问', null)
    const left = chat(2, 'assistant', '答一', 1)
    const right = chat(3, 'assistant', '答二', 1)

    const { placed } = layoutTree([root, left, right], 2, M)
    const at = (id: number) => placed.find((p) => p.record.id === id)!

    // 两个叶子的中心是 5 和 17，父节点落在 (5+17)/2 = 11，左上角 11-5+1 = 7
    expect(at(2).x).toBe(1)
    expect(at(3).x).toBe(13)
    expect(at(1).x).toBe(7)
    expect(at(1).y, '父节点在上一层').toBeLessThan(at(2).y)
  })

  it('只有走过的那一支算 onPath，另一支不算', () => {
    const root = chat(1, 'user', '问', null)
    const taken = chat(2, 'assistant', '选了这条', 1)
    const other = chat(3, 'assistant', '没选的', 1)

    const { placed, edges } = layoutTree([root, taken, other], 2, M)
    const at = (id: number) => placed.find((p) => p.record.id === id)!

    expect(at(1).onPath).toBe(true)
    expect(at(2).onPath).toBe(true)
    expect(at(3).onPath, '没走的那一支不该看起来像走过').toBe(false)
    // 连线要两端都在路上才算走过，否则分叉处那一笔会两条都高亮
    expect(edges.find((e) => e.x2 === at(3).x + M.nodeW / 2)!.onPath).toBe(false)
  })

  it('叶子认得出来，中间节点不算', () => {
    const root = chat(1, 'user', '一', null)
    const mid = chat(2, 'assistant', '二', 1)
    const leafA = chat(3, 'user', '三', 2)
    const leafB = chat(4, 'user', '四', 2)

    const { placed } = layoutTree([root, mid, leafA, leafB], 3, M)
    const leaves = placed.filter((p) => p.isLeaf).map((p) => p.record.id)
    expect(leaves).toEqual([3, 4])
  })

  it('父节点被过滤掉时，连线跨过去接到更上面', () => {
    // 过滤之后中间那层不在这批里。不跨过去的话下面整支看起来是断开的孤儿
    const root = chat(1, 'user', '一', null)
    const grandchild = chat(3, 'user', '三', 2) // 2 号不在传入的这批里
    const { edges } = layoutTree([root, { ...grandchild, parent_id: 1 }], 1, M)
    expect(edges).toHaveLength(1)
  })

  it('空树也给得出一块画布，不返回 NaN', () => {
    // Math.max() 无参数是 -Infinity，宽高一旦成了 NaN，整个 SVG 就不画了
    const { placed, edges, w, h } = layoutTree([], null, M)
    expect(placed).toEqual([])
    expect(edges).toEqual([])
    expect(w).toBe(11)
    expect(h).toBe(5)
  })

  it('节点很多也不爆栈', () => {
    // 用 Math.max(...arr) 求宽高时，数组长到几万就会 RangeError，而这个长度等于消息条数
    const long = Array.from({ length: 60_000 }, (_, i) =>
      chat(i + 1, 'user', String(i), i === 0 ? null : i),
    )
    expect(() => layoutTree(long, long.length, M)).not.toThrow()
  })
})
