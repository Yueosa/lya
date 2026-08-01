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
 * 当前这次渲染用的图片信息。
 *
 * marked 的渲染器是全局配置，而这个值每次调用可能不同，所以在解析前放进来。
 * 解析是同步的、单线程的，不会有两次渲染交叉。
 */
let currentImages: ImageContext | undefined

// 在**生成 HTML 的时候**就把本地路径换成接口地址，而不是回头去改字符串。
// 两个原因：DOMPurify 的协议白名单不含 file:，等它消毒完 src 已经被整个剥掉了；
// 而且拿正则去改一段 HTML 本来就脆。改在源头的话，消毒器看到的只是一个普通的
// 相对地址。
marked.use({
  renderer: {
    image({ href, title, text }) {
      const path = currentImages ? localPath(href, currentImages.home) : null
      const src =
        path && currentImages
          ? `/api/local-image?${new URLSearchParams({ path, token: currentImages.token })}`
          : href
      const attrs = [`src="${escapeAttr(src)}"`, `alt="${escapeAttr(text)}"`]
      if (title) attrs.push(`title="${escapeAttr(title)}"`)
      return `<img ${attrs.join(' ')}>`
    },
  },
})

function escapeAttr(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')
}

/**
 * 把 Markdown 渲染成可以直接塞进 DOM 的 HTML。
 *
 * `images` 给了就把本地图片路径改写成后端接口。没给（比如还没拿到令牌）时
 * 本地图片会渲染成坏图——这比渲染成一个不带令牌、必然 403 的地址要诚实。
 */
export function renderMarkdown(text: string, images?: ImageContext): string {
  currentImages = images
  try {
    const raw = marked.parse(text, { async: false })
    return DOMPurify.sanitize(raw, {
      ADD_ATTR: ['target'],
      // style 也要挡：一段 CSS 足以把整个界面盖住做成钓鱼页
      FORBID_TAGS: ['script', 'iframe', 'form', 'style', 'object', 'embed'],
      FORBID_ATTR: ['onerror', 'onload', 'onclick'],
    })
  } finally {
    currentImages = undefined
  }
}

/**
 * 认出指向家目录内的本地路径；不是的话返回 `null`。
 *
 * 后端那边还会再校验一遍（解析符号链接后比对家目录），所以这里判错也不会变成
 * 任意文件读取——这里只负责让能显示的显示出来。
 */
function localPath(src: string, home: string): string | null {
  let path = src.startsWith('file://') ? src.slice('file://'.length) : src

  // 走 renderer 之后 href 通常是原样的，但 Markdown 里手写编码过的路径也常见，
  // 解一次保证送给后端的是真实路径。不解的话 URLSearchParams 会把 % 再编一遍
  // （%E5 → %25E5），后端拿到的是另一个路径，中文文件名的图片就全打不开了
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
