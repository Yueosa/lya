/**
 * `runtime.toml` ↔ 设置页表单的双向映射。
 *
 * 最要紧的是**往返**那一条：读进来再写回去，值不变。这段映射有两个方向、二十来个字段，加一项
 * 只改一边不会报错、也不会有别的测试红，表现是「设置页能填，保存后没生效」或者「保存后这一项
 * 被清成默认值」。往返测试正好卡住这种漏。
 */

import { describe, expect, it } from 'vitest'

import { RUNTIME_FORM_DEFAULTS, readRuntimeForm, runtimeFormPayload } from './runtimeForm'
import { megabytesToBytes } from '../utils/formatBytes'

/**
 * 一份每项都和默认值不同的配置，这样「漏了一边」必然表现成值变回默认。
 *
 * 留存开关默认全是 `true`，所以这里必须全填 `false`——填 `true` 的那几项等于没测。这个坑第一
 * 次写就踩了，靠下面「每一项都确实不是默认值」那条自检抓出来的。`true` 那一侧由「空配置」那条
 * 覆盖：空配置读出来全是 `true`，写回再读还得是 `true`。
 */
const FULL = {
  agent: {
    max_tool_rounds: 7,
    max_parallel_tools: 5,
    max_consecutive_tool_failures: 9,
    default_work_mode: 'ask',
    default_api_mode: 'responses',
    default_model: 'pro',
  },
  tools: { enabled: ['bash', 'file_read'] },
  memory: { max_index_entries: 11, max_index_chars: 2222, index_summary_chars: 33 },
  shell: { confirm: 'always' },
  media: {
    image: { max_bytes: megabytesToBytes(8), retain_local: false, retain_web: false },
    video: { max_bytes: megabytesToBytes(64), retain_local: false, retain_web: false },
    audio: { max_bytes: megabytesToBytes(16), retain_local: false, retain_web: false },
  },
}

describe('往返', () => {
  it('读进来再写回去，每一项都还在', () => {
    const once = readRuntimeForm(FULL)
    const twice = readRuntimeForm(runtimeFormPayload(once) as Record<string, unknown>)

    expect(twice.form).toEqual(once.form)
    expect(twice.toolsMode).toBe(once.toolsMode)
    expect([...twice.toolsEnabled].sort()).toEqual([...once.toolsEnabled].sort())
  })

  it('读出来的每一项都确实不是默认值', () => {
    // 少了这句，上面那条在「两边都漏了同一项」时照样绿：漏掉的项两次都读成默认值，
    // 前后一致，往返看起来是好的
    const { form } = readRuntimeForm(FULL)
    const same = Object.keys(form).filter(
      (key) =>
        form[key as keyof typeof form] === RUNTIME_FORM_DEFAULTS[key as keyof typeof form],
    )
    expect(same, '这些字段没被 FULL 覆盖到，往返测不到它们').toEqual([])
  })

  it('空配置读出来就是默认值，写回去再读还是它', () => {
    const once = readRuntimeForm({})
    expect(once.form).toEqual(RUNTIME_FORM_DEFAULTS)

    const twice = readRuntimeForm(runtimeFormPayload(once) as Record<string, unknown>)
    expect(twice.form).toEqual(RUNTIME_FORM_DEFAULTS)
  })
})

describe('读取', () => {
  it('媒体上限在字节和 MB 之间换算', () => {
    const { form } = readRuntimeForm({ media: { image: { max_bytes: 33_554_432 } } })
    expect(form.maxImageMb).toBe(32)
  })

  it('留存开关只有显式 false 才算关', () => {
    // 缺省是开。写成 `!!x` 的话缺省变成关，用户的媒体会静默不留
    const off = readRuntimeForm({ media: { image: { retain_local: false } } })
    expect(off.form.retainLocal).toBe(false)

    const missing = readRuntimeForm({ media: { image: {} } })
    expect(missing.form.retainLocal, '没写就是开').toBe(true)
  })

  it('手改 TOML 打错成非数字时退回默认，不让 NaN 进表单', () => {
    // NaN 会让输入框空着，保存时又把 NaN 写回配置
    const { form } = readRuntimeForm({ agent: { max_tool_rounds: '不是数字' } })
    expect(form.maxToolRounds).toBe(RUNTIME_FORM_DEFAULTS.maxToolRounds)
  })

  it('api 栈只认 responses，别的都当 completions', () => {
    expect(readRuntimeForm({ agent: { default_api_mode: 'responses' } }).form.defaultApiMode).toBe(
      'responses',
    )
    expect(readRuntimeForm({ agent: { default_api_mode: '瞎写的' } }).form.defaultApiMode).toBe(
      'completions',
    )
  })
})

describe('写回', () => {
  it('没选默认模型时写 null，把这个键删掉', () => {
    // 空串是非法 id，会被后端启动校验拦下来
    const state = readRuntimeForm({})
    const payload = runtimeFormPayload(state) as { agent: { default_model: unknown } }
    expect(payload.agent.default_model).toBeNull()
  })

  it('工具三种模式各写成不同的东西', () => {
    const asPayload = (runtime: Record<string, unknown>) =>
      runtimeFormPayload(readRuntimeForm(runtime)) as { tools: { enabled: unknown } }

    // 没有 enabled 这个键 = 全部启用，写回时用 null 删键
    expect(asPayload({}).tools.enabled).toBeNull()
    // 空数组 = 一个都不启用。这和「全部启用」在界面上是两个选项，写成同一个东西就串了
    expect(asPayload({ tools: { enabled: [] } }).tools.enabled).toEqual([])
    expect(asPayload({ tools: { enabled: ['b', 'a'] } }).tools.enabled).toEqual(['a', 'b'])
  })

  it('每一项都写出来，不因为等于默认值就省略', () => {
    // 省略的含义是「跟着将来的默认值变」，和「我明确选了这个数」不是一回事
    const payload = runtimeFormPayload(readRuntimeForm({})) as {
      agent: Record<string, unknown>
      memory: Record<string, unknown>
      shell: Record<string, unknown>
    }
    expect(payload.agent['max_tool_rounds']).toBe(RUNTIME_FORM_DEFAULTS.maxToolRounds)
    expect(payload.memory['max_index_chars']).toBe(RUNTIME_FORM_DEFAULTS.maxIndexChars)
    expect(payload.shell['confirm']).toBe(RUNTIME_FORM_DEFAULTS.shellConfirm)
  })
})
