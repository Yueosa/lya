/**
 * Markdown 消毒之后那一步：高亮、公式、图表、代码块顶栏。
 *
 * 从 `MarkdownBody.vue` 里拆出来，是因为这些函数不认识 Vue——它们吃的是一段已经
 * 挂进文档的 DOM，吐的还是 DOM。锁在 `<script setup>` 里的代价是：测它们必须
 * mount 整个组件，而组件还带着主题、偏好、imageContext 一堆无关的东西。
 *
 * 组件那边只剩三件事：算 html、盯着 html / streaming / themeId 变了就来叫这里、
 * 以及那一大坨样式。
 *
 * # 轮次编号
 *
 * 流式时正文每来一个增量就重渲一次，而公式和图表都要等动态 import 落地才画得出来，
 * 于是同一瞬间可能有好几轮在飞。带上 [`EnhanceSession`] 里的 generation，过期的那几轮
 * 自己退场——否则一张刚渲染好的旧图会盖回已经更新过的正文上。
 */

import hljs from './hljs'
import { MATH_CLASS } from '../model/math'
import { mountDiagram } from './diagramHost'
import { bindChatImages } from './useImageLightbox'
import { bindChatMediaPaths } from './useChatMedia'
import { bindDiagramZoom } from './useDiagramLightbox'

/** 一轮增强所需的外部状态。 */
export interface EnhanceOptions {
  /** 这段正文还会继续长——图表在这期间一律不画，见 [`renderDiagrams`]。 */
  streaming: boolean
  /** 当前主题 id；图表配色是渲染时烤进 SVG 的。 */
  theme: string
}

/**
 * 一次挂载对应一份会话。
 *
 * generation 由调用方在每次启动增强前递增；进行中的异步步骤看到自己那一号过期了就退场。
 */
export interface EnhanceSession {
  generation: number
}

/** 开一份新会话。 */
export function createEnhanceSession(): EnhanceSession {
  return { generation: 0 }
}

/**
 * 给一段刚挂进文档的 Markdown DOM 做后处理。
 *
 * 高亮是同步的、便宜的，先做；公式和图表各跑各的——mermaid 是个上百万字节的大件，
 * 它加载失败或者渲染时抛了，不该顺手把公式排版也一起带走。
 */
export async function enhanceMarkdown(
  container: HTMLElement,
  session: EnhanceSession,
  options: EnhanceOptions,
): Promise<void> {
  const mine = session.generation
  bindChatImages(container)
  bindChatMediaPaths(container)
  highlightCode(container)

  const done = await Promise.allSettled([
    renderDiagrams(container, session, mine, options),
    renderMath(container, session, mine),
  ])
  for (const outcome of done) {
    if (outcome.status === 'rejected') console.error('[markdown] 增强失败', outcome.reason)
  }
}

/** 换主题后按新配色重画已经在页面上的图。 */
export async function redrawDiagrams(
  container: HTMLElement,
  session: EnhanceSession,
  theme: string,
): Promise<void> {
  const mine = session.generation
  const hosts = Array.from(container.querySelectorAll<HTMLElement>('.lya-diagram'))
  if (hosts.length === 0) return

  const { renderDiagram } = await import('./mermaid')
  if (session.generation !== mine) return

  for (const host of hosts) {
    const source = host.dataset['source']
    if (!source) continue
    const svg = await renderDiagram(source, theme)
    if (session.generation !== mine) return
    if (svg) mountDiagram(host, svg)
  }
}

/**
 * 把 mermaid 代码块换成图。
 *
 * # 说完了才画
 *
 * 流式期间一张都不画。这不只是省开销，更是因为**边写边画是错的**：半截的流程图往往
 * 是合法 mermaid（`graph TD` 加一条边就能解析），于是每来一个 delta 就画出一张不完整
 * 的图；而 `html` 是 computed，`v-html` 每次把整段 DOM 换掉，刚画好的那张又变回代码块
 * 再被画一遍。实测一条 612 字、三张图的回复，图表元素被插进页面 **189 次**（该是 3 次），
 * 主线程比同样长的纯文字多忙 842ms，其中 30 个 delta 单独就超过一帧——看上去就是三块
 * 东西在那儿疯狂闪。
 *
 * 所以流式期间源码就以代码块的样子逐行长出来，等 `streaming` 翻成 false 那一下再画一次。
 *
 * # 画不出来怎么办
 *
 * 原样留着当代码块，并把原因摆在下面：模型很爱写 `A[启动(初始化)]` 这种方括号里塞圆
 * 括号的写法，而它在 mermaid 里是语法错。不说的话，看的人只会以为图表功能坏了。
 */
export async function renderDiagrams(
  container: HTMLElement,
  session: EnhanceSession,
  mine: number,
  options: EnhanceOptions,
): Promise<void> {
  const blocks = Array.from(
    container.querySelectorAll<HTMLElement>('pre code.language-mermaid'),
  )
  if (blocks.length === 0) return

  if (options.streaming) {
    // 这期间不画，但可以先把那个上百万字节的大件拉下来：正文还在长，这几秒本来是闲的，
    // 不占就得等说完之后再等一次下载，代码块要在那里多停一会儿
    void import('./mermaid')
    return
  }

  let mermaid: typeof import('./mermaid')
  try {
    mermaid = await import('./mermaid')
  } catch (err) {
    // 整个组件都没加载起来，这一屏的图一张都画不出来，更该说清楚
    if (session.generation === mine) {
      for (const block of blocks) noteFailure(block, `图表组件加载失败：${String(err)}`)
    }
    return
  }
  if (session.generation !== mine) return

  for (const block of blocks) {
    const source = (block.textContent ?? '').trim()
    if (!source) continue
    const svg = await mermaid.renderDiagram(source, options.theme)
    if (session.generation !== mine) return

    if (!svg) {
      const why = await mermaid.explainDiagram(source, options.theme)
      if (session.generation !== mine) return
      noteFailure(block, why)
      continue
    }

    const shell = block.closest('.md-code') ?? block.closest('pre')
    if (!shell?.parentElement) continue
    const host = document.createElement('div')
    host.className = 'lya-diagram'
    host.dataset['source'] = source
    mountDiagram(host, svg)
    shell.replaceWith(host)
  }
  bindDiagramZoom(container)
}

