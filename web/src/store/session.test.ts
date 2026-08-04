import { describe, expect, it } from 'vitest'

import type { LyaEvent, MessagePayload, MessageRecord, Snapshot } from '../api/wire'
import { applyEvents, applySnapshot, canSend, emptyState } from './session'

function record(id: number, payload: MessagePayload, parent: number | null = null): MessageRecord {
  return {
    id,
    session_id: 's',
    parent_id: parent,
    sort_key: id,
    payload,
    created_at: '2026-08-01T12:00:00Z',
  }
}

function draft(): MessagePayload {
  return {
    v: 1,
    role: 'assistant',
    kind: 'chat',
    status: 'streaming',
    openai: { role: 'assistant', content: '' },
    lya: {},
  }
}

function finished(text: string): MessagePayload {
  return {
    v: 1,
    role: 'assistant',
    kind: 'chat',
    status: 'complete',
    openai: { role: 'assistant', content: text },
    lya: {},
  }
}

function pendingHitl(): MessagePayload {
  return {
    v: 1,
    role: 'hitl',
    kind: 'form',
    status: 'pending',
    lya: { hitl: { type: 'form', form_id: 'f', title: '选一个', questions: [] } },
  }
}

/** 一轮正常的流式对话。 */
function normalTurn(id: number): LyaEvent[] {
  return [
    { type: 'round_started', round: 1 },
    { type: 'message_committed', record: record(id, draft()) },
    { type: 'reasoning_delta', text: '想一下' },
    { type: 'message_delta', text: '你' },
    { type: 'message_delta', text: '好' },
    { type: 'message_updated', record: record(id, finished('你好')) },
    { type: 'turn_end', reason: { kind: 'completed' } },
  ]
}

describe('applySnapshot', () => {
  it('整体替换而不是合并', () => {
    const snapshot: Snapshot = {
      session: {
        id: 's',
        title: '聊天',
        status: 'active',
        active_leaf_id: 1,
        work_mode: 'agent',
        persona: null,
        model_id: null,
        api_mode: 'completions',
        enabled_tools: null,
        created_at: '2026-08-01T12:00:00Z',
        updated_at: '2026-08-01T12:00:00Z',
      },
      messages: [record(1, finished('旧的'))],
      running: null,
    }
    // 后端在订阅者跟不上时会再补一份快照，那时必须整体替换才能对齐
    const stale = { ...emptyState(), messages: [record(99, finished('过期的'))] }
    const state = applySnapshot(stale, snapshot)
    expect(state.messages.map((m) => m.id)).toEqual([1])
    expect(state.meta?.title).toBe('聊天')
  })

  it('认出快照里已经挂起的 HITL', () => {
    const snapshot: Snapshot = {
      session: null as never,
      messages: [record(1, finished('问你个事')), record(2, pendingHitl(), 1)],
      running: null,
    }
    // 进程重启或换设备打开时，得知道现在正等着用户回答
    expect(applySnapshot(emptyState(), snapshot).pendingHitlId).toBe(2)
  })

  it('待确认后面还有 tool 结果时仍能识别 HITL', () => {
    const snapshot: Snapshot = {
      session: null as never,
      messages: [
        record(1, finished('问你个事')),
        record(2, pendingHitl(), 1),
        record(3, {
          v: 1,
          role: 'tool',
          kind: 'tool_result',
          status: 'complete',
          openai: { role: 'tool', content: 'ok', tool_call_id: 'c1' },
          lya: {},
        }, 2),
      ],
      running: null,
    }
    expect(applySnapshot(emptyState(), snapshot).pendingHitlId).toBe(2)
    expect(canSend(applySnapshot(emptyState(), snapshot))).toBe(false)
  })
})

