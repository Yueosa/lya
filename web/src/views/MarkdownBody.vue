<!--
  渲染一段 Markdown，并给代码块补上高亮与复制按钮。

  高亮放在渲染之后用 DOM 操作补，而不是在字符串阶段做：`highlight.js` 要吃真实
  元素，而且流式过程中正文每来一个增量就要重渲一次，只对新出现的代码块动手比
  整段重新高亮便宜得多。
-->

<script setup lang="ts">
import hljs from '../ui/hljs'
import { computed, nextTick, ref, watch } from 'vue'

import { imageContext } from '../app/useChat'
import { prefs } from '../app/usePrefs'
import { renderMarkdown } from '../model/markdown'
import { MATH_CLASS } from '../model/math'
import { themeId } from '../themes'
import { mountDiagram } from '../ui/diagramHost'
import { bindChatImages } from '../ui/useImageLightbox'
import { bindChatMediaPaths } from '../ui/useChatMedia'
import { bindDiagramZoom } from '../ui/useDiagramLightbox'

const props = withDefaults(
  defineProps<{
    text: string
    /** 详情页说明文案：列表左对齐、换行与原先 pre-wrap 一致 */
    variant?: 'default' | 'doc'
    /**
     * 显示 Markdown 原文。
     *
     * 由调用方决定而不是在这里读偏好：原文模式既有整屏的默认值，又能在单条消息
     * 上翻转，那笔账只有 `ChatTimeline` 算得清。这里只管照做。
     */
    raw?: boolean
    /**
     * 这段正文还会继续长。
     *
     * 只影响画不出来的图表报不报错：写到一半的图表解析不通过是必然的，那时候
     * 报错纯属噪音。说完了还画不出来才是真错。
     */
    streaming?: boolean
  }>(),
  { variant: 'default', raw: false, streaming: false },
)

const root = ref<HTMLElement | null>(null)
// 原文模式下连解析都不做：省一趟无用功，也让这条路上不存在任何 v-html
const html = computed(() =>
  props.raw ? '' : renderMarkdown(props.text, imageContext.value ?? undefined),
)
const codeWrap = computed(() => prefs.codeBlockWrap && props.variant === 'default')

/*
  正文每来一个增量就重渲一次，而公式和图表都要等动态 import 落地才画得出来，
  于是同一瞬间可能有好几轮在飞。带上轮次编号，过期的那几轮自己退场——否则一张
  刚渲染好的旧图会盖回已经更新过的正文上。
*/
let generation = 0

watch(
  html,
  async () => {
    if (props.raw) return
    const mine = ++generation
    await nextTick()
    if (root.value && generation === mine) await enhance(root.value, mine)
  },
  { immediate: true },
)

// 图表的配色是渲染时烤进 SVG 的，换主题不重画就会留着上一套颜色
watch(themeId, async () => {
  if (props.raw || !root.value) return
  const mine = ++generation
  await redrawDiagrams(root.value, mine)
})

/*
  说完之后再走一遍图表。

  收尾那一刻正文往往已经不变了，光盯着 html 是等不到这一下的——于是「写坏的图表
  要报错」永远差最后一步，页面上停在最后一次流式渲染的样子：一块没有下文的代码块。
*/
watch(
  () => props.streaming,
  async (now, before) => {
    if (props.raw || now || !before || !root.value) return
    const mine = ++generation
    await renderDiagrams(root.value, mine)
  },
)

