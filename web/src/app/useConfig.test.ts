/**
 * 共享配置：几屏读同一份，而且改动之后会跟上。
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const config = vi.fn()

vi.mock('./client', () => ({ client: { config } }))

async function load() {
  vi.resetModules()
  return import('./useConfig')
}

function payload(identity: string) {
  return {
    prompt: {
      environment: '',
      operations: '',
      voice: '',
      identity,
      style: '',
    },
    runtime: { agent: {} },
    models: [],
  }
}

beforeEach(() => {
  config.mockReset().mockResolvedValue(payload('第一版身份'))
})

describe('ensureConfig', () => {
  it('几屏一起要也只拉一次', async () => {
    const { ensureConfig, configState } = await load()

    await Promise.all([ensureConfig(), ensureConfig(), ensureConfig()])
    await ensureConfig()

    expect(config).toHaveBeenCalledTimes(1)
    expect(configState.prompt.value?.identity).toBe('第一版身份')
  })

  it('读失败留下原因，而且不把上一份好数据抹掉', async () => {
    const { ensureConfig, reloadConfig, configState } = await load()
    await ensureConfig()

    config.mockRejectedValue(new Error('连接被拒绝'))
    await reloadConfig()

    expect(configState.error.value).toBe('连接被拒绝')
    expect(configState.prompt.value?.identity).toBe('第一版身份')
  })
})

describe('onConfigChanged', () => {
  it('配置变了，共享的那份跟着变', async () => {
    const { ensureConfig, onConfigChanged, configState } = await load()
    await ensureConfig()
    expect(configState.prompt.value?.identity).toBe('第一版身份')

    config.mockResolvedValue(payload('改过的身份'))
    onConfigChanged()
    await vi.waitFor(() => expect(configState.prompt.value?.identity).toBe('改过的身份'))

    expect(config).toHaveBeenCalledTimes(2)
  })

  it('重取一定真的发请求，不被「已经有了」挡住', async () => {
    const { ensureConfig, reloadConfig } = await load()
    await ensureConfig()
    await reloadConfig()
    expect(config).toHaveBeenCalledTimes(2)
  })
})
