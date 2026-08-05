/**
 * 主题预览必须渲染真组件。
 *
 * 这一页失真过一次：里面是一份手抄的 markup，导航少了后来加的「人设」「存储」，
 * 输入栏还留着早已搬走的模式段选择，代码块是裸 `<pre>` 而真实的代码块有语言条、
 * 复制按钮和行号。抄一份的代价就是每加一个组件都得记得回来补，漏了不报错——只是
 * 换主题时看不出哪里没配好。
 *
 * 所以这里不检查长相（那是眼睛的事），只检查**预览里出现的是真组件的产物**：
 * 一旦有人把它退回手抄，或者真组件的结构变了而预览没跟上，这些断言就会红。
 */

import { mount } from '@vue/test-utils'
import { flushPromises } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { NAV_ITEMS } from '../shell/types'
import ThemePreview from './ThemePreview.vue'
import { SAMPLE_MARKDOWN } from './themeSamples'
import { vTip } from './vTip'

async function render(themeId: string) {
  const wrapper = mount(ThemePreview, {
    props: { themeId },
    global: { directives: { tip: vTip } },
  })
  // MarkdownBody 的高亮与行号是渲染之后用 DOM 操作补的，要等它跑完
  await flushPromises()
  await new Promise((resolve) => requestAnimationFrame(resolve))
  return wrapper
}

describe('主题预览', () => {
  it('导航项跟着 NAV_ITEMS 走，不手写', async () => {
    for (const themeId of ['mtf', 'mc']) {
      const wrapper = await render(themeId)
      const text = wrapper.text()
      for (const item of NAV_ITEMS) {
        expect(text, `${themeId} 缺少导航项「${item.label}」`).toContain(item.label)
      }
    }
  })

  it('代码块是 MarkdownBody 的真实产物：语言条 + 行号', async () => {
    const wrapper = await render('mtf')

    expect(wrapper.find('.md-code').exists(), '缺少代码块外层').toBe(true)
    expect(wrapper.find('.md-bar').exists(), '缺少语言条').toBe(true)

    const gutter = wrapper.find('.md-code__lines')
    expect(gutter.exists(), '缺少行号槽').toBe(true)

    // 行号数要跟样例里围栏代码块的真实行数对上。从样例反推而不是写死一串数字：
    // 写死的话改样例就得记得回来改这里，而忘了改只会让断言变成一句空话
    const fence = SAMPLE_MARKDOWN.match(/```rust\n([\s\S]*?)```/)
    expect(fence, '样例里得有一段 rust 围栏代码').not.toBeNull()
    const lines = fence![1]!.replace(/\n$/, '').split('\n').length
    expect(gutter.text().split('\n')).toEqual(
      Array.from({ length: lines }, (_, i) => String(i + 1)),
    )
  })

  it('覆盖到近几轮加的组件', async () => {
    const wrapper = await render('mtf')

    // 折叠块（工具 / 思考）
    const folds = wrapper.findAll('.fold')
    expect(folds.length, '折叠块至少要有三态').toBeGreaterThanOrEqual(3)
    expect(wrapper.find('.fold--failed').exists(), '缺少失败态折叠块').toBe(true)

    // 跳到最新：三种可见状态都要在
    const jumps = wrapper.findAll('.chat__jump')
    expect(jumps.length).toBe(3)
    expect(wrapper.find('.chat__jump--follow').exists()).toBe(true)
    expect(wrapper.find('.chat__jump--done').exists()).toBe(true)

    // 媒体附注：路径条与失败占位
    expect(wrapper.find('.lya-chat-media-path').exists()).toBe(true)
    expect(wrapper.find('.lya-chat-media-error').exists()).toBe(true)

    // 存储占用条（StorageBreakdown 的真实产物）
    expect(wrapper.find('.storage').exists(), '缺少存储占用').toBe(true)
  })

  it('气泡三态都在', async () => {
    const wrapper = await render('mtf')
    expect(wrapper.find('.bubble--user').exists()).toBe(true)
    expect(wrapper.find('.bubble--assistant').exists()).toBe(true)
    expect(wrapper.find('.bubble--interrupted').exists()).toBe(true)
  })
})