async function enhance(container: HTMLElement, mine: number): Promise<void> {
  bindChatImages(container)
  bindChatMediaPaths(container)
  highlightCode(container)
  // 先高亮后换图：图表源码在渲染出来之前先以代码块的样子待着，流式输出时
  // 看到的是逐行长出来的源码，而不是一块空白
  //
  // 两件事各跑各的：mermaid 是个上百万字节的大件，它加载失败或者渲染时抛了，
  // 不该顺手把公式排版也一起带走——那会让「一张图画不出来」看起来像是
  // 「公式和图表两个功能一起坏了」，而后者根本无从查起
  const done = await Promise.allSettled([
    renderDiagrams(container, mine),
    renderMath(container, mine),
  ])
  for (const outcome of done) {
    if (outcome.status === 'rejected') console.error('[markdown] 增强失败', outcome.reason)
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

/**
 * 把 mermaid 代码块换成图。
 *
 * 渲染不成就原样留着当代码块——流式输出时源码必然有半截的一刻，那不是错误。
 * 但话说完了还画不出来就是真错，那时候把原因摆在代码块下面：模型很爱写
 * `A[启动(初始化)]` 这种方括号里塞圆括号的写法，而它在 mermaid 里是语法错。
 * 不说的话，看的人只会以为图表功能坏了。
 */
async function renderDiagrams(container: HTMLElement, mine: number): Promise<void> {
  const blocks = Array.from(
    container.querySelectorAll<HTMLElement>('pre code.language-mermaid'),
  )
  if (blocks.length === 0) return

  let mermaid: typeof import('../ui/mermaid')
  try {
    mermaid = await import('../ui/mermaid')
  } catch (err) {
    // 整个组件都没加载起来，这一屏的图一张都画不出来，更该说清楚
    if (!props.streaming && generation === mine) {
      for (const block of blocks) noteFailure(block, `图表组件加载失败：${String(err)}`)
    }
    return
  }
  if (generation !== mine) return

  for (const block of blocks) {
    const source = (block.textContent ?? '').trim()
    if (!source) continue
    const svg = await mermaid.renderDiagram(source, themeId.value)
    if (generation !== mine) return

    if (!svg) {
      if (props.streaming) continue
      const why = await mermaid.explainDiagram(source, themeId.value)
      if (generation !== mine) return
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

/** 换主题后按新配色重画已经在页面上的图。 */
async function redrawDiagrams(container: HTMLElement, mine: number): Promise<void> {
  const hosts = Array.from(container.querySelectorAll<HTMLElement>('.lya-diagram'))
  if (hosts.length === 0) return

  const { renderDiagram } = await import('../ui/mermaid')
  if (generation !== mine) return

  for (const host of hosts) {
    const source = host.dataset['source']
    if (!source) continue
    const svg = await renderDiagram(source, themeId.value)
    if (generation !== mine) return
    if (svg) mountDiagram(host, svg)
  }
}

/**
 * 把占位元素换成排好版的公式。
 *
 * 占位元素里那段文字就是模型写的 LaTeX，KaTeX 从字符串建 DOM，所以这一步没有
 * 任何模型生成的 HTML 进入页面，见 model/math.ts。
 */
async function renderMath(container: HTMLElement, mine: number): Promise<void> {
  const nodes = Array.from(
    container.querySelectorAll<HTMLElement>(`.${MATH_CLASS}`),
  ).filter((node) => node.dataset['done'] !== '1')
  if (nodes.length === 0) return

  const katex = (await import('../ui/katex')).default
  if (generation !== mine) return

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
</script>

<template>
  <!-- 原文走插值而不是 v-html：这条路上根本没有 HTML，也就没有消毒这回事 -->
  <pre v-if="raw" class="md-raw">{{ text }}</pre>

  <!-- 内容已经过 DOMPurify 消毒，见 model/markdown.ts -->
  <div
    v-else
    ref="root"
    class="md"
    :class="{ 'md--doc': variant === 'doc', 'md--code-wrap': codeWrap }"
    v-html="html"
  />
</template>

<style scoped>
/* 原文是拿来看和拷的，所以长行折行而不是横向滚动——气泡里没有横滚的余地 */
.md-raw {
  margin: 0;
  min-width: 0;
  max-width: 100%;
  padding: 10px 12px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 0.92em;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
  user-select: text;
}

.md {
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  word-break: break-word;
  overflow-wrap: anywhere;
  line-height: 1.55;
  white-space: normal;
}

.md :deep(> *:first-child) {
  margin-top: 0;
}

.md :deep(> *:last-child) {
  margin-bottom: 0;
}

.md :deep(h1),
.md :deep(h2),
.md :deep(h3) {
  margin: 0.65em 0 0.3em;
  font-weight: 600;
  line-height: 1.35;
}

.md :deep(h1) {
  font-size: var(--text-lg);
}

.md :deep(h2) {
  font-size: calc(var(--text-md) + 1px);
}

.md :deep(h3) {
  font-size: var(--text-md);
  color: var(--accent);
}

.md :deep(p) {
  margin: 0.28em 0;
  font-size: var(--text-md);
  line-height: 1.55;
}

.md :deep(li > p) {
  margin: 0;
}

.md :deep(li > p + p) {
  margin-top: 0.35em;
}

.md :deep(li + li) {
  margin-top: 0.12em;
}

.md :deep(a) {
  color: var(--info);
}

.md :deep(code) {
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--info) 12%, var(--bg-sunken));
  color: var(--info);
  font-family: var(--font-mono);
  font-size: 0.92em;
}

.md :deep(pre) {
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
  margin: 0.6em 0;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  border: var(--border-width) solid var(--border);
  overflow-x: auto;
}

.md :deep(.md-code) {
  min-width: 0;
  max-width: 100%;
  margin: 0.6em 0;
}

.md :deep(.md-bar) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
  padding: 4px 12px;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  background: var(--surface-active);
  color: var(--text-faint);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.md :deep(.md-bar__copy) {
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
  padding: 0 2px;
}

.md :deep(.md-bar__copy:hover) {
  color: var(--accent);
}

/*
  行号栏和代码必须共用同一个字号基准，所以字号定在它们的共同父节点上。

  否则：UA 样式给 `pre` 的 font-family 是裸的 `monospace`，会触发浏览器的
  「默认等宽字号」quirk 把 pre 压到 13px，而行号栏挂在 body 上是 16px 基准。
  两边各自算 0.92em 就是 11.96px vs 14.72px，行高一乘，行号越往下偏得越多。
*/
.md :deep(.md-code__body) {
  display: flex;
  min-width: 0;
  max-width: 100%;
  border: var(--border-width) solid var(--border);
  border-top: none;
  border-radius: 0 0 var(--radius-sm) var(--radius-sm);
  background: var(--bg-sunken);
  overflow: auto;
  font-family: var(--font-mono);
  font-size: 0.92em;
  line-height: 1.55;
}

.md :deep(.md-code__lines) {
  flex-shrink: 0;
  padding: 12px 8px 12px 10px;
  border-right: var(--border-width) solid var(--border);
  background: color-mix(in srgb, var(--surface-active) 80%, var(--bg-sunken));
  color: var(--text-faint);
  font: inherit;
  text-align: right;
  user-select: none;
  white-space: pre;
}

/* padding 一律由里面的 code 出，pre 自己不能再垫一层——
   否则代码文字比行号栏低一个 padding，肉眼就是行号对不上。
   `font: inherit` 用来盖掉 UA 给 pre 的等宽字号 quirk，见 .md-code__body */
.md :deep(.md-pre--barred),
.md :deep(.md-pre--lined) {
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 0;
  flex: 1;
  min-width: 0;
  font: inherit;
}

.md :deep(.md-code__body pre code) {
  display: block;
  padding: 12px 14px 12px 0;
  /* 跟着 .md-code__body 的基准走，别再自己算一遍 0.92em */
  font: inherit;
  white-space: pre;
}

.md--code-wrap :deep(.md-code__body) {
  overflow-x: hidden;
}

/* 换行后一个逻辑行占多个视觉行，行号栏没法再对齐，索引也就没意义了 */
.md--code-wrap :deep(.md-code__lines) {
  display: none;
}

.md--code-wrap :deep(.md-code__body pre code) {
  /* 行号栏没了，左边距得由 code 自己补上 */
  padding-left: 14px;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.md :deep(pre code) {
  padding: 0;
  background: none;
}

.md :deep(blockquote) {
  margin: 0.6em 0;
  padding: 2px 12px;
  border-left: var(--border-accent-width) solid var(--accent);
  background: var(--accent-soft);
  color: var(--text-muted);
}

.md :deep(ul),
.md :deep(ol) {
  margin: 0.28em 0;
}

.md :deep(ul) {
  padding-left: 1.4em;
}

/* Zpix 等 UI 字体不含可靠数字/项目符号，marker 用系统 sans */
.md :deep(ul > li::marker),
.md :deep(ol > li::before) {
  font-family: ui-sans-serif, system-ui, -apple-system, 'Segoe UI', 'Noto Sans SC', sans-serif;
  font-variant-numeric: tabular-nums;
  text-shadow: none;
}

.md :deep(ol) {
  list-style: none;
  counter-reset: lya-ol;
  padding-left: 0;
}

.md :deep(ol > li) {
  counter-increment: lya-ol;
  padding-left: 1.6em;
  position: relative;
}

.md :deep(ol > li::before) {
  content: counter(lya-ol) '. ';
  position: absolute;
  left: 0;
  width: 1.5em;
  text-align: right;
}

.md :deep(li) {
  margin: 0;
  line-height: 1.55;
}

.md :deep(img.lya-chat-image) {
  display: block;
  max-width: 100%;
  max-height: min(72vh, 640px);
  width: auto;
  height: auto;
  object-fit: contain;
  margin-inline: auto;
  border-radius: var(--radius-sm);
}

.md :deep(video.lya-chat-video) {
  display: block;
  max-width: 100%;
  max-height: min(72vh, 640px);
  /* 元数据没到之前先占一块 16/9 的地方，见 useChatMedia.trackVideoRatio */
  width: 100%;
  height: auto;
  aspect-ratio: var(--local-media-ratio, 16 / 9);
  object-fit: contain;
  margin-inline: auto;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
}

/* 知道真实尺寸了就按真实尺寸摆，小视频不该被拉满整栏 */
.md :deep(video.lya-chat-video[data-sized='1']) {
  width: auto;
}

.md :deep(audio.lya-chat-audio) {
  display: block;
  width: 100%;
  max-width: 100%;
}

.md :deep(.lya-chat-media-path) {
  margin: 4px 0 0.6em;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text-faint);
  word-break: break-all;
  line-height: 1.4;
  text-align: center;
}

/* 加载失败的媒体藏掉，位置留给下面那行提示 */
.md :deep(img[data-failed='1']),
.md :deep(video[data-failed='1']),
.md :deep(audio[data-failed='1']) {
  display: none;
}

.md :deep(.lya-chat-media-error) {
  margin: 4px 0 0.6em;
  padding: 10px 12px;
  border: var(--border-width) dashed var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  color: var(--text-muted);
  font-size: var(--text-xs);
  word-break: break-all;
  line-height: 1.5;
}

/* 展示公式独占一行。宽公式横向滚动，不然会把气泡撑破 */
.md :deep(.lya-math[data-display='1']) {
  display: block;
  margin: 0.6em 0;
  text-align: center;
  overflow-x: auto;
  overflow-y: hidden;
}

/* 还没渲染的公式（KaTeX 正在下载）先按等宽显示原文，比空白诚实 */
.md :deep(.lya-math:not([data-done='1'])) {
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.md :deep(.lya-diagram) {
  margin: 0.6em 0;
  padding: 10px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  overflow-x: auto;
}

/* 紧贴在源码下面：它解释的就是上面那块东西 */
.md :deep(.lya-diagram-error) {
  margin: -0.4em 0 0.6em;
  padding: 8px 12px;
  border: var(--border-width) solid var(--danger);
  border-top: none;
  border-radius: 0 0 var(--radius-sm) var(--radius-sm);
  background: var(--danger-soft);
  color: var(--danger);
  font-size: var(--text-xs);
}

/* mermaid 的报错自带一行 ^ 指着出错的列，等宽才对得齐 */
.md :deep(.lya-diagram-error pre) {
  margin: 4px 0 0;
  padding: 0;
  border: none;
  background: none;
  color: inherit;
  font-family: var(--font-mono);
  font-size: inherit;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-x: auto;
}

.md :deep(table) {
  border-collapse: collapse;
  margin: 0.6em 0;
}

.md :deep(th),
.md :deep(td) {
  padding: 4px 10px;
  border: var(--border-width) solid var(--border);
}

.md :deep(th) {
  background: var(--surface-hover);
}

.md--doc {
  font-size: var(--text-sm);
  line-height: var(--leading);
}

.md--doc :deep(p) {
  font-size: inherit;
}

.md--doc :deep(ul),
.md--doc :deep(ol) {
  padding-left: 0;
  padding-inline-start: 0;
}

.md--doc :deep(ul) {
  list-style: disc;
  list-style-position: inside;
}

.md--doc :deep(ol > li) {
  padding-left: 0;
  position: static;
}

.md--doc :deep(ol > li::before) {
  content: counter(lya-ol) ') ';
  position: static;
  width: auto;
  text-align: left;
}

.md--doc :deep(li + li) {
  margin-top: 0.35em;
}
</style>
