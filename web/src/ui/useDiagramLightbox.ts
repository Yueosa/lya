/**
 * 图表灯箱：放大、滚轮缩放、拖动平移，以及复制/保存。
 *
 * 图表和图片不一样的地方在于它值得放大——流程图一复杂，气泡那点宽度里就只剩
 * 一团糊的线。所以这里比图片灯箱多了缩放和平移。
 *
 * 外壳（遮罩、工具栏、Esc）复用 `lightbox.ts`。
 */

import { diagramSvg } from './diagramHost'
import { openLightbox, type LightboxAction } from './lightbox'
import { toast } from './useToast'

/** 缩放上限。再大也只是看像素。 */
const MAX_SCALE = 8

/**
 * 缩放下限。
 *
 * 只是「一般情况下别缩成一个点」，装不下舞台的图另算——见 [`bindZoomPan`] 里的
 * `floor()`，那种图的下限得低到能看全为止。
 */
const MIN_SCALE = 0.2

/**
 * 打开时最多放大到几倍。
 *
 * 打开灯箱就是为了看清，所以小图该放大到铺满舞台；但三个方块的图铺满 1600px 会变成
 * 一张海报，字大得反而难读。2 倍是「明显比气泡里大」和「还像张图」之间那道线。
 */
const MAX_INITIAL_SCALE = 2

/** 导出位图时的倍率：贴到别处去的图，1 倍会糊。 */
const RASTER_SCALE = 2

interface Size {
  width: number
  height: number
}

function themeColor(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

/**
 * 这张图本身多大。
 *
 * 读 `viewBox` 而不是量元素：mermaid 给的 SVG 是 `width="100%"` 配一条 `max-width`，
 * 量出来的是「在当前这个容器里被摆成了多大」，换个容器就换一个数，而且灯箱里那份还
 * 带着缩放的 transform，量出来是「现在看着多大」。viewBox 里那两个数才是图自己的尺寸。
 */
function naturalSize(svg: SVGElement): Size {
  const view = (svg.getAttribute('viewBox') ?? '').split(/[\s,]+/).filter(Boolean).map(Number)
  const [, , width, height] = view
  if (view.length === 4 && width && height && width > 0 && height > 0) {
    return { width: Math.round(width), height: Math.round(height) }
  }
  // 没有 viewBox 的图不是 mermaid 画的，那就退回量一遍，总比 300×150 的默认值靠谱
  const box = svg.getBoundingClientRect()
  return {
    width: Math.max(1, Math.round(box.width)),
    height: Math.max(1, Math.round(box.height)),
  }
}

/**
 * 把 SVG 序列化成能独立打开的一份。
 *
 * 必须补上 `width`/`height` 和 xmlns：页面里的 SVG 靠 CSS 撑开尺寸，脱离页面之后
 * 没有 CSS，不写死尺寸的话浏览器会按 300×150 的默认值画，导出来就是一角截图。
 */
function serialize(svg: SVGElement): { text: string; width: number; height: number } {
  const { width, height } = naturalSize(svg)

  const clone = svg.cloneNode(true) as SVGElement
  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
  clone.setAttribute('xmlns:xlink', 'http://www.w3.org/1999/xlink')
  clone.setAttribute('width', String(width))
  clone.setAttribute('height', String(height))

  return { text: new XMLSerializer().serializeToString(clone), width, height }
}

async function toPngBlob(svg: SVGElement): Promise<Blob> {
  const { text, width, height } = serialize(svg)
  const url = URL.createObjectURL(new Blob([text], { type: 'image/svg+xml;charset=utf-8' }))
  try {
    const image = new Image()
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve()
      image.onerror = () => reject(new Error('svg decode failed'))
      image.src = url
    })

    const canvas = document.createElement('canvas')
    canvas.width = width * RASTER_SCALE
    canvas.height = height * RASTER_SCALE
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('no 2d context')

    // 不铺底色的话导出的是透明背景，贴到白底的聊天窗口里，深色主题的字就没了
    ctx.fillStyle = themeColor('--bg', '#ffffff')
    ctx.fillRect(0, 0, canvas.width, canvas.height)
    ctx.drawImage(image, 0, 0, canvas.width, canvas.height)

    return await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error('toBlob failed'))), 'image/png')
    })
  } finally {
    URL.revokeObjectURL(url)
  }
}

