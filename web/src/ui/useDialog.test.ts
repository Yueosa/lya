import { beforeEach, describe, expect, it, vi } from 'vitest'

import { accept, cancel, confirm, confirmAsync, dialogState, prompt, setValue } from './useDialog'

beforeEach(() => {
  // 上一条测试若留了个开着的弹窗，取消掉
  cancel()
})

describe('confirm', () => {
  it('确认得到 true，取消得到 false', async () => {
    const yes = confirm({ title: '删除？' })
    expect(dialogState.open).toBe(true)
    void accept()
    await expect(yes).resolves.toBe(true)
    expect(dialogState.open).toBe(false)

    const no = confirm({ title: '删除？' })
    cancel()
    await expect(no).resolves.toBe(false)
  })

  it('再弹一个会把前一个当作取消，不留悬着的 Promise', async () => {
    const first = confirm({ title: '第一个' })
    const second = confirm({ title: '第二个' })
    // 不处理的话 first 永远不兑现，调用方会卡死
    await expect(first).resolves.toBe(false)
    expect(dialogState.title).toBe('第二个')
    void accept()
    await expect(second).resolves.toBe(true)
  })
})

describe('prompt', () => {
  it('确认得到文本，取消得到 null', async () => {
    const answer = prompt({ title: '改名', initial: '旧名字' })
    expect(dialogState.value).toBe('旧名字')
    setValue('新名字')
    void accept()
    await expect(answer).resolves.toBe('新名字')

    const cancelled = prompt({ title: '改名' })
    cancel()
    await expect(cancelled).resolves.toBeNull()
  })
})

describe('confirmAsync', () => {
  it('执行期间弹窗不关，做完才关', async () => {
    let release: (() => void) | null = null
    const work = new Promise<void>((resolve) => {
      release = resolve
    })

    const done = confirmAsync({ title: '清空', run: () => work })
    void accept()
    await vi.waitFor(() => expect(dialogState.busy).toBe(true))
    // 底下的活还在跑，这时候关掉会让人以为什么都没发生
    expect(dialogState.open).toBe(true)

    release!()
    await expect(done).resolves.toBe(true)
    expect(dialogState.open).toBe(false)
  })

  it('执行期间不给取消', async () => {
    let release: (() => void) | null = null
    const work = new Promise<void>((resolve) => {
      release = resolve
    })
    const done = confirmAsync({ title: '清空', run: () => work })
    void accept()
    await vi.waitFor(() => expect(dialogState.busy).toBe(true))

    cancel()
    expect(dialogState.open).toBe(true)

    release!()
    await done
  })

  it('失败时留在弹窗里报错，让用户能直接重试', async () => {
    let attempts = 0
    const done = confirmAsync({
      title: '保存',
      run: async () => {
        attempts += 1
        if (attempts === 1) throw new Error('网络不通')
      },
    })

    void accept()
    await vi.waitFor(() => expect(dialogState.error).toBe('网络不通'))
    // 关掉再弹一个报错的话，用户已经失去上下文了
    expect(dialogState.open).toBe(true)
    expect(dialogState.busy).toBe(false)

    void accept()
    await expect(done).resolves.toBe(true)
    expect(attempts).toBe(2)
  })
})
