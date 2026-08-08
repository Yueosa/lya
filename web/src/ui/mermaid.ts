/**
 * mermaid 懒加载入口：把图表源码渲染成 SVG，并负责把它安全地挂进页面。
 *
 * # 为什么要挂进 shadow root
 *
 * mermaid 产出的 SVG 里带一个 `<style>`。内联 SVG 里的 `<style>` **作用于整个
 * 文档**，不是只作用于这张图——而正文是模型写的，`model/markdown.ts` 之所以把
 * `style` 标签列进禁止清单，就是因为一段 CSS 足以把整个界面盖住做成钓鱼页。
 * 让图表带着 `<style>` 进来，等于把刚锁上的门又开了。
 *
 * 挂进 shadow root 之后这件事从结构上就不可能了：影子树里的 CSS 出不去。顺带
 * 还解决了反方向的麻烦——主题的全局样式也进不来，不会去和 mermaid 自己那套
 * 配色打架。挂载那几个函数在 `ui/diagramHost.ts`，那边不依赖 mermaid。
 */

import DOMPurify from 'dompurify'
import mermaid from 'mermaid'

/** 渲染好的 SVG 缓存上限。流式输出时同一张图会被反复要，不缓存就是反复重画。 */
const CACHE_LIMIT = 60

const cache = new Map<string, string>()
let seq = 0
let configuredFor: string | null = null

function token(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim()
  return value || fallback
}

/** 取色器：token 名进去，mermaid 认得的具体颜色出来。用完记得 `done()`。 */
interface Palette {
  color(name: string, fallback: string): string
  done(): void
}

/**
 * 造一个取色器。
 *
 * 不能直接用 [`token`]，中间隔着两道坎，缺一道图就画不出来：
 *
 * 1. **算式得算完。** 自定义属性读出来是字面值，而主题里的颜色常写成
 *    `color-mix(in srgb, 某个颜色 30%, transparent)` 这样的算式。塞进一个真实元素
 *    的 `color` 再读回来，浏览器一定会替我们算完。
 * 2. **写法得压回老式 `rgb()`。** 上一步在 Chrome 里算出来的是
 *    `color(srgb 0.4 0.79 0.98 / 0.3)`，而 mermaid 内部用 khroma 调明暗，它只认
 *    `#rgb` / `rgb()`，见到别的当场抛 `Unsupported color format`。canvas 是最省事
 *    的归一化器：什么写法填进去，读回来都是四个整数。
 *
 * 半透明顺手合成到主题背景上。留着 alpha 也能跑，但 khroma 会拿它去推边框明暗，
 * 推出来的颜色和用户在界面别处看到的那个对不上。
 *
 * 这不是假想中的隐患：ba 主题的 `--accent-soft`、`--surface-hover`、
 * `--surface-active` 都是 `color-mix()`，于是那套主题下的图表**一张都画不出来**，
 * 而别的主题好好的——只坏一套皮肤，最难往颜色上想。
 */
function palette(): Palette {
  const probe = document.createElement('span')
  probe.style.display = 'none'
  document.body.appendChild(probe)

  const canvas = document.createElement('canvas')
  canvas.width = 1
  canvas.height = 1
  const ctx = canvas.getContext('2d', { willReadFrequently: true })

  const computed = (name: string, fallback: string): string => {
    probe.style.color = `var(--${name}, ${fallback})`
    return getComputedStyle(probe).color || fallback
  }

  // 底色先定下来，后面每个半透明都合成到它上面
  const backdrop = computed('bg', '#ffffff')

  return {
    color(name, fallback) {
      const value = computed(name, fallback)
      if (!ctx) return value
      ctx.clearRect(0, 0, 1, 1)
      // 认不出来的写法会让赋值原地失效，所以每次都先垫一层保底再覆盖
      ctx.fillStyle = fallback
      ctx.fillStyle = backdrop
      ctx.fillRect(0, 0, 1, 1)
      ctx.fillStyle = fallback
      ctx.fillStyle = value
      ctx.fillRect(0, 0, 1, 1)
      const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data
      return `rgb(${r}, ${g}, ${b})`
    },
    done() {
      probe.remove()
    },
  }
}

/**
 * 让 mermaid 的配色跟着主题走。
 *
 * 用 `base` 主题再逐项给值，而不是挑一个它内置的深色/浅色主题：内置主题是一整套
 * 写死的颜色，换到别的主题上必然有一处不搭，而这里的每种主题连圆角和边框粗细
 * 都不一样。
 */
function configure(theme: string): void {
  const pal = palette()
  const color = (name: string, fallback: string) => pal.color(name, fallback)

  try {
    mermaid.initialize({
      startOnLoad: false,
      // 图表源码来自模型，不能让它注册点击回调或塞进原始 HTML
      securityLevel: 'strict',
      // 画不出来的图由调用方自己说，不要它往 body 里塞那张「Syntax error」，见 [`renderOnce`]
      suppressErrorRendering: true,
      theme: 'base',
      // 字体不是颜色，原样给就行
      fontFamily: token('font-ui', 'sans-serif'),
      themeVariables: {
        background: color('bg', '#ffffff'),
        mainBkg: color('surface', '#ffffff'),
        primaryColor: color('accent-soft', '#eef2ff'),
        primaryTextColor: color('text', '#111111'),
        primaryBorderColor: color('accent', '#4f46e5'),
        secondaryColor: color('surface-hover', '#f3f4f6'),
        tertiaryColor: color('surface-active', '#e5e7eb'),
        lineColor: color('border-strong', '#9ca3af'),
        textColor: color('text', '#111111'),
        nodeBorder: color('border-strong', '#9ca3af'),
        clusterBkg: color('bg-sunken', '#f9fafb'),
        clusterBorder: color('border', '#e5e7eb'),
        titleColor: color('text', '#111111'),
        edgeLabelBackground: color('bg-sunken', '#f9fafb'),
        errorBkgColor: color('danger-soft', '#fee2e2'),
        errorTextColor: color('danger', '#dc2626'),
      },
    })
  } finally {
    pal.done()
  }
  configuredFor = theme
}

