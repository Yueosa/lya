/**
 * 三种行内记号：删除线 `~~这样~~`、下标 `H~2~O`、上标 `E=mc^2^`。
 *
 * # 为什么波浪号这么麻烦
 *
 * 中文里 `~` 是个语气符号（「好耶~」「等一下~~」），用得很随意。GFM 的删除线**单个波浪号
 * 也认**，于是「这样~那样~就好了」会被当成删除线，正文里莫名其妙缺一块——比不渲染糟得多,
 * 因为读的人不会想到是渲染问题。
 *
 * 先前的处理是把删除线整个关掉。那治住了单波浪号，但连 `~~这样~~` 也一起没了——而双波浪号
 * 是没有歧义的，没人会用两个波浪号表示语气。所以现在按数量分开：
 *
 * - `~~x~~` → 删除线。两个波浪号，认。
 * - `~x~` → 下标，**但只在 `x` 是「像下标的东西」时**。
 * - 其余的 `~` → 原样是字。
 *
 * # 「像下标的东西」是什么
 *
 * 真实用途里下标只有数字、拉丁字母和几个算式符号：`H~2~O`、`CO~2~`、`x~i~`、`a~n+1~`。而
 * 中文语气号包的是中文。所以下标的内容限定成不含空白的短 ASCII 串——`~那样~` 里是中文，
 * 不match，仍然是字。这条界线不完美（`~ok~` 会变成下标），但它把常见的两类分开了，而且
 * 错的那一侧是「多渲染了一个下标」，不是「正文缺一块」。
 *
 * 要写中文下标或复杂公式，用 `$…$`——那才是它该去的地方，见 `model/math.ts`。
 *
 * 上标 `^x^` 同一套规则。`^` 在中文里几乎不出现，风险比波浪号小得多，但保持一致更好记。
 */

import { type MarkedExtension, type Tokens } from 'marked'

/**
 * 下标／上标的内容。
 *
 * 不含空白、不含中文、最长 12 个字符。放宽任何一条都会开始咬到中文语气号；`+-=(),.` 是为了
 * `a~n+1~`、`x~(i)~` 这类还算常见的写法。
 */
const CONTENT = '[0-9A-Za-z+\\-=(),.]{1,12}'

/** `~x~`；开头不许再跟一个 `~`，那是删除线的活。 */
const SUB = new RegExp(`^~(?!~)(${CONTENT})~(?!~)`)

/** `^x^`。 */
const SUP = new RegExp(`^\\^(?!\\^)(${CONTENT})\\^`)

/**
 * `~~x~~`。
 *
 * 懒匹配加上必须以非空白收尾，所以 `~~H~2~O~~` 会整段吃掉（内部那对波浪号在子分词里再变成
 * 下标），而不是在第一个 `~` 处断开。
 */
const DEL = /^~~(?=\S)([\s\S]*?\S)~~/

interface MarkToken extends Tokens.Generic {
  text: string
}

function mark(src: string, pattern: RegExp, type: string): MarkToken | undefined {
  const found = pattern.exec(src)
  if (!found) return undefined
  return { type, raw: found[0], text: found[1] ?? '' }
}

/**
 * 下标与上标。
 *
 * 内容只可能是 ASCII 字母数字和几个符号，所以不必再走一遍行内分词——里面不会有需要解析的
 * 东西，直接当文本渲染。
 */
export const inlineMarksExtension: MarkedExtension = {
  extensions: [
    {
      name: 'subscript',
      level: 'inline',
      start(src: string) {
        const at = src.indexOf('~')
        return at === -1 ? undefined : at
      },
      tokenizer(src: string) {
        return mark(src, SUB, 'subscript')
      },
      renderer(token) {
        return `<sub>${(token as MarkToken).text}</sub>`
      },
    },
    {
      name: 'superscript',
      level: 'inline',
      start(src: string) {
        const at = src.indexOf('^')
        return at === -1 ? undefined : at
      },
      tokenizer(src: string) {
        return mark(src, SUP, 'superscript')
      },
      renderer(token) {
        return `<sup>${(token as MarkToken).text}</sup>`
      },
    },
  ],
  /**
   * 只认双波浪号的删除线，盖掉 GFM 那个连单波浪号也认的版本。
   *
   * 覆盖 tokenizer 而不是加扩展：`del` 是 marked 自带的类型，加一个同名扩展只会打架，而这里
   * 要的正是「同样的 token，更严的入口」。
   */
  tokenizer: {
    del(src: string): Tokens.Del | undefined {
      const found = DEL.exec(src)
      if (!found) return undefined
      const text = found[1] ?? ''
      return {
        type: 'del',
        raw: found[0],
        text,
        // 删除线里面还能有别的记号（粗体、行内代码、下标），照常再分一次词
        tokens: this.lexer.inlineTokens(text),
      }
    },
  },
}
