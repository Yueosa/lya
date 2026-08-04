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
import { bindChatImages } from '../ui/useImageLightbox'
import { bindChatMediaPaths } from '../ui/useChatMedia'

const props = withDefaults(
  defineProps<{
    text: string
    /** 详情页说明文案：列表左对齐、换行与原先 pre-wrap 一致 */
    variant?: 'default' | 'doc'
  }>(),
  { variant: 'default' },
)

const root = ref<HTMLElement | null>(null)
const html = computed(() => renderMarkdown(props.text, imageContext.value ?? undefined))
const codeWrap = computed(() => prefs.codeBlockWrap && props.variant === 'default')

watch(
  html,
  async () => {
    await nextTick()
    if (root.value) enhance(root.value)
  },
  { immediate: true },
)

/** 给还没处理过的代码块加高亮、顶栏与行号。 */
function enhance(container: HTMLElement): void {
  bindChatImages(container)
  bindChatMediaPaths(container)
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
</script>

<template>
  <!-- 内容已经过 DOMPurify 消毒，见 model/markdown.ts -->
  <div
    ref="root"
    class="md"
    :class="{ 'md--doc': variant === 'doc', 'md--code-wrap': codeWrap }"
    v-html="html"
  />
</template>

<style scoped>
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
  width: auto;
  height: auto;
  object-fit: contain;
  margin-inline: auto;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
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