function download(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  anchor.click()
  URL.revokeObjectURL(url)
}

/**
 * 一个滚轮事件该把倍率乘上多少。
 *
 * 按 `deltaY` 的大小算，不是每个事件固定一档：触控板两指一划会发出几十个 `deltaY` 只有
 * 个位数的事件，固定一档的话手指还没停就已经顶到上限了。指数形式让「滚同样的距离」在
 * 任何倍率下都是同样的视觉变化量，也顺手保证了往回滚一下就正好回到原来那个倍率。
 *
 * 除数取 700 是为了让鼠标滚轮的一格（`deltaY` 为 120 左右）差不多是 1.2 倍。
 */
function zoomStep(event: WheelEvent): number {
  // deltaMode 是「行」或「页」时 deltaY 是行数页数，得先换算成像素，不然一格只动一点点
  const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 400 : 1
  // 有些设备会甩出上千的 deltaY，不夹一下的话一个事件就从头缩到底
  const delta = Math.max(-120, Math.min(120, event.deltaY * unit))
  return Math.exp(-delta / 700)
}

/**
 * 给舞台装上缩放与平移，返回解绑函数。
 *
 * 缩放以光标为锚点，不是以中心：放大一张流程图时，你盯着的多半是某个角落的节点，
 * 按中心缩放会把它推出视野，然后就得靠拖动找回来。
 *
 * 位置全部由这里的 transform 说了算，样式表里不做任何居中——图层贴着舞台左上角，
 * 尺寸就是图的尺寸。两边各摆一次的话，图比舞台大的时候 flex 会悄悄把居中改成靠左上，
 * 而这里的锚点算式还按居中算，缩放就开始漂。
 */
function bindZoomPan(stage: HTMLElement, layer: HTMLElement, size: Size): () => void {
  let scale = 1
  let x = 0
  let y = 0
  let dragging = false
  let lastX = 0
  let lastY = 0

  function apply(): void {
    layer.style.transform = `translate(${x}px, ${y}px) scale(${scale})`
  }

  /** 整张图刚好装进舞台的倍率。 */
  function fitScale(): number {
    const box = stage.getBoundingClientRect()
    // 还没上屏时量不出尺寸，别拿 0 去做除数
    if (box.width <= 0 || box.height <= 0) return 1
    return Math.min(box.width / size.width, box.height / size.height)
  }

  /**
   * 这张图能缩到多小。
   *
   * 下限得跟着图放宽，不能真的是个常数：长图不难长到超过舞台五倍（一张画满分支的时序图
   * 四五千像素高是常事，而舞台只有 800 上下），那时候「整张看得见」需要的倍率就掉到 0.2
   * 以下，被常数挡在外面——灯箱最该做到的一件事，恰好做不到。
   */
  function floor(): number {
    return Math.min(MIN_SCALE, fitScale())
  }

  /** 摆成「整张图居中」。打开时和双击时都用它。 */
  function fit(): void {
    const box = stage.getBoundingClientRect()
    scale = Math.min(MAX_INITIAL_SCALE, fitScale())
    x = (box.width - size.width * scale) / 2
    y = (box.height - size.height * scale) / 2
    apply()
  }

  function onWheel(event: WheelEvent): void {
    event.preventDefault()
    const box = stage.getBoundingClientRect()
    const px = event.clientX - box.left
    const py = event.clientY - box.top
    const next = Math.min(MAX_SCALE, Math.max(floor(), scale * zoomStep(event)))
    const ratio = next / scale
    // 让光标下的那个点在缩放前后落在同一处
    x = px - (px - x) * ratio
    y = py - (py - y) * ratio
    scale = next
    apply()
  }

  function onPointerDown(event: PointerEvent): void {
    dragging = true
    lastX = event.clientX
    lastY = event.clientY
    stage.setPointerCapture(event.pointerId)
    stage.classList.add('lya-lightbox__stage--dragging')
  }

  function onPointerMove(event: PointerEvent): void {
    if (!dragging) return
    x += event.clientX - lastX
    y += event.clientY - lastY
    lastX = event.clientX
    lastY = event.clientY
    apply()
  }

  function onPointerUp(event: PointerEvent): void {
    dragging = false
    if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId)
    stage.classList.remove('lya-lightbox__stage--dragging')
  }

  stage.addEventListener('wheel', onWheel, { passive: false })
  stage.addEventListener('pointerdown', onPointerDown)
  stage.addEventListener('pointermove', onPointerMove)
  stage.addEventListener('pointerup', onPointerUp)
  stage.addEventListener('pointercancel', onPointerUp)
  stage.addEventListener('dblclick', fit)
  fit()

  return () => {
    stage.removeEventListener('wheel', onWheel)
    stage.removeEventListener('pointerdown', onPointerDown)
    stage.removeEventListener('pointermove', onPointerMove)
    stage.removeEventListener('pointerup', onPointerUp)
    stage.removeEventListener('pointercancel', onPointerUp)
    stage.removeEventListener('dblclick', fit)
  }
}

