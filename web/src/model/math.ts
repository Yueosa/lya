/**
 * 认出正文里的数学公式——只分词，不排版。
 *
 * 产出的是一个装着原始 LaTeX 的占位元素，真正的排版由 `MarkdownBody` 在**消毒
 * 之后**交给 KaTeX 做。
 *
 * 分两步是消毒边界逼出来的：正文是模型写的，而 KaTeX 的产物是一大片带
 * `<span>` 嵌套的 HTML。让它在消毒之前进管线，就等于要 DOMPurify 去分辨
 * 「这段 HTML 是 KaTeX 生成的还是模型自己塞进来的」——它分不了，放宽白名单
 * 又等于给注入开了门。占位元素只有一个 class 和一段纯文本，过完消毒再由 KaTeX
 * 从那段文本建 DOM，模型写的东西全程都只是字符串。
 */

import { type MarkedExtension, type Tokens } from 'marked'

/** 占位元素的 class，`MarkdownBody` 按它找待渲染的公式。 */
export const MATH_CLASS = 'lya-math'

interface MathToken extends Tokens.Generic {
  text: string
  display: boolean
}

/*
  行内 `$…$` 的规则必须严。中文聊天里 `$` 多数时候只是个美元号，而一旦
  「$5 到 $10」被当成公式，中间那段文字会**整段消失**在一个渲染失败的公式里——
  比不渲染糟得多。所以：开头的 `$` 后面不许跟空白，结尾的 `$` 前面不许有空白，
  中间不许换行，闭合的 `$` 后面也不许紧跟数字。
*/
const INLINE_DOLLAR = /^\$(?!\s)((?:\\.|[^\n$])+?)(?<!\s)\$(?!\d)/
const INLINE_PAREN = /^\\\(([\s\S]+?)\\\)/
/** 夹在段落中间的 `$$…$$` 也算展示公式，不必非得独占一段。 */
const INLINE_DOUBLE = /^\$\$([\s\S]+?)\$\$/
const BLOCK_DOLLAR = /^ {0,3}\$\$([\s\S]+?)\$\$[ \t]*(?:\n+|$)/
const BLOCK_BRACKET = /^ {0,3}\\\[([\s\S]+?)\\\][ \t]*(?:\n+|$)/

function escapeText(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

/**
 * 占位元素。
 *
 * 只用 `<span>` 加 `class` 加 `data-*`，这三样都在 DOMPurify 的默认白名单里，
 * 不用为了它放宽任何规则。展示公式靠 CSS 变成块级，不用 `<div>`——块级元素塞进
 * 段落里会被解析器拆出去，公式就跑到段落外面了。
 */
function placeholder(token: MathToken): string {
  const display = token.display ? '1' : '0'
  return `<span class="${MATH_CLASS}" data-display="${display}">${escapeText(token.text)}</span>`
}

function match(
  src: string,
  pattern: RegExp,
  type: string,
  display: boolean,
): MathToken | undefined {
  const found = pattern.exec(src)
  if (!found) return undefined
  return { type, raw: found[0], text: found[1] ?? '', display }
}

/**
 * 数学公式扩展。
 *
 * 认四种定界符：`$…$`、`$$…$$`、`\(…\)`、`\[…\]`。后两种要认是因为不少模型
 * 默认就吐反斜杠那种，只认 `$` 的话它们的公式全是原文。
 *
 * 代码块和行内代码不受影响：分词发生在 token 层，围栏与反引号先被吃掉，里面的
 * `$` 根本走不到这里。
 */
export const mathExtension: MarkedExtension = {
  extensions: [
    {
      name: 'mathBlock',
      level: 'block',
      start(src: string) {
        return /(?:^|\n) {0,3}(?:\$\$|\\\[)/.exec(src)?.index
      },
      tokenizer(src: string) {
        return (
          match(src, BLOCK_DOLLAR, 'mathBlock', true) ??
          match(src, BLOCK_BRACKET, 'mathBlock', true)
        )
      },
      renderer(token) {
        return placeholder(token as MathToken)
      },
    },
    {
      name: 'mathInline',
      level: 'inline',
      start(src: string) {
        const at = src.search(/\$|\\[([]/)
        return at === -1 ? undefined : at
      },
      // `$$` 要排在 `$` 前面试，否则双美元会被拆成两个空的行内公式
      tokenizer(src: string) {
        return (
          match(src, INLINE_DOUBLE, 'mathInline', true) ??
          match(src, INLINE_PAREN, 'mathInline', false) ??
          match(src, INLINE_DOLLAR, 'mathInline', false)
        )
      },
      renderer(token) {
        return placeholder(token as MathToken)
      },
    },
  ],
}
