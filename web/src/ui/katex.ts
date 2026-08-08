/**
 * KaTeX 懒加载入口。
 *
 * 库和它那套字体样式一起关在这个模块里，只有正文真的出现公式时才会被动态
 * import 进来——绝大多数对话里一个公式都没有，没道理让所有人先下一份数学字体。
 */

import katex from 'katex'
import 'katex/dist/katex.min.css'

export default katex
