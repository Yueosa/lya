import { describe, expect, it } from 'vitest'

import type { MessagePayload, MessageRecord, TurnBuffer } from '../api/wire'
import { buildTimeline, type TimelineItem } from './timeline'

let nextId = 1
let clock = Date.parse('2026-08-01T10:00:00Z')

function record(payload: MessagePayload, options: { parent?: number | null; skipMs?: number } = {}): MessageRecord {
  clock += options.skipMs ?? 60_000
  const id = nextId++
  return {
    id,
    session_id: 's',
    parent_id: options.parent === undefined ? id - 1 || null : options.parent,
    sort_key: id,
    payload,
    created_at: new Date(clock).toISOString(),
  }
}

function user(text: string): MessagePayload {
  return {
    v: 1,
    role: 'user',
    kind: 'chat',
    status: 'complete',
    openai: { role: 'user', content: text },
    lya: {},
  }
}

function assistant(text: string, extras: Partial<MessagePayload> = {}): MessagePayload {
  return {
    v: 1,
    role: 'assistant',
    kind: 'chat',
    status: 'complete',
    openai: { role: 'assistant', content: text },
    lya: {},
    ...extras,
  }
}

function calling(text: string, callId: string, name: string, args: string): MessagePayload {
  return {
    v: 1,
    role: 'assistant',
    kind: 'tool_call',
    status: 'complete',
    openai: {
      role: 'assistant',
      content: text,
      tool_calls: [{ id: callId, type: 'function', function: { name, arguments: args } }],
    },
    lya: {},
  }
}

function toolResult(callId: string, content: string): MessagePayload {
  return {
    v: 1,
    role: 'tool',
    kind: 'tool_result',
    status: 'complete',
    openai: { role: 'tool', content, tool_call_id: callId },
    lya: {},
  }
}

function reset() {
  nextId = 1
  clock = Date.parse('2026-08-01T10:00:00Z')
}

function messageAt(items: TimelineItem[], index: number) {
  const found = items.filter((item) => item.kind === 'message')[index]
  return found?.kind === 'message' ? found.message : null
}

function lastMessageItem(items: TimelineItem[]) {
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i]!
    if (item.kind === 'message') return item
  }
  return null
}