/**
 * 在代码块下面挂一条「这张图为什么没变成图」。
 *
 * 源码留着不动：它是这段话的一部分，而且往往正是用户要拿去让模型改的东西。
 */
function noteFailure(block: HTMLElement, why: string): void {
  const shell = block.closest('.md-code') ?? block.closest('pre')
  if (!shell) return
  // 同一轮里可能被走到两次（加载失败那条路会遍历全部块），别叠罗汉
  if (shell.nextElementSibling?.classList.contains('lya-diagram-error')) return

  const note = document.createElement('div')
  note.className = 'lya-diagram-error'
  const head = document.createElement('strong')
  head.textContent = '这张图画不出来'
  const body = document.createElement('pre')
  // 原因里带着模型写的源码片段，只当文本塞，绝不走 innerHTML
  body.textContent = why
  note.append(head, body)
  shell.after(note)
}

/**
 * 把占位元素换成排好版的公式。
 *
 * 占位元素里那段文字就是模型写的 LaTeX，KaTeX 从字符串建 DOM，所以这一步没有
 * 任何模型生成的 HTML 进入页面，见 model/math.ts。
 */
async function renderMath(
  container: HTMLElement,
  session: EnhanceSession,
  mine: number,
): Promise<void> {
  const nodes = Array.from(
    container.querySelectorAll<HTMLElement>(`.${MATH_CLASS}`),
  ).filter((node) => node.dataset['done'] !== '1')
  if (nodes.length === 0) return

  const katex = (await import('./katex')).default
  if (session.generation !== mine) return

  for (const node of nodes) {
    const source = node.textContent ?? ''
    try {
      katex.render(source, node, {
        displayMode: node.dataset['display'] === '1',
        // 写错的公式画成红色原文就好，不该让它中断整段正文的渲染
        throwOnError: false,
        errorColor: 'var(--danger)',
        // \href、\htmlClass 这类能生成任意属性的命令一律不认
        trust: false,
        strict: false,
      })
      node.dataset['done'] = '1'
    } catch {
      // 极少数输入仍会抛，那就把原文留在那儿，比整段炸掉强
    }
  }
}

/** 给还没处理过的代码块加高亮、顶栏与行号。 */
function highlightCode(container: HTMLElement): void {
  for (const block of Array.from(container.querySelectorAll<HTMLElement>('pre code'))) {
    const pre = block.closest('pre')
    if (!pre) continue

    if (block.dataset['done'] !== '1') {
      hljs.highlightElement(block)
      block.dataset['done'] = '1'
    }

    ensureCodeBlock(pre, block)
    syncLineNumbers(pre, block)
  }
}

function ensureCodeBlock(pre: HTMLElement, code: HTMLElement): void {
  if (pre.closest('.md-code')) return

  const wrap = document.createElement('div')
  wrap.className = 'md-code'
  const parent = pre.parentElement!
  parent.insertBefore(wrap, pre)
  wrap.appendChild(headerBar(code))

  const body = document.createElement('div')
  body.className = 'md-code__body'
  wrap.appendChild(body)
  body.appendChild(pre)
  pre.classList.add('md-pre--barred', 'md-pre--lined')
}

function syncLineNumbers(pre: HTMLElement, code: HTMLElement): void {
  const body = pre.parentElement
  if (!body?.classList.contains('md-code__body')) return

  // markdown-it 的围栏代码块正文末尾必带一个 \n，直接 split 会多算一行
  const text = (code.textContent ?? '').replace(/\n$/, '')
  const lineCount = Math.max(1, text.split('\n').length)
  let gutter = body.querySelector('.md-code__lines') as HTMLElement | null
  if (!gutter) {
    gutter = document.createElement('div')
    gutter.className = 'md-code__lines'
    gutter.setAttribute('aria-hidden', 'true')
    body.insertBefore(gutter, pre)
  }
  gutter.textContent = Array.from({ length: lineCount }, (_, i) => String(i + 1)).join('\n')
}

/** 代码块顶栏：左边语言，右边复制。 */
function headerBar(block: HTMLElement): HTMLElement {
  const bar = document.createElement('div')
  bar.className = 'md-bar'

  const lang = document.createElement('span')
  lang.textContent =
    Array.from(block.classList)
      .find((name) => name.startsWith('language-'))
      ?.slice(9) ??
    block.dataset['highlighted'] ??
    'text'
  bar.appendChild(lang)

  const button = document.createElement('button')
  button.className = 'md-bar__copy'
  button.type = 'button'
  button.textContent = '复制'
  button.addEventListener('click', async () => {
    await navigator.clipboard.writeText(block.textContent ?? '')
    button.textContent = '已复制'
    setTimeout(() => (button.textContent = '复制'), 1200)
  })
  bar.appendChild(button)
  return bar
}
