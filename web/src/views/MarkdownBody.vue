<!--
  渲染一段 Markdown，并给代码块补上高亮与复制按钮。

  高亮放在渲染之后用 DOM 操作补，而不是在字符串阶段做：`highlight.js` 要吃真实
  元素，而且流式过程中正文每来一个增量就要重渲一次，只对新出现的代码块动手比
  整段重新高亮便宜得多。
-->

<script setup lang="ts">
import hljs from 'highlight.js'
import { computed, nextTick, ref, watch } from 'vue'

import { imageContext } from '../app/useChat'
import { renderMarkdown } from '../model/markdown'

const props = defineProps<{ text: string }>()

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
  // NodeList 在当前 lib 设定下不可迭代，转成数组
  for (const block of Array.from(container.querySelectorAll<HTMLElement>('pre code'))) {
    // 流式时同一个块会被反复看到，标记一下免得重复高亮
    if (block.dataset['done'] === '1') continue
    hljs.highlightElement(block)
    block.dataset['done'] = '1'

    const pre = block.closest('pre')
    if (!pre || pre.querySelector('.md-copy')) continue
    pre.appendChild(copyButton(block))
  }
}

function copyButton(block: HTMLElement): HTMLButtonElement {
  const button = document.createElement('button')
  button.className = 'btn btn--sm md-copy'
  button.type = 'button'
  button.textContent = '复制'
  button.addEventListener('click', async () => {
    await navigator.clipboard.writeText(block.textContent ?? '')
    button.textContent = '已复制'
    setTimeout(() => (button.textContent = '复制'), 1200)
  })
  return button
}
</script>

<template>
  <!-- 内容已经过 DOMPurify 消毒，见 model/markdown.ts -->
  <div ref="root" class="md" v-html="html" />
</template>

<style scoped>
.md {
  word-break: break-word;
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
  margin: 0.8em 0 0.4em;
  font-size: var(--text-lg);
  color: var(--accent);
}

.md :deep(p) {
  margin: 0.5em 0;
}

.md :deep(a) {
  color: var(--info);
}

.md :deep(code) {
  padding: 1px 5px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  font-family: var(--font-mono);
  font-size: 0.92em;
}

.md :deep(pre) {
  position: relative;
  margin: 0.6em 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  overflow-x: auto;
}

.md :deep(pre code) {
  padding: 0;
  background: none;
}

/* 平时藏起来，鼠标移到代码块上才出现，免得挡内容 */
.md :deep(.md-copy) {
  position: absolute;
  top: 6px;
  right: 6px;
  opacity: 0;
  transition: var(--transition);
}

.md :deep(pre:hover .md-copy) {
  opacity: 1;
}

.md :deep(blockquote) {
  margin: 0.6em 0;
  padding: 2px 12px;
  border-left: 3px solid var(--accent);
  background: var(--accent-soft);
  color: var(--text-muted);
}

.md :deep(ul),
.md :deep(ol) {
  margin: 0.5em 0;
  padding-left: 1.4em;
}

.md :deep(img) {
  max-width: 100%;
  border-radius: var(--radius-sm);
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
</style>