describe('applyEvent', () => {
  it('增量累积成正文与思考', () => {
    const state = applyEvents(emptyState(), normalTurn(1).slice(0, 5))
    expect(state.running?.content).toBe('你好')
    expect(state.running?.reasoning).toBe('想一下')
    expect(state.running?.message_id).toBe(1)
  })

  it('定稿后丢掉缓冲，避免同一段话显示两遍', () => {
    const state = applyEvents(emptyState(), normalTurn(1))
    expect(state.running).toBeNull()
    expect(state.messages).toHaveLength(1)
    expect(state.messages[0]?.payload.openai?.content).toBe('你好')
  })

  it('消息被删掉时从列表里去掉，不留幽灵', () => {
    // 模型一个字都没说，占位消息会被清掉——不处理这条事件的话，
    // 界面上会留一个永远抹不掉的空气泡
    const state = applyEvents(emptyState(), [
      { type: 'round_started', round: 1 },
      { type: 'message_committed', record: record(1, draft()) },
      { type: 'message_deleted', id: 1 },
      { type: 'turn_end', reason: { kind: 'empty_response' } },
    ])
    expect(state.messages).toHaveLength(0)
    expect(state.running).toBeNull()
    expect(state.endReason).toEqual({ kind: 'empty_response' })
  })

  it('新一轮把上一轮的调用清掉', () => {
    const state = applyEvents(emptyState(), [
      { type: 'round_started', round: 1 },
      { type: 'call_started', call_id: 'c1', name: 'file_read', kind: 'tool' },
      { type: 'call_finished', call_id: 'c1', name: 'file_read', success: true },
      { type: 'round_started', round: 2 },
    ])
    // 不清的话上一轮的工具会一直挂在界面上
    expect(state.running?.calls).toEqual([])
    expect(state.running?.round).toBe(2)
  })

  it('调用完成时回填结果', () => {
    const state = applyEvents(emptyState(), [
      { type: 'round_started', round: 1 },
      { type: 'call_started', call_id: 'c1', name: 'bash', kind: 'tool' },
      { type: 'call_started', call_id: 'c2', name: 'form', kind: 'action' },
      { type: 'call_finished', call_id: 'c2', name: 'form', success: false },
    ])
    expect(state.running?.calls.map((c) => [c.call_id, c.ok])).toEqual([
      ['c1', null],
      ['c2', false],
    ])
  })

  it('没有轮次在跑时忽略残留的增量', () => {
    // 乱序到达的残留事件不该凭空造出一条来历不明的消息
    const state = applyEvents(emptyState(), [{ type: 'message_delta', text: '幽灵' }])
    expect(state.running).toBeNull()
  })

  it('同批多条 HITL 时 pending 指向第一条', () => {
    const hitl = (id: number, parent: number) =>
      record(id, { ...pendingHitl(), kind: 'tool_confirm' }, parent)
    const snapshot: Snapshot = {
      session: null as never,
      messages: [record(1, finished('批处理')), hitl(2, 1), hitl(3, 2)],
      running: null,
    }
    expect(applySnapshot(emptyState(), snapshot).pendingHitlId).toBe(2)
  })

  it('记录 tool_batch_started', () => {
    const state = applyEvents(emptyState(), [
      {
        type: 'tool_batch_started',
        batch_id: 'b1',
        message_id: 1,
        calls: [{ call_id: 'c1', name: 'bash', needs_review: true }],
      },
    ])
    expect(state.activeToolBatch?.batchId).toBe('b1')
  })

  it('挂起与结清 HITL', () => {
    const suspended = applyEvents(emptyState(), [
      { type: 'message_committed', record: record(2, pendingHitl(), 1) },
      { type: 'await_human', message_id: 2 },
      { type: 'turn_end', reason: { kind: 'awaiting_human' } },
    ])
    expect(suspended.pendingHitlId).toBe(2)
    expect(canSend(suspended)).toBe(false)

    const resolved = applyEvents(suspended, [
      {
        type: 'message_updated',
        record: record(2, { ...pendingHitl(), status: 'resolved' }, 1),
      },
    ])
    expect(resolved.pendingHitlId).toBeNull()
    expect(canSend(resolved)).toBe(true)
  })

  it('跑着的时候不能发新消息', () => {
    const running = applyEvents(emptyState(), [{ type: 'round_started', round: 1 }])
    expect(canSend(running)).toBe(false)
  })

  it('落库的消息按 sort_key 插入而不是按到达顺序', () => {
    const state = applyEvents(emptyState(), [
      { type: 'message_committed', record: record(5, finished('后来的')) },
      { type: 'message_committed', record: record(3, finished('先来的')) },
    ])
    expect(state.messages.map((m) => m.id)).toEqual([3, 5])
  })

  it('同一条消息更新时替换而不是追加', () => {
    const state = applyEvents(emptyState(), [
      { type: 'message_committed', record: record(1, draft()) },
      { type: 'message_updated', record: record(1, finished('改好了')) },
    ])
    expect(state.messages).toHaveLength(1)
    expect(state.messages[0]?.payload.status).toBe('complete')
  })

  it('失败原因留在状态上供界面显示', () => {
    const state = applyEvents(emptyState(), [
      { type: 'round_started', round: 1 },
      { type: 'turn_end', reason: { kind: 'failed', message: 'HTTP 401' } },
    ])
    expect(state.endReason).toEqual({ kind: 'failed', message: 'HTTP 401' })
    // 失败也要收掉缓冲，否则半截正文会一直挂着
    expect(state.running).toBeNull()
  })

  it('新一轮开始时清掉上一轮的结束原因', () => {
    const state = applyEvents(emptyState(), [
      { type: 'round_started', round: 1 },
      { type: 'turn_end', reason: { kind: 'failed', message: '401' } },
      { type: 'round_started', round: 1 },
    ])
    expect(state.endReason).toBeNull()
  })

  it('provider_search updates running buffer', () => {
    const state = applyEvents(emptyState(), [
      { type: 'round_started', round: 1 },
      { type: 'provider_search', call_id: 'ws1', phase: 'searching', query: 'Rust' },
      { type: 'provider_search', call_id: 'ws1', phase: 'completed', query: 'Rust' },
    ])
    expect(state.running?.provider_searches).toEqual([
      { call_id: 'ws1', phase: 'completed', query: 'Rust' },
    ])
  })

  it('不改动传进来的状态', () => {
    const before = emptyState()
    const snapshot = Object.freeze({ ...before })
    applyEvents(before, normalTurn(1))
    // 纯函数：整个流式生命周期能拿一串事件跑完，不碰网络也不改共享状态
    expect(before).toEqual(snapshot)
  })
})