function remember(key: string, svg: string): void {
  cache.set(key, svg)
  if (cache.size > CACHE_LIMIT) {
    const oldest = cache.keys().next().value
    if (oldest !== undefined) cache.delete(oldest)
  }
}

/**
 * 渲染一次，并保证 mermaid 不会在 `document.body` 里留下东西。
 *
 * mermaid 画图是先往 `document.body` 挂一个临时 `<div id="d{id}">`，在里面画完再读走
 * `innerHTML`，最后自己把 div 收掉。**失败的路上它收不干净**：语法过了、倒在绘制那一
 * 步时，它会把一张写着「Syntax error in text」的 SVG 画进那个 div 然后抛出去，div 就
 * 留在 body 里了。那是张 `width="100%"` 的图，挂在 body 末尾等于糊在整个界面上，而且
 * 每失败一次多一张——一张画不出来的图，配上流式结束后的重跑和紧跟着的 [`explainDiagram`]，
 * 一次就能糊上四张。
 *
 * `suppressErrorRendering` 关掉了「画报错」，但它只护住 mermaid 自己 try 起来的两段；
 * 配色算不出来时抛在那两段之外（见 [`configure`] 里 khroma 那档事），临时 div 照样留下。
 * 所以这里按 id 再兜一遍：id 是我们给的，收拾自己的东西不用猜。
 */
async function renderOnce(id: string, source: string): Promise<string> {
  try {
    const { svg } = await mermaid.render(id, source)
    return svg
  } finally {
    document.getElementById(`d${id}`)?.remove()
    document.getElementById(id)?.remove()
  }
}

/** 从 mermaid 抛出来的东西里取一句能给人看的话。 */
function reason(err: unknown): string {
  if (err && typeof err === 'object') {
    // mermaid 的解析错误把带列指示的那段放在 str 上，比 message 有用得多
    const shaped = err as { str?: unknown; message?: unknown }
    if (typeof shaped.str === 'string' && shaped.str) return shaped.str
    if (typeof shaped.message === 'string' && shaped.message) return shaped.message
  }
  return String(err)
}

/**
 * 问出一段源码到底错在哪。
 *
 * 只在源码**确定不会再变**的时候问：流式输出中的图必然有半截的一刻，那时候的
 * 报错全是噪音，所以热路径上的 [`renderDiagram`] 依旧安静地回落，解释单独要。
 */
export async function explainDiagram(source: string, theme: string): Promise<string> {
  try {
    // 把失败那条路原样再走一遍，不抑制报错。只 parse 是不够的：语法没问题、
    // 倒在渲染那一步的情况真的发生过（主题颜色是 color-mix 算式，mermaid 解不了），
    // 而那时候「语法能通过」这句话等于什么都没说
    if (configuredFor !== theme) configure(theme)
    await mermaid.parse(source)
    seq += 1
    await renderOnce(`lya-diagram-why-${seq}`, source)
    return '重试时又画出来了，刷新一下试试。'
  } catch (err) {
    return reason(err)
  }
}

/**
 * 渲染一张图，失败返回 `null`。
 *
 * 失败是**正常情况**而不是异常：流式输出时每张图都必然经历一段源码还没写完的
 * 时间，那时候解析不通过是理所当然的。所以先 `parse` 验一遍再渲染，让调用方
 * 安静地回落成代码块，而不是闪一片红色报错。
 *
 * 但「安静」只该持续到这段话说完为止。源码不再变还是画不出来，那就是模型真的
 * 写错了，调用方该拿 [`explainDiagram`] 问一句再告诉用户——否则页面上只剩一块
 * 代码块，看的人分不清是这张图写坏了还是整个功能坏了。
 */
export async function renderDiagram(source: string, theme: string): Promise<string | null> {
  const key = `${theme}\n${source}`
  const hit = cache.get(key)
  if (hit !== undefined) return hit

  try {
    // 配色也在 try 里面：它同样会抛（主题颜色写成算式时 khroma 解不了），而放在
    // 外面的那阵子，这个异常会一路窜出去，把调用方整条增强链一起带走——连「图画
    // 不出来」的提示都轮不到执行，页面上只剩一块没有下文的代码块
    if (configuredFor !== theme) configure(theme)

    // suppressErrors 让它返回 false 而不是抛，半截源码不该在控制台刷一片栈
    const ok = await mermaid.parse(source, { suppressErrors: true })
    if (!ok) return null

    seq += 1
    const svg = await renderOnce(`lya-diagram-${seq}`, source)
    // 影子树挡住了 CSS 外泄，脚本这类还是照常清掉
    const clean = DOMPurify.sanitize(svg, {
      USE_PROFILES: { svg: true, svgFilters: true },
      ADD_TAGS: ['style', 'foreignObject'],
      FORBID_TAGS: ['script', 'iframe', 'object', 'embed'],
    })
    remember(key, clean)
    return clean
  } catch {
    return null
  }
}

