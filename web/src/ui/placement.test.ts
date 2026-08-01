import { describe, expect, it } from 'vitest'

import { placeNear } from './placement'

const VIEWPORT = { width: 1000, height: 800 }
const MENU = { width: 200, height: 300 }

describe('placeNear', () => {
  it('放得下就贴着锚点', () => {
    expect(placeNear({ left: 100, top: 100 }, MENU, VIEWPORT)).toEqual({ left: 100, top: 100 })
  })

  it('右边放不下就往左翻，而不是贴着边挤', () => {
    // 在 x=900 右键，菜单宽 200，往右会超出；翻过去从 700 开始展开
    expect(placeNear({ left: 900, top: 100 }, MENU, VIEWPORT).left).toBe(700)
  })

  it('下边放不下就往上翻', () => {
    expect(placeNear({ left: 100, top: 700 }, MENU, VIEWPORT).top).toBe(400)
  })

  it('两边都放不下时压到边上，不跑出视口', () => {
    // 菜单比视口还宽，翻不翻都放不下
    const huge = { width: 1200, height: 300 }
    const placed = placeNear({ left: 500, top: 100 }, huge, VIEWPORT)
    expect(placed.left).toBeGreaterThanOrEqual(0)
  })

  it('贴着左上角时不会算出负坐标', () => {
    const placed = placeNear({ left: 0, top: 0 }, MENU, VIEWPORT)
    expect(placed.left).toBeGreaterThanOrEqual(0)
    expect(placed.top).toBeGreaterThanOrEqual(0)
  })

  it('恰好卡在边界上留出余量', () => {
    // 锚点 + 菜单正好等于视口宽，没有余量，应当翻转
    expect(placeNear({ left: 800, top: 100 }, MENU, VIEWPORT).left).toBe(600)
  })
})