/**
 * 打开图表灯箱。
 *
 * `svg` 会被克隆，页面上那份不动——否则关掉灯箱之后气泡里就空了。
 */
export function openDiagramLightbox(svg: SVGElement, source: string): void {
  const stage = document.createElement('div')
  stage.className = 'lya-lightbox__stage'

  const layer = document.createElement('div')
  layer.className = 'lya-lightbox__layer'
  // 克隆件同样带着那个 `<style>`，所以这里也得关进影子树，理由见 ui/mermaid.ts
  const shadow = layer.attachShadow({ mode: 'open' })
  const copy = svg.cloneNode(true) as SVGElement

  /*
    尺寸写死成像素，两个属性都得换掉。

    mermaid 给的是 `width="100%"` 配一条内联 `max-width`。`max-width` 留着就放不大，
    但只去掉它、留下 `width="100%"`，图就会被拉到舞台那么宽——一张 231×2566 的窄长图
    被撑成 1027×11423，于是「1 倍」不再是原始大小，而是一个跟着窗口宽度变的随机倍数，
    打开时只看得见顶上那 7%，而且要缩到 0.07 倍才看得全，早就穿过缩放下限了。
  */
  const size = naturalSize(copy)
  copy.removeAttribute('style')
  copy.setAttribute('width', String(size.width))
  copy.setAttribute('height', String(size.height))
  shadow.innerHTML = '<style>:host{display:block}svg{display:block}</style>'
  shadow.appendChild(copy)
  layer.style.width = `${size.width}px`
  layer.style.height = `${size.height}px`
  stage.appendChild(layer)

  const actions: LightboxAction[] = [
    {
      label: '复制图片',
      onSelect: async () => {
        try {
          const blob = await toPngBlob(copy)
          await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
          toast('图表已复制', 'success')
        } catch {
          toast('复制图表失败', 'error')
        }
      },
    },
    {
      label: '复制源码',
      onSelect: async () => {
        try {
          await navigator.clipboard.writeText(source)
          toast('已复制', 'success')
        } catch {
          toast('复制失败', 'error')
        }
      },
    },
    {
      label: '保存 SVG',
      onSelect: () => {
        const { text } = serialize(copy)
        download(new Blob([text], { type: 'image/svg+xml;charset=utf-8' }), 'diagram.svg')
        toast('已开始下载', 'success')
      },
    },
  ]

  // 先上屏再绑：适配倍率要量舞台，而没进文档的元素量出来是 0
  let unbind = (): void => {}
  openLightbox({
    actions,
    body: stage,
    panelClass: 'lya-lightbox__panel--wide',
    onClose: () => unbind(),
  })
  unbind = bindZoomPan(stage, layer, size)
}

/** 给渲染好的图表绑点击放大。 */
export function bindDiagramZoom(container: HTMLElement): void {
  for (const host of Array.from(container.querySelectorAll<HTMLElement>('.lya-diagram'))) {
    if (host.dataset['bound'] === '1') continue
    host.dataset['bound'] = '1'
    host.style.cursor = 'zoom-in'
    host.addEventListener('click', () => {
      // 图在影子树里，light DOM 上 querySelector 是找不到的
      const svg = diagramSvg(host)
      const source = host.dataset['source']
      if (svg && source) openDiagramLightbox(svg, source)
    })
  }
}