describe('buildTimeline', () => {
  it('把用户与助手消息按顺序铺开', () => {
    reset()
    const items = buildTimeline({
      messages: [record(user('你好')), record(assistant('你好呀'))],
    })
    expect(items).toHaveLength(3)
    expect(items[0]).toMatchObject({ kind: 'time-gap' })
    expect(items[1]).toMatchObject({ kind: 'message', message: { role: 'user' } })
    expect(items[2]).toMatchObject({ kind: 'message', message: { role: 'assistant' } })
  })

  it('第一条消息前插会话开始时间', () => {
    reset()
    const items = buildTimeline({ messages: [record(user('你好'))] })
    expect(items.map((item) => item.kind)).toEqual(['time-gap', 'message'])
  })

  it('思考排在正文前面', () => {
    reset()
    const items = buildTimeline({
      messages: [record(assistant('结论', { lya: { reasoning: '先想一想' } }))],
    })
    const message = messageAt(items, 0)
    // 模型就是先想后说的，倒过来读着别扭
    expect(message?.blocks.map((b) => b.type)).toEqual(['reasoning', 'text'])
  })

  it('把工具结果并进调用块，不让它单独占一行', () => {
    reset()
    const items = buildTimeline({
      messages: [
        record(user('看看图片')),
        record(calling('我扫一下。', 'call_1', 'image_scan', '{"path":"~/图片"}')),
        record(toolResult('call_1', '共 12 张')),
      ],
    })

    // 三条消息进来，出去只有两项对话——工具结果被吸收了，开头多一条时间
    expect(items.filter((item) => item.kind === 'message')).toHaveLength(2)
    expect(items.some((item) => item.kind === 'time-gap')).toBe(true)
    const message = messageAt(items, 1)
    expect(message?.blocks.map((b) => b.type)).toEqual(['text', 'tool'])

    const tool = message?.blocks.find((b) => b.type === 'tool')
    expect(tool?.type === 'tool' && tool.call.result?.content).toBe('共 12 张')
    // 参数解析成对象供界面展示，同时保留原始串
    expect(tool?.type === 'tool' && tool.call.arguments).toEqual({ path: '~/图片' })
  })

  it('参数不是合法 JSON 时保留原始串而不是整条渲染失败', () => {
    reset()
    const items = buildTimeline({
      messages: [record(calling('', 'call_1', 'bash', '{坏掉的'))],
    })
    const message = messageAt(items, 0)
    const tool = message?.blocks[0]
    expect(tool?.type === 'tool' && tool.call.arguments).toBeUndefined()
    expect(tool?.type === 'tool' && tool.call.rawArguments).toBe('{坏掉的')
  })

  it('还没拿到结果的调用不给 result，界面据此显示执行中', () => {
    reset()
    const items = buildTimeline({
      messages: [record(calling('稍等', 'call_1', 'bash', '{}'))],
    })
    const message = messageAt(items, 0)
    const tool = message?.blocks.find((b) => b.type === 'tool')
    expect(tool?.type === 'tool' && tool.call.result).toBeUndefined()
  })

  it('隔太久插一行时间', () => {
    reset()
    const items = buildTimeline({
      messages: [record(user('早')), record(assistant('早'), { skipMs: 61 * 60_000 })],
    })
    expect(items.map((i) => i.kind)).toEqual(['time-gap', 'message', 'time-gap', 'message'])
  })

  it('间隔超过 10 分钟插一行时间', () => {
    reset()
    const items = buildTimeline({
      messages: [record(user('早')), record(assistant('晚'), { skipMs: 11 * 60_000 })],
    })
    expect(items.map((i) => i.kind)).toEqual(['time-gap', 'message', 'time-gap', 'message'])
  })

  it('隔得不久就不插', () => {
    reset()
    const items = buildTimeline({
      messages: [record(user('早')), record(assistant('早'), { skipMs: 9 * 60_000 })],
    })
    expect(items.map((i) => i.kind)).toEqual(['time-gap', 'message', 'message'])
  })

  it('跨天即使间隔不到半小时也要插', () => {
    reset()
    // 按**本地**午夜构造：用户关心的是自己这边跨没跨天，硬编 UTC 时刻在
    // 不同时区的机器上会得出不同结果
    const midnight = new Date(2026, 7, 2, 0, 0, 0)
    clock = midnight.getTime() - 20 * 60_000
    const items = buildTimeline({
      messages: [record(user('睡了'), { skipMs: 0 }), record(assistant('晚安'), { skipMs: 20 * 60_000 })],
    })
    expect(items.map((i) => i.kind)).toEqual(['time-gap', 'message', 'time-gap', 'message'])
  })

  it('系统消息变成居中提示而不是对话气泡', () => {
    reset()
    const system: MessagePayload = {
      v: 1,
      role: 'system',
      kind: 'chat',
      status: 'complete',
      openai: { role: 'system', content: '用户把工作模式切换为 agent。' },
      lya: {},
    }
    const items = buildTimeline({ messages: [record(user('切模式')), record(system)] })
    // 不显示的话，用户会看到助手突然能用某些工具却毫无提示
    expect(items[2]).toMatchObject({ kind: 'notice', text: '用户把工作模式切换为 agent。' })
  })

  it('HITL 节点带上用户当时的原始作答', () => {
    reset()
    const hitl: MessagePayload = {
      v: 1,
      role: 'hitl',
      kind: 'form',
      status: 'resolved',
      lya: {
        hitl: { type: 'form', form_id: 'f', title: '选一个', questions: [] },
        meta: { answer: { items: [{ question_id: 'q', values: ['a'] }] } },
      },
    }
    const items = buildTimeline({ messages: [record(hitl)] })
    const message = messageAt(items, 0)
    const block = message?.blocks[0]
    expect(block?.type).toBe('hitl')
    // 回看时要能原样回显勾选项，不必从渲染后的中文里反解
    expect(block?.type === 'hitl' && block.answer).toEqual({
      items: [{ question_id: 'q', values: ['a'] }],
    })
  })

  it('有兄弟节点时给出分支切换器', () => {
    reset()
    const u = record(user('你好'), { parent: null })
    const a1 = record(assistant('第一版'), { parent: u.id })
    const a2 = record(assistant('第二版'), { parent: u.id })

    // 当前分支走的是第二版，但整棵树里有两个兄弟
    const items = buildTimeline({ messages: [u, a2], tree: [u, a1, a2] })
    const message = messageAt(items, 1)
    expect(message?.branch).toEqual({ index: 1, total: 2, siblingIds: [a1.id, a2.id] })
  })

  it('tool 节点不算进兄弟列表', () => {
    reset()
    const u = record(user('你好'), { parent: null })
    const a1 = record(assistant('第一版'), { parent: u.id })
    const tool = record(
      {
        v: 1,
        role: 'tool',
        kind: 'tool_result',
        status: 'complete',
        openai: { role: 'tool', content: 'ok', tool_call_id: 'c1' },
        lya: {},
      },
      { parent: u.id },
    )
    const a2 = record(assistant('第二版'), { parent: u.id })
    const items = buildTimeline({ messages: [u, a2], tree: [u, a1, tool, a2] })
    expect(messageAt(items, 1)?.branch).toEqual({
      index: 1,
      total: 2,
      siblingIds: [a1.id, a2.id],
    })
  })

  it('没有分叉就不给切换器', () => {
    reset()
    const u = record(user('你好'), { parent: null })
    const a = record(assistant('回答'), { parent: u.id })
    const items = buildTimeline({ messages: [u, a], tree: [u, a] })
    const message = messageAt(items, 1)
    expect(message?.branch).toBeUndefined()
  })

  it('没给整棵树时不显示切换器而不是报错', () => {
    reset()
    const items = buildTimeline({ messages: [record(user('你好'))] })
    const message = messageAt(items, 0)
    expect(message?.branch).toBeUndefined()
  })

  it('把还没落库的这一轮接在末尾', () => {
    reset()
    const running: TurnBuffer = {
      round: 1,
      message_id: null,
      content: '正在说',
      reasoning: '正在想',
      calls: [],
    }
    const items = buildTimeline({ messages: [record(user('你好'))], running })
    const last = lastMessageItem(items)
    expect(last).toMatchObject({ kind: 'message', message: { status: 'streaming' } })
    const message = last?.kind === 'message' ? last.message : null
    expect(message?.blocks.map((b) => b.type)).toEqual(['reasoning', 'text'])
  })

  it('缓冲要盖住那条空占位，否则界面上没有流式输出', () => {
    reset()
    const u = record(user('你好'))
    // 后端先落一条空的助手消息，好让界面有 id 可挂增量；真正在长的字在缓冲里
    const draft = record(assistant('', { status: 'streaming' }))
    const running: TurnBuffer = {
      round: 1,
      message_id: draft.id,
      content: '正在一个字一个字地说',
      reasoning: '先想想',
      calls: [],
    }

    const items = buildTimeline({ messages: [u, draft], running })
    const last = lastMessageItem(items)
    const message = last?.kind === 'message' ? last.message : null

    // 只渲染落库那条的话，这里会是空的——那正是「发了消息但没有流式输出」
    expect(message?.blocks.map((b) => b.type)).toEqual(['reasoning', 'text'])
    const text = message?.blocks.find((b) => b.type === 'text')
    expect(text?.type === 'text' && text.text).toBe('正在一个字一个字地说')
    expect(message?.status).toBe('streaming')
    // 而且不能既画占位又画一条临时的，那样同一段话会出现两遍
    expect(items.filter((item) => item.kind === 'message')).toHaveLength(2)
  })

  it('缓冲里的工具调用也要显示出来', () => {
    reset()
    const draft = record(assistant('', { status: 'streaming' }))
    const running: TurnBuffer = {
      round: 1,
      message_id: draft.id,
      content: '',
      reasoning: '',
      calls: [{ call_id: 'c1', name: 'bash', kind: 'tool', ok: null }],
    }
    const items = buildTimeline({ messages: [draft], running })
    const message = messageAt(items, 0)
    const tool = message?.blocks.find((b) => b.type === 'tool')
    expect(tool?.type === 'tool' && tool.call.name).toBe('bash')
    // ok still null 表示还在跑
    expect(tool?.type === 'tool' && tool.call.result).toBeUndefined()
  })

  it('这一轮已经落库就不重复添一条', () => {
    reset()
    const u = record(user('你好'))
    const a = record(assistant('说完了'))
    const running: TurnBuffer = {
      round: 1,
      message_id: a.id,
      content: '说完了',
      reasoning: '',
      calls: [],
    }
    const items = buildTimeline({ messages: [u, a], running })
    expect(items.filter((item) => item.kind === 'message')).toHaveLength(2)
    expect(items.some((item) => item.kind === 'time-gap')).toBe(true)
  })

  it('失败要出错误项，否则界面转个圈就什么都没发生', () => {
    reset()
    const items = buildTimeline({
      messages: [record(user('你好'))],
      endReason: { kind: 'failed', message: 'HTTP 401' },
    })
    expect(items.at(-1)).toEqual({
      kind: 'error',
      reason: { kind: 'failed', message: 'HTTP 401' },
    })
  })

  it('正常收尾和等待用户都不算错误', () => {
    reset()
    for (const kind of ['completed', 'awaiting_human'] as const) {
      const items = buildTimeline({
        messages: [record(user('你好'))],
        endReason: { kind },
      })
      expect(items.some((i) => i.kind === 'error')).toBe(false)
    }
  })

  it('轮数打满与被取消也要说一声', () => {
    reset()
    for (const kind of ['max_rounds', 'cancelled', 'empty_response'] as const) {
      const items = buildTimeline({
        messages: [record(user('你好'))],
        endReason: { kind },
      })
      expect(items.at(-1)).toMatchObject({ kind: 'error', reason: { kind } })
    }
  })

  it('中断状态原样传给界面', () => {
    reset()
    const items = buildTimeline({
      messages: [record(assistant('说到一半', { status: 'interrupted' }))],
    })
    const message = messageAt(items, 0)
    // 不区分的话，用户分不清模型是说完了还是被打断了
    expect(message?.status).toBe('interrupted')
  })
})
