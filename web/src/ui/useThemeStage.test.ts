/**
 * 记忆大厅要记住用户挑的是哪一张。
 *
 * 它不是走马灯，是**挑一张长期看**的东西——每次回大厅重置到第一张说不过去。
 *
 * 重点是**按文件名记，不按下标**：素材目录是用户自己往里丢文件的，加一张或删一张
 * 之后下标就指到别的东西上了，而名字不会。这条测试盯的就是这个区别。
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * happy-dom 里没有 localStorage（`utils/storage` 因此一路静默降级），
 * 所以这里补一个内存桩，否则「记住选择」这件事根本无从验起——测试会绿，
 * 但绿的原因是什么都没存。
 */
const store = new Map<string, string>()
Object.defineProperty(globalThis, 'localStorage', {
  configurable: true,
  value: {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
    clear: () => store.clear(),
  },
})

const listed = vi.fn()

vi.mock('../app/chat/client', () => ({ client: { themeAssets: () => listed() } }))
vi.mock('../app/chat/state', () => {
  return { imageBootstrap: { value: { token: 't', home: '/home/x' } } }
})

const assets = (...names: string[]) => ({
  dir: '/tmp/cg',
  exists: true,
  assets: names.map((name) => ({ name, media: 'video' as const, bytes: 1 })),
})

async function stage() {
  const { useThemeStage } = await import('./useThemeStage')
  const s = useThemeStage({ theme: 'ba', kind: 'cg', remember: true })
  // load() 是构造时就发出去的，等它落地
  await vi.waitFor(() => expect(s.items.value.length).toBeGreaterThan(0))
  return s
}

describe('记忆大厅的选择', () => {
  beforeEach(() => {
    store.clear()
    vi.resetModules()
  })

  it('切换之后再打开，回到同一张', async () => {
    listed.mockResolvedValue(assets('a.mp4', 'b.mp4', 'c.mp4'))
    const first = await stage()
    first.go(2)
    expect(first.current.value?.name).toBe('c.mp4')

    // 重新挂一次 = 重新进大厅
    const again = await stage()
    expect(again.current.value?.name, '该回到上次挑的那张').toBe('c.mp4')
  })

  it('素材增删之后仍然认得出是哪一张', async () => {
    listed.mockResolvedValue(assets('a.mp4', 'b.mp4', 'c.mp4'))
    const first = await stage()
    first.go(2)

    // 在前面插一张：按下标记的话会跑到 b.mp4 上
    listed.mockResolvedValue(assets('new.mp4', 'a.mp4', 'b.mp4', 'c.mp4'))
    const again = await stage()
    expect(again.current.value?.name, '按名字记才不会指错').toBe('c.mp4')
  })

  it('记的那张被删掉就回到第一张', async () => {
    listed.mockResolvedValue(assets('a.mp4', 'b.mp4'))
    const first = await stage()
    first.go(1)

    listed.mockResolvedValue(assets('a.mp4'))
    const again = await stage()
    expect(again.current.value?.name).toBe('a.mp4')
  })
})
