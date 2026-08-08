<!--
  渲染一段 Markdown，并给代码块补上高亮与复制按钮。

  高亮放在渲染之后用 DOM 操作补，而不是在字符串阶段做：`highlight.js` 要吃真实
  元素，而且流式过程中正文每来一个增量就要重渲一次，只对新出现的代码块动手比
  整段重新高亮便宜得多。
-->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import { imageContext } from '../app/useChat'
import { prefs } from '../app/usePrefs'
import { renderMarkdown } from '../model/markdown'
import { themeId } from '../themes'
import {
  createEnhanceSession,
  enhanceMarkdown,
  redrawDiagrams,
  renderDiagrams,
} from '../ui/markdownEnhance'

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
     * 图表在这期间**一律不画**，只以代码块的样子待着，见 ui/markdownEnhance。
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
  刚渲染好的旧图会盖回已经更新过的正文上。逻辑本体在 ui/markdownEnhance.ts。
*/
const session = createEnhanceSession()

watch(
  html,
  async () => {
    if (props.raw) return
    const mine = ++session.generation
    await nextTick()
    if (root.value && session.generation === mine) {
      await enhanceMarkdown(root.value, session, {
        streaming: props.streaming,
        theme: themeId.value,
      })
    }
  },
  { immediate: true },
)

// 图表的配色是渲染时烤进 SVG 的，换主题不重画就会留着上一套颜色
watch(themeId, async () => {
  if (props.raw || !root.value) return
  session.generation += 1
  await redrawDiagrams(root.value, session, themeId.value)
})

/*
  说完了，这才是图表被画出来的那一下。

  流式期间 renderDiagrams 一概不动手（理由见 ui/markdownEnhance），所以这个 watcher
  不是补漏而是正路。而且非它不可：收尾那一刻正文往往已经不再变了，光盯着 html
  是等不到这一下的。

  先 nextTick 再动手：收尾常常伴着一次正文替换（运行缓冲换成落库的那条消息），那会
  重新走一遍 v-html。不等它落地就画，画完立刻被整段 DOM 覆盖掉，白画一次。
*/
watch(
  () => props.streaming,
  async (now, before) => {
    if (props.raw || now || !before || !root.value) return
    const mine = ++session.generation
    await nextTick()
    if (root.value && session.generation === mine) {
      await renderDiagrams(root.value, session, mine, {
        streaming: false,
        theme: themeId.value,
      })
    }
  },
)
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
