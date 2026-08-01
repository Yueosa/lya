/**
 * 拿真实后端跑出来的事件流验一遍。
 *
 * 别的测试用的都是我手写的事件——形状照着 wire dump 抄的，但「事件以什么顺序、
 * 什么节奏到达」是我猜的。这份夹具是连上真后端、真发一句话、把 SSE 原样录下来
 * 的 98 条事件，所以它验的是**猜得对不对**，而不是我自己跟自己对答案。
 *
 * 录制方式：订阅 `/api/sessions/{id}/subscribe`，发一条消息，把 `event:` 与
 * `data:` 成对存成 JSON。后端消息结构变了就重录一份。
 */

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { toEvent } from '../api/client'
import type { Envelope, Snapshot } from '../api/wire'
import { buildTimeline } from '../model/timeline'
import { applyEvent, applySnapshot, canSend, emptyState, isRunning } from './session'

interface Recorded {
  event: string
  data: Envelope
}

const recorded: Recorded[] = JSON.parse(
  readFileSync(join(import.meta.dirname, '__fixtures__/real-turn.json'), 'utf8'),
)

/** 像界面那样把整条流走一遍。 */
function replay(upTo = recorded.length) {
  let state = emptyState()
  for (const item of recorded.slice(0, upTo)) {
    if (item.event === 'snapshot') {
      state = applySnapshot(state, item.data.payload as unknown as Snapshot)
      continue
    }
    const event = toEvent(item.data)
    if (event) state = applyEvent(state, event)
  }
  return state
}

describe('真实事件流', () => {
  it('每条事件都认得出来', () => {
    const unknown = recorded
      .filter((item) => item.event !== 'snapshot')
      .filter((item) => toEvent(item.data) === null)
      .map((item) => item.event)
    // 认不出说明前端类型和后端对不上，那才是最难查的那种偏差
    expect(unknown).toEqual([])
  })

  it('到达顺序和 store 的假设一致', () => {
    const order: string[] = []
    for (const item of recorded) {
      if (order.at(-1) !== item.event) order.push(item.event)
    }
    expect(order).toEqual([
      'snapshot',
      // 用户自己发的那条。第一次录制时它根本不在流里——用户消息由 hub 写进树，
      // 而 agent 只为自己做的事发事件，于是发消息的人自己看不到自己发的消息
      'message_committed',
      'round_started',
      // 助手的空占位
      'message_committed',
      'reasoning_delta',
      'message_delta',
      // 定稿。没有这个事件的话，界面手里会一直是那条空占位
      'message_updated',
      'turn_end',
    ])
  })

  it('放完之后落到一个干净的收尾状态', () => {
    const state = replay()
    expect(state.endReason).toEqual({ kind: 'completed' })
    // 缓冲要清掉，否则同一段正文会和落库的那条重影
    expect(state.running).toBeNull()
    expect(isRunning(state)).toBe(false)
    expect(canSend(state)).toBe(true)
  })

  it('时间线是「一问一答」两条消息', () => {
    const items = buildTimeline({
      messages: replay().messages,
      running: null,
      endReason: { kind: 'completed' },
    })
    const roles = items
      .filter((item) => item.kind === 'message')
      .map((item) => (item.kind === 'message' ? item.message.role : ''))
    expect(roles).toEqual(['user', 'assistant'])
    // 正常收尾不该冒出错误项
    expect(items.some((item) => item.kind === 'error')).toBe(false)
  })

  it('思考与正文分成两块，不会混在一起', () => {
    const items = buildTimeline({ messages: replay().messages, running: null })
    const reply = items.at(-1)
    const blocks = reply?.kind === 'message' ? reply.message.blocks : []
    expect(blocks.map((block) => block.type)).toEqual(['reasoning', 'text'])
  })

  it('中途停下时缓冲里是已经流出来的那半截', () => {
    // 停在最后一条 message_delta 之后、定稿之前
    const beforeFinal = recorded.findIndex((item) => item.event === 'message_updated')
    const state = replay(beforeFinal)

    expect(isRunning(state)).toBe(true)
    expect(canSend(state)).toBe(false)
    expect(state.running?.content.length).toBeGreaterThan(0)
    expect(state.running?.reasoning.length).toBeGreaterThan(0)

    // 这时界面上应当已经能看到正在写的内容
    const items = buildTimeline({ messages: state.messages, running: state.running })
    const last = items.at(-1)
    expect(last?.kind === 'message' && last.message.status).toBe('streaming')
  })

  it('增量拼起来就是最终落库的正文', () => {
    const deltas = recorded
      .filter((item) => item.event === 'message_delta')
      .map((item) => item.data.payload['text'] as string)
      .join('')

    const finalRecord = recorded.filter((item) => item.event === 'message_updated').at(-1)
    const stored = (finalRecord?.data.payload as { record: { payload: { openai: { content: string } } } })
      .record.payload.openai.content

    // 对不上就说明增量丢了或者重复了，那在界面上表现为吞字或者复读
    expect(deltas).toBe(stored)
  })
})
