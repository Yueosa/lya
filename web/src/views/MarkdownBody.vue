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
import { renderMarkdown } from '../model/markdown'
import { bindChatImages } from '../ui/useImageLightbox'

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

watch(
  html,
  async () => {
    await nextTick()
    if (root.value) enhance(root.value)
  },
  { immediate: true },
)

/** 给还没处理过的代码块加高亮和复制按钮。 */
function enhance(container: HTMLElement): void {
  bindChatImages(container)
  // NodeList 在当前 lib 设定下不可迭代，转成数组
  for (const block of Array.from(container.querySelectorAll<HTMLElement>('pre code'))) {
    // 流式时同一个块会被反复看到，标记一下免得重复高亮
    if (block.dataset['done'] === '1') continue
    hljs.highlightElement(block)
    block.dataset['done'] = '1'

    const pre = block.closest('pre')
    if (!pre || pre.previousElementSibling?.classList.contains('md-bar')) continue
    // 顶栏而不是浮在代码上的按钮——浮着的那个必然挡住第一行的字
    pre.parentElement?.insertBefore(headerBar(block), pre)
    pre.classList.add('md-pre--barred')
  }
}

/** 代码块顶栏：左边语言，右边复制。 */
function headerBar(block: HTMLElement): HTMLElement {
  const bar = document.createElement('div')
  bar.className = 'md-bar'

  const lang = document.createElement('span')
  // highlight.js 会把识别出的语言写进 class，取来当标签
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
  <div ref="root" class="md" :class="{ 'md--doc': variant === 'doc' }" v-html="html" />
</template>

<style scoped>
.md {
  min-width: 0;
  max-width: 100%;
  word-break: break-word;
  line-height: 1.55;
  /* 气泡不再 pre-wrap；HTML 里 marked 输出的换行符不能当可见空白 */
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

/* GFM 松散列表会在 li 里再包一层 p，默认 margin 会把行距撑爆 */
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
  max-width: 100%;
  margin: 0.6em 0;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  border: var(--border-width) solid var(--border);
  overflow-x: auto;
}

/* 顶栏：左边语言、右边复制。和下面的代码块拼成一整块 */
.md :deep(.md-bar) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 0.6em 0 0;
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

.md :deep(.md-pre--barred) {
  margin-top: 0;
  border-top-left-radius: 0;
  border-top-right-radius: 0;
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
  padding-left: 1.4em;
}

.md :deep(li) {
  margin: 0;
  line-height: 1.55;
}

.md :deep(img) {
  max-width: 100%;
  border-radius: var(--radius-sm);
}

.md :deep(video.lya-chat-video) {
  display: block;
  max-width: 100%;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
}

.md :deep(audio.lya-chat-audio) {
  display: block;
  width: 100%;
  max-width: 100%;
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

/* 工具/记忆等详情说明：对齐 prose，列表不额外缩进 */
.md--doc {
  font-size: var(--text-sm);
  line-height: var(--leading);
}

.md--doc :deep(p) {
  font-size: inherit;
}

.md--doc :deep(ul),
.md--doc :deep(ol) {
  margin: 0.28em 0;
  padding-left: 0;
  padding-inline-start: 0;
}

.md--doc :deep(ul) {
  list-style: disc;
  list-style-position: inside;
}

.md--doc :deep(ol) {
  list-style: none;
  counter-reset: lya-doc-ol;
}

.md--doc :deep(ol > li) {
  counter-increment: lya-doc-ol;
}

.md--doc :deep(ol > li::before) {
  content: counter(lya-doc-ol) ') ';
}

.md--doc :deep(li + li) {
  margin-top: 0.35em;
}
</style>
