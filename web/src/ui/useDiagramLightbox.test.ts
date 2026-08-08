/**
 * 图表灯箱**给图定多大**这件事。
 *
 * 缩放和平移手感不在这里测：那要真实布局，happy-dom 量什么都是 0。这里守的是尺寸从
 * 哪儿来——出过事的正是这一处：mermaid 给的 SVG 是 `width="100%"` 配一条内联
 * `max-width`，灯箱为了能放大把 `max-width` 摘了，却留下 `width="100%"`，于是图被拉到
 * 舞台那么宽。一张 231×2566 的窄长图被撑成 1027×11423，「1 倍」从此不是原始大小而是
 * 一个跟着窗口宽度变的随机倍数，打开时只看得见顶上那 7%，还要缩到 0.07 倍才看得全——
 * 早穿过缩放下限了，也就是说「看全这张图」根本做不到。
 */

import { beforeEach, describe, expect, it } from 'vitest'

import { closeLightbox } from './lightbox'
import { openDiagramLightbox } from './useDiagramLightbox'

/** 造一张 mermaid 那个样子的 SVG：viewBox 是真尺寸，width 是 100%。 */
function fakeDiagram(width: number, height: number): SVGElement {
  const holder = document.createElement('div')
  holder.innerHTML =
    `<svg viewBox="0 0 ${width} ${height}" width="100%" style="max-width: ${width}px;">` +
    '<style>.node{fill:red}</style><rect width="10" height="10"/></svg>'
  return holder.querySelector('svg')!
}

function layer(): HTMLElement {
  return document.querySelector<HTMLElement>('.lya-lightbox__layer')!
}

/** 灯箱里那张图。它在影子树里，light DOM 上找不到。 */
function shown(): SVGElement {
  return layer().shadowRoot!.querySelector('svg')!
}

beforeEach(() => {
  closeLightbox()
  document.body.innerHTML = ''
})

describe('openDiagramLightbox', () => {
  it('按 viewBox 写死像素尺寸，不留 width="100%"', async () => {
    openDiagramLightbox(fakeDiagram(231, 2566), 'graph TD')

    expect(shown().getAttribute('width'), 'width="100%" 会让图被拉到舞台那么宽').toBe('231')
    expect(shown().getAttribute('height')).toBe('2566')
    expect(shown().getAttribute('style'), 'max-width 留着就放不大').toBeNull()
  })

  it('图层的盒子就是图的尺寸', async () => {
    // 位置全由 transform 说了算，图层撑满舞台再靠 flex 居中的话，图比舞台大的时候
    // flex 会悄悄改成靠左上，而缩放锚点还按居中算，缩放就开始漂
    openDiagramLightbox(fakeDiagram(231, 2566), 'graph TD')

    expect(layer().style.width).toBe('231px')
    expect(layer().style.height).toBe('2566px')
    expect(layer().style.transform, '打开时就该摆好').toMatch(/scale\(/)
  })

  it('页面上那张图不动', async () => {
    // 灯箱关掉之后气泡里还得有图，所以进灯箱的必须是克隆件
    const original = fakeDiagram(231, 2566)
    openDiagramLightbox(original, 'graph TD')

    expect(original.getAttribute('width')).toBe('100%')
    expect(original.getAttribute('style')).toContain('max-width')
  })
})
