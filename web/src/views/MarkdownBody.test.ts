/**
 * @vitest-environment jsdom
 *
 * `MarkdownBody` 的渲染后处理。
 *
 * 这里测的是**消毒之后那一步**：`model/markdown.ts` 只把公式认成一个装着 LaTeX
 * 的占位元素，真正排版是在这个组件里交给 KaTeX 的。两边分别测过不等于接得上，
 * 中间断一环的表现是公式永远停在原文状态，而两边的单测都还是绿的。
 *
 * 图表**画得出来**的样子不在这里测：那要真实布局（`getBBox`），jsdom 给不了。
 * 画不出来的几条路走得到，而它们恰恰是平时看不见、坏了也没人发现的那几条。
 */

import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import MarkdownBody from './MarkdownBody.vue'

async function render(text: string, raw = false) {
  const wrapper = mount(MarkdownBody, { props: { text, raw } })
  await flushPromises()
  return wrapper
}

/**
 * 等到 KaTeX 真的把公式画出来。
 *
 * 不能只 `flushPromises`：KaTeX 是动态 import 进来的，等的是一次真实的模块加载，
 * 不是一两个已经排上队的 microtask。
 */
async function renderMath(text: string) {
  const wrapper = await render(text)
  await vi.waitFor(() => expect(wrapper.find('.katex').exists()).toBe(true), { timeout: 5000 })
  return wrapper
}

describe('MarkdownBody', () => {
  it('占位元素最终被 KaTeX 排版出来', async () => {
    const wrapper = await renderMath('质能 $E=mc^2$ 关系')

    // 排完版就不该再有「待渲染」的占位残留
    expect(wrapper.find('.lya-math:not([data-done="1"])').exists()).toBe(false)
  })

  it('展示公式和行内公式分得开', async () => {
    const block = await renderMath('$$\\sum_{i=0}^{n} x_i$$')
    expect(block.find('.lya-math[data-display="1"]').exists()).toBe(true)
    expect(block.find('.katex-display').exists(), '展示公式该用 display 模式').toBe(true)

    const inline = await renderMath('行内 $x_1$ 公式')
    expect(inline.find('.katex-display').exists()).toBe(false)
  })

  it('公式写错了只红一处，不带塌整段正文', async () => {
    // 模型写错 LaTeX 是常事。一条坏公式让整条消息渲染不出来是不可接受的
    const wrapper = await render('前面 $\\frac{1}{$ 后面还有话')
    expect(wrapper.text()).toContain('后面还有话')
  })

  it('图表那一步塌了也不影响公式排版', async () => {
    // mermaid 是个上百万字节的大件，加载失败、渲染时抛，都是会发生的事。
    // 它和公式原先串在一条 await 链上，于是它一抛，后面的公式排版整段不执行——
    // 表现出来是「公式和图表两个功能一起坏了」，而真凶只有一个。
    vi.doMock('../ui/mermaid', () => {
      throw new Error('装作 mermaid 加载不起来')
    })

    const wrapper = await render('```mermaid\ngraph TD\nA-->B\n```\n\n但这条公式 $E=mc^2$ 得照常排版。')
    // 先确认这一轮真的走进了失败路径。少了这句，mock 没生效时它照样是绿的——
    // 那就成了一个永远不会红的测试，比没有更坏
    await vi.waitFor(
      () => expect(wrapper.find('.lya-diagram-error').text()).toContain('图表组件加载失败'),
      { timeout: 5000 },
    )
    await vi.waitFor(() => expect(wrapper.find('.katex').exists()).toBe(true), { timeout: 5000 })

    vi.doUnmock('../ui/mermaid')
  })

  it('还在输出时一次都不去渲染图表，说完了才画', async () => {
    /*
      边写边画是错的，不只是慢：半截的流程图往往是合法 mermaid（`graph TD` 加一条边
      就能解析），于是每个 delta 都画出一张不完整的图；而 html 是 computed，v-html 每次
      把整段 DOM 换掉，刚画好的又变回代码块再被画一遍。实测一条 612 字、三张图的回复
      要往页面上插 189 次图表元素（该是 3 次），看上去就是三块东西在疯狂闪。

      测的是「有没有去调 renderDiagram」而不是「页面上有没有图」。后者在 jsdom 里恒为
      「没有」——没有 getBBox，真的 mermaid 到布局那步一定倒——那样写出来的断言在缺陷
      版本下同样是绿的，等于没测。所以这里换成假的 mermaid：它一定成功，于是「画了」
      和「没画」才区分得开。
    */
    vi.resetModules()
    const renderDiagram = vi.fn().mockResolvedValue('<svg viewBox="0 0 10 10"></svg>')
    const explainDiagram = vi.fn().mockResolvedValue('不该问到这一句')
    vi.doMock('../ui/mermaid', () => ({ renderDiagram, explainDiagram }))

    const source = '```mermaid\ngraph TD\n    A[开始] --> B[结束]\n```'
    const wrapper = mount(MarkdownBody, { props: { text: '', streaming: true } })

    for (let at = 8; at <= source.length; at += 6) {
      await wrapper.setProps({ text: source.slice(0, at) })
      await flushPromises()
    }
    expect(renderDiagram, '流式期间一次都不该去渲染').not.toHaveBeenCalled()
    expect(wrapper.find('.lya-diagram').exists()).toBe(false)

    // 代码块得留在那儿——用户看到的是源码逐行长出来，而不是一块空白
    expect(wrapper.find('pre code.language-mermaid').exists()).toBe(true)
    expect(wrapper.text()).toContain('A[开始]')

    // 另一半：说完之后必须真的去画。少了这句，把 renderDiagrams 改成直接 return 也是绿的
    await wrapper.setProps({ streaming: false })
    await vi.waitFor(() => expect(wrapper.find('.lya-diagram').exists()).toBe(true), {
      timeout: 5000,
    })
    expect(renderDiagram, '整段只该画这一次').toHaveBeenCalledTimes(1)

    vi.doUnmock('../ui/mermaid')
    vi.resetModules()
  })

  it('话说完了还画不出来的图表，要说明原因', async () => {
    // 模型很爱写 `A[启动(初始化)]`——方括号里塞圆括号在 mermaid 里是语法错。
    // 不说的话页面上只剩一块代码块，看的人分不清是这张图写坏了还是功能坏了
    const source = '```mermaid\ngraph TD\n    A[启动(初始化)] --> B[结束]\n```'

    const streaming = mount(MarkdownBody, { props: { text: source, streaming: true } })
    await flushPromises()
    // 写到一半的图必然解析不通过，那时候报错纯属噪音
    expect(streaming.find('.lya-diagram-error').exists(), '还在输出时不该报错').toBe(false)

    await streaming.setProps({ streaming: false })
    await vi.waitFor(
      () => expect(streaming.find('.lya-diagram-error').exists()).toBe(true),
      { timeout: 5000 },
    )
    // 源码得留着：它往往正是用户要拿去让模型改的东西
    expect(streaming.text()).toContain('A[启动(初始化)]')
  })

  it('原文模式不产生任何 HTML', async () => {
    const source = '# 标题\n\n**粗体**与 $x^2$'
    const wrapper = await render(source, true)

    expect(wrapper.find('pre.md-raw').exists()).toBe(true)
    expect(wrapper.text()).toContain('**粗体**')
    expect(wrapper.find('h1').exists(), '原文模式不该解析 markdown').toBe(false)
    expect(wrapper.find('.katex').exists(), '原文模式不该渲染公式').toBe(false)
    // 走的是插值不是 v-html，所以源码一字不差
    expect(wrapper.find('pre.md-raw').text()).toBe(source)
  })
})
