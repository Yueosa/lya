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

/** 本地/会话图片改写所需的信息，来自 `/api/bootstrap` 与当前会话。 */
export interface ImageContext {
  /** 访问图片端点的令牌。 */
  token: string
  /** 家目录；只有落在它下面的路径才会被改写成 local。 */
  home: string
  /** 有则走 `/api/sessions/{id}/media/image` 缓存端点。 */
  sessionId?: string
}

/**
 * 当前这次渲染用的图片信息。
 *
 * marked 的渲染器是全局配置，而这个值每次调用可能不同，所以在解析前放进来。
 * 解析是同步的、单线程的，不会有两次渲染交叉。
 */
let currentImages: ImageContext | undefined

function mediaParams(kind: 'local' | 'web', src: string, ctx: ImageContext): string {
  const base = ctx.sessionId
    ? `/api/sessions/${encodeURIComponent(ctx.sessionId)}/media/image`
    : kind === 'local'
      ? '/api/local-image'
      : ''
  if (!base) return src
  const params = new URLSearchParams({ token: ctx.token })
  if (ctx.sessionId) {
    params.set('kind', kind)
    params.set('src', src)
  } else {
    params.set('path', src)
  }
  return `${base}?${params}`
}

function isRemoteUrl(href: string): boolean {
  const trimmed = href.trim()
  return trimmed.startsWith('http://') || trimmed.startsWith('https://')
}

// 在**生成 HTML 的时候**就把路径换成接口地址，而不是回头去改字符串。
marked.use({
  renderer: {
    image({ href, title, text }) {
      let src = href
      if (currentImages) {
        const local = localPath(href, currentImages.home)
        if (local) {
          src = mediaParams('local', local, currentImages)
        } else if (isRemoteUrl(href) && currentImages.sessionId) {
          src = mediaParams('web', href.trim(), currentImages)
        }
      }
      const attrs = [
        `src="${escapeAttr(src)}"`,
        `alt="${escapeAttr(text)}"`,
        `class="lya-chat-image"`,
      ]
      if (title) attrs.push(`title="${escapeAttr(title)}"`)
      return `<img ${attrs.join(' ')}>`
    },
  },
})

function escapeAttr(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')
}

/** 列表项之间不要空行——否则 GFM 会产出 `<li><p>…</p></li>` 松散列表。 */
function tightenListMarkdown(text: string): string {
  return text.replace(
    /^([ \t]*(?:[-*+]|\d+\.)[ \t].*)\n\n+(?=[ \t]*(?:[-*+]|\d+\.)[ \t])/gm,
    '$1\n',
  )
}

/** 松散列表再包一层 p 没必要，unwrap 后间距只由 li 控制。 */
function unwrapLooseListItems(html: string): string {
  return html.replace(/<li>\s*<p>([\s\S]*?)<\/p>\s*<\/li>/gi, '<li>$1</li>')
}

/** 去掉空段落，避免气泡里出现大块空白。 */
function compactHtml(html: string): string {
  return html
    .replace(/<p>(?:\s|&nbsp;|<br\s*\/?>)*<\/p>/gi, '')
    .replace(/>\s+</g, '><')
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
    const raw = marked.parse(tightenListMarkdown(text), { async: false })
    return compactHtml(
      unwrapLooseListItems(
        DOMPurify.sanitize(raw, {
          ADD_ATTR: ['target'],
          // style 也要挡：一段 CSS 足以把整个界面盖住做成钓鱼页
          FORBID_TAGS: ['script', 'iframe', 'form', 'style', 'object', 'embed'],
          FORBID_ATTR: ['onerror', 'onload', 'onclick'],
        }),
      ),
    )
  } finally {
    currentImages = undefined
  }
}

/**
 * 把家目录内的绝对路径转成图片 API 地址；否则 `null`。
 */
export function localImageUrl(src: string, ctx?: ImageContext | null): string | null {
  if (!ctx) return null
  const path = localPath(src, ctx.home)
  if (!path) return null
  return mediaParams('local', path, ctx)
}

/**
 * 认出指向家目录内的本地路径；不是的话返回 `null`。
 *
 * 后端那边还会再校验一遍（解析符号链接后比对家目录），所以这里判错也不会变成
 * 任意文件读取——这里只负责让能显示的显示出来。
 */
function localPath(src: string, home: string): string | null {
  let path = src.trim()
  // Markdown 尖括号路径：< /home/... >
  if (path.startsWith('<') && path.endsWith('>')) {
    path = path.slice(1, -1).trim()
  }

  if (path.startsWith('file://')) {
    path = path.slice('file://'.length)
    // file:///home/... 留三斜杠，file://home 少见，统一去掉 scheme 后 trim
    if (path.startsWith('//')) path = path.slice(2)
  }

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
