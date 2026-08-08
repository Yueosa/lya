/**
 * 图表的影子树宿主。
 *
 * 单独放一个模块是为了**不把 mermaid 拖进主包**：灯箱和正文都要挂图表，而灯箱
 * 是常驻代码。这几个函数一行 mermaid 都不碰，渲染那部分才需要懒加载。
 *
 * 为什么非要影子树见 `ui/mermaid.ts`：mermaid 的 SVG 自带 `<style>`，而内联
 * SVG 里的 `<style>` 是作用于整个文档的。
 */

/** 影子树里没有外部样式，图表的尺寸约束得跟着一起塞进去。 */
const HOST_STYLE = `
  :host { display: block; }
  svg { display: block; max-width: 100%; height: auto; margin: 0 auto; }
`

/**
 * 把 SVG 挂进宿主元素的影子树。
 *
 * 影子树只建一次，之后重复调用只换内容——同一个元素上再 `attachShadow` 会抛错。
 */
export function mountDiagram(host: HTMLElement, svg: string): void {
  const shadow = host.shadowRoot ?? host.attachShadow({ mode: 'open' })
  shadow.innerHTML = `<style>${HOST_STYLE}</style>${svg}`
}

/** 取出影子树里那张图，供灯箱克隆。 */
export function diagramSvg(host: HTMLElement): SVGElement | null {
  return host.shadowRoot?.querySelector('svg') ?? null
}
