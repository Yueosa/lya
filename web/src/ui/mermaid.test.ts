/**
 * 图表渲染的**失败姿势**。
 *
 * 画得出来的样子不在这里测：那要真实布局，happy-dom 给不了。这里守的是画不出来
 * 时的行为，而那恰恰是出过事的地方——配色一步抛出去的异常曾经一路窜到调用方，
 * 把整条增强链带走，连「这张图画不出来」的提示都没轮到执行。
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

const initialize = vi.fn()
const parse = vi.fn()
const render = vi.fn()

vi.mock('mermaid', () => ({ default: { initialize, parse, render } }))

beforeEach(() => {
  vi.resetModules()
  document.body.innerHTML = ''
  initialize.mockReset().mockReturnValue(undefined)
  parse.mockReset().mockResolvedValue(true)
  render.mockReset().mockResolvedValue({ svg: '<svg></svg>' })
})

/**
 * 学 mermaid 渲染失败时的样子：临时容器留在 body 里，自己抛出去。
 *
 * 它画图是先往 body 挂一个 `<div id="d{id}">` 当画板，成功了才收走；倒在绘制那一步
 * 时它收不干净，那个 div 就留下了。
 */
function leaveTempDiv(): void {
  render.mockImplementation(async (id: string) => {
    const div = document.createElement('div')
    div.id = `d${id}`
    div.innerHTML = '<svg id="' + id + '"><text>Syntax error in text</text></svg>'
    document.body.appendChild(div)
    throw new Error('Cannot read properties of undefined (reading \'x\')')
  })
}

/** 每次都重新 import：模块里缓存了「上次按哪套主题配过」。 */
async function load() {
  return import('./mermaid')
}

describe('renderDiagram', () => {
  it('配色这一步抛了也只是画不出来，不会把异常扔给调用方', async () => {
    // 主题把颜色写成 color-mix() 时，mermaid 内部的 khroma 解不了就在
    // initialize 里抛。这个异常原先窜到 MarkdownBody，令那一屏的公式排版
    // 和失败提示统统不执行——一处配色问题，表现成整套功能坏掉
    initialize.mockImplementation(() => {
      throw new Error('Unsupported color format: "color(srgb 0.4 0.8 0.99 / 0.3)"')
    })

    const { renderDiagram } = await load()
    await expect(renderDiagram('graph TD\nA-->B', 'ba')).resolves.toBeNull()
  })

  it('源码没写完就安静回落', async () => {
    parse.mockResolvedValue(false)

    const { renderDiagram } = await load()
    expect(await renderDiagram('graph T', 'default')).toBeNull()
    expect(render, '没通过解析就不该再去渲染').not.toHaveBeenCalled()
  })

  it('画不出来时不在 body 里留下那张「Syntax error」', async () => {
    // 语法过了、倒在绘制那一步时，mermaid 会把报错画进它挂在 body 上的临时容器再抛，
    // 容器就留下了。那是张 width="100%" 的 SVG，挂在 body 末尾等于糊在整个界面上，
    // 每失败一次多一张——用户截图里一次糊了四张
    leaveTempDiv()

    const { renderDiagram } = await load()
    expect(await renderDiagram('sequenceDiagram\nactivate C', 'default')).toBeNull()
    expect(document.body.textContent).not.toContain('Syntax error')
    expect(document.body.children.length, 'body 里不该多出东西').toBe(0)
  })
})

describe('configure', () => {
  it('不让 mermaid 自己画报错', async () => {
    // 报错怎么呈现由调用方决定（代码块下面挂一条说明）。交给 mermaid 的话它画到
    // 自己那个临时容器里，而失败路上它收不干净
    const { renderDiagram } = await load()
    await renderDiagram('graph TD\nA-->B', 'default')
    expect(initialize).toHaveBeenCalledWith(
      expect.objectContaining({ suppressErrorRendering: true }),
    )
  })
})

describe('explainDiagram', () => {
  it('倒在渲染那一步时，说的是真正的原因', async () => {
    // 只 parse 一遍是不够的：语法没问题、倒在后面的情况真的发生过，
    // 那时候回一句「语法能通过」等于什么都没说
    parse.mockResolvedValue(true)
    render.mockRejectedValue(new Error('khroma 解不了这个颜色'))

    const { explainDiagram } = await load()
    expect(await explainDiagram('graph TD\nA-->B', 'ba')).toContain('khroma 解不了这个颜色')
  })

  it('问一句原因也不该在 body 里留东西', async () => {
    // 这条路会把失败的渲染原样再走一遍，于是同一个坑要踩第二次：一张画不出来的图
    // 「渲染 + 问原因」是两张残留，流式结束后再重跑一遍就是四张
    leaveTempDiv()

    const { explainDiagram } = await load()
    expect(await explainDiagram('sequenceDiagram\nactivate C', 'default')).toContain('reading')
    expect(document.body.children.length).toBe(0)
  })

  it('语法错就报语法错，带上 mermaid 那段列指示', async () => {
    parse.mockRejectedValue({ str: "Parse error on line 2:\n----^\nExpecting 'SQE', got 'PS'" })

    const { explainDiagram } = await load()
    const why = await explainDiagram('graph TD\nA[启动(初始化)]', 'default')
    expect(why).toContain('Parse error on line 2')
    expect(why, '带列指示的那段比 message 有用得多').toContain('----^')
  })
})
