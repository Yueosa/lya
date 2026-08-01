/**
 * Markdown 渲染。
 *
 * 三步：`marked` 解析 → `DOMPurify` 消毒 → 高亮由渲染后的 DOM 补。
 *
 * **消毒不是可选项**：正文是模型生成的，而模型读过的网页里可能藏着东西。
 * 一段 `<img onerror=...>` 就能在你自己的页面上执行脚本，而这个页面手里有
 * 图片令牌、能调所有本地接口。
 */

import DOMPurify from 'dompurify'
import { marked } from 'marked'

marked.use({ gfm: true, breaks: true })

// 关掉删除线：中文里 `~` 用得很随意，`~这样~` 会被误判成删除线
marked.use({
  tokenizer: {
    del() {
      return undefined
    },
  },
})

/** 本地图片改写所需的信息，来自 `/api/bootstrap`。 */
export interface ImageContext {
  /** 访问 `/api/local-image` 的令牌。 */
  token: string
  /** 家目录；只有落在它下面的路径才会被改写。 */
  home: string
}

/**
 * 把 Markdown 渲染成可以直接塞进 DOM 的 HTML。
 *
 * `images` 给了就把本地图片路径改写成后端接口。没给（比如还没拿到令牌）时
 * 本地图片会渲染成坏图——这比渲染成一个不带令牌、必然 403 的地址要诚实。
 */
export function renderMarkdown(text: string, images?: ImageContext): string {
  const raw = marked.parse(text, { async: false })
  const clean = DOMPurify.sanitize(raw, {
    ADD_ATTR: ['target'],
    // style 也要挡：一段 CSS 足以把整个界面盖住做成钓鱼页
    FORBID_TAGS: ['script', 'iframe', 'form', 'style', 'object', 'embed'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick'],
  })
  return images ? rewriteLocalImages(clean, images) : clean
}

/**
 * 把 `![](/home/你/图.png)` 换成能真正显示出来的地址。
 *
 * 只改家目录内的绝对路径与 `file://`。后端那边还会再校验一遍（解析符号链接后
 * 比对家目录），所以这里改错了也不会变成任意文件读取——这里只负责让能显示的
 * 显示出来。
 */
function rewriteLocalImages(html: string, images: ImageContext): string {
  return html.replace(/<img\b[^>]*\bsrc="([^"]+)"[^>]*>/g, (tag, src: string) => {
    const path = localPath(src, images.home)
    if (!path) return tag
    const query = new URLSearchParams({ path, token: images.token })
    return tag.replace(src, `/api/local-image?${query}`)
  })
}

/** 认出指向家目录内的本地路径；不是的话返回 `null`。 */
function localPath(src: string, home: string): string | null {
  let path = src.startsWith('file://') ? src.slice('file://'.length) : src

  // marked 输出的 src 已经是百分号编码的，这里必须先解回来。不解的话
  // URLSearchParams 会把 % 再编一遍（%E5 → %25E5），后端拿到的是另一个路径，
  // 于是所有中文文件名的图片都打不开
  try {
    path = decodeURIComponent(path)
  } catch {
    // 编码坏了就当它不是本地路径，交给浏览器原样处理
    return null
  }

  if (!path.startsWith('/')) return null
  // 只放行家目录内的。别处的图后端也会拒，提前挡掉省一次往返
  if (!path.startsWith(home.endsWith('/') ? home : `${home}/`)) return null
  return path
}
