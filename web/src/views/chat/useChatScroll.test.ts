import { describe, expect, it } from 'vitest'

import { computeTimelineWindow, nextShownFrom } from './useChatScroll'

describe('computeTimelineWindow', () => {
  it('uses renderTail during session enter', () => {
    expect(computeTimelineWindow(200, 48, null)).toEqual({ offset: 152, start: 152 })
  })

  it('defaults to MAX_VISIBLE tail when not expanded', () => {
    expect(computeTimelineWindow(150, null, null)).toEqual({ offset: 30, start: 30 })
    expect(computeTimelineWindow(120, null, null)).toEqual({ offset: 0, start: 0 })
  })

  it('honors shownFrom = 0 as fully expanded', () => {
    expect(computeTimelineWindow(150, null, 0)).toEqual({ offset: 0, start: 0 })
  })

  it('prefers shownFrom over renderTail after loadEarlier', () => {
    expect(computeTimelineWindow(200, 48, 92)).toEqual({ offset: 92, start: 92 })
  })
})

describe('nextShownFrom', () => {
  it('steps back by chunk until zero', () => {
    expect(nextShownFrom(80)).toBe(20)
    expect(nextShownFrom(30)).toBe(0)
    expect(nextShownFrom(0)).toBe(0)
  })
})

describe('loadEarlier regression', () => {
  it('first click from auto window can reach full timeline', () => {
    const before = computeTimelineWindow(150, null, null)
    expect(before.offset).toBe(30)

    const after = computeTimelineWindow(150, null, nextShownFrom(before.offset))
    expect(after).toEqual({ offset: 0, start: 0 })
  })
})
