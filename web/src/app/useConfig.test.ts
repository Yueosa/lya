/**
 * 共享配置：几屏读同一份，而且改动之后会跟上。
 *
 * 钉的是两个真实症状：
 *
 * 1. **四处各拉一遍。** 设置页、工具页、人设页、会话设置页原先各自 `client.config()`，
 *    互不知情。
 * 2. **改完别处不刷新。** 后端写完配置就广播 `config_changed`，前端也收到了，但只用它刷
 *    模型列表——配置本身没人刷。表现最难看的一处是会话设置页里的全局人设：挂载时读一次,
 *    之后在人设页改完，那一行显示的还是旧正文，而界面上没有任何迹象说明它过期了。
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const config = vi.fn()

vi.mock('./client', () => ({ client: { config } }))

async function load() {
  vi.resetModules()
  return import('./useConfig')
}

function payload(persona: string) {
  return { persona, runtime: { agent: {} }, models: [] }
}

beforeEach(() => {
  config.mockReset().mockResolvedValue(payload('第一版人设'))
})

describe('ensureConfig', () => {
  it('几屏一起要也只拉一次', async () => {
    const { ensureConfig, configState } = await load()

    // 同一瞬间几屏挂载：并发这条要靠 inflight 挡住，先后那条要靠已有值挡住
    await Promise.all([ensureConfig(), ensureConfig(), ensureConfig()])
    await ensureConfig()

    expect(config).toHaveBeenCalledTimes(1)
    expect(configState.defaultPersona.value).toBe('第一版人设')
  })

  it('读失败留下原因，而且不把上一份好数据抹掉', async () => {
    const { ensureConfig, reloadConfig, configState } = await load()
    await ensureConfig()

    config.mockRejectedValue(new Error('连接被拒绝'))
    await reloadConfig()

    expect(configState.error.value).toBe('连接被拒绝')
    expect(
      configState.defaultPersona.value,
      '重取失败时界面该继续显示上一份，而不是突然变空',
    ).toBe('第一版人设')
  })
})

describe('onConfigChanged', () => {
  it('配置变了，共享的那份跟着变', async () => {
    const { ensureConfig, onConfigChanged, configState } = await load()
    await ensureConfig()
    expect(configState.defaultPersona.value).toBe('第一版人设')

    // 在别处改了人设，后端广播过来
    config.mockResolvedValue(payload('改过的人设'))
    onConfigChanged()
    await vi.waitFor(() => expect(configState.defaultPersona.value).toBe('改过的人设'))

    expect(config).toHaveBeenCalledTimes(2)
  })

  it('重取一定真的发请求，不被「已经有了」挡住', async () => {
    // 这是 ensureConfig 和 reloadConfig 的分工：前者是「没有就去拿」，后者是「无论如何
    // 再拿一次」。混成一个的话，广播来了也刷不动——那就回到了这个 bug 本身
    const { ensureConfig, reloadConfig } = await load()
    await ensureConfig()
    await reloadConfig()
    expect(config).toHaveBeenCalledTimes(2)
  })
})
