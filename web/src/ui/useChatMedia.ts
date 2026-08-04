/**
 * 聊天媒体的路径条与加载失败占位。
 *
 * 图片以前没有路径条，只能点开灯箱才知道文件在哪——但「这是哪个文件」在扫一眼消息
 * 时就该看得见，不该藏在一次点击后面。灯箱仍然只有图片有，那是放大和复制用的；
 * 视频音频仍然走浏览器自带的播放器，不给它们造灯箱。
 */

import {
  fetchMediaMeta,
  isSessionMediaSrc,
  mediaOriginText,
  mediaRetainText,
  type MediaMeta,
} from './mediaMeta'

const PATH_BAR_CLASS = 'lya-chat-media-path'
const ERROR_CLASS = 'lya-chat-media-error'

function retainLabel(meta: MediaMeta): string {
  return meta.retained_kind === 'hardlink' ? '副本（与原文件共用空间）' : '本地副本'
}

/** 找媒体后面挂着的那块附加信息；路径条和失败提示各一块，顺序不定。 */
function siblingWithClass(el: Element, className: string): Element | null {
  let cursor = el.nextElementSibling
  for (let step = 0; cursor && step < 2; step += 1) {
    if (cursor.classList.contains(className)) return cursor
    cursor = cursor.nextElementSibling
  }
  return null
}

/** 在元素后面插入或更新路径条。 */
function upsertPathBar(el: Element, meta: MediaMeta): void {
  const origin = mediaOriginText(meta)
  if (!origin) return
  const retain = mediaRetainText(meta)

  const lines = [origin]
  if (retain) lines.push(`${retainLabel(meta)}：${retain}`)

  const existing = siblingWithClass(el, PATH_BAR_CLASS)
  const bar = existing ?? document.createElement('div')
  bar.className = PATH_BAR_CLASS
  bar.textContent = ''
  for (const line of lines) {
    const row = document.createElement('div')
    row.className = 'lya-chat-media-path__line'
    row.textContent = line
    bar.appendChild(row)
  }
  // 完整信息塞进 tooltip：硬链接副本不占单独一行，但想查的时候查得到
  bar.setAttribute('title', [origin, meta.retained_path ?? ''].filter(Boolean).join('\n'))
  if (!existing) el.insertAdjacentElement('afterend', bar)
}

async function attachPathLabel(el: Element, displayUrl: string): Promise<void> {
  try {
    upsertPathBar(el, await fetchMediaMeta(displayUrl))
  } catch {
    // 元数据拿不到就不显示路径条，媒体本身照常播放
  }
}

/**
 * 媒体加载失败时给一句人话。
 *
 * DOMPurify 会摘掉 `onerror` 属性，所以只能在这里绑；不绑的话失败表现就是浏览器原生的
 * 破图或空播放器，一个字的解释都没有，也看不出是远程挂了还是我们的端点出了问题。
 */
function upsertErrorNote(el: HTMLElement, origin: string | null): void {
  const existing = siblingWithClass(el, ERROR_CLASS)
  const note = existing ?? document.createElement('div')
  note.className = ERROR_CLASS
  note.textContent = origin ? `媒体加载失败：${origin}` : '媒体加载失败'
  note.setAttribute('title', origin ?? '')
  if (!existing) el.insertAdjacentElement('afterend', note)
}

function bindError(el: HTMLElement, src: string): void {
  el.addEventListener(
    'error',
    () => {
      // 破图图标和空播放器都不如一句话，藏掉换成提示
      el.dataset['failed'] = '1'
      const query = src.split('?')[1] ?? ''
      upsertErrorNote(el, new URLSearchParams(query).get('src'))
    },
    { once: true },
  )
}

/**
 * 视频在拿到元数据之前是浏览器默认的 300×150，之后才跳到真实尺寸——一条消息里几个
 * 视频就能把下面的内容顶走一大截。先按 16/9 占位（CSS 里的默认值），元数据到了再换成
 * 真实比例，把这一跳压到最小。剩下的高度变化交给滚动容器的 ResizeObserver 收尾。
 */
function trackVideoRatio(video: HTMLVideoElement): void {
  const apply = (): void => {
    if (!video.videoWidth || !video.videoHeight) return
    video.style.setProperty('--local-media-ratio', `${video.videoWidth} / ${video.videoHeight}`)
    video.dataset['sized'] = '1'
  }
  if (video.readyState >= HTMLMediaElement.HAVE_METADATA) {
    apply()
    return
  }
  video.addEventListener('loadedmetadata', apply, { once: true })
}

/** 给 Markdown 渲染出的聊天媒体绑路径条、尺寸占位与失败提示。 */
export function bindChatMediaPaths(container: HTMLElement): void {
  const selector = 'img.lya-chat-image, video.lya-chat-video, audio.lya-chat-audio'
  for (const el of Array.from(container.querySelectorAll<HTMLElement>(selector))) {
    if (el instanceof HTMLVideoElement && el.dataset['sized'] !== '1') trackVideoRatio(el)
    if (el.dataset['pathBound'] === '1') continue
    const src = el.getAttribute('src')
    if (!src) continue
    el.dataset['pathBound'] = '1'
    bindError(el, src)
    if (isSessionMediaSrc(src)) void attachPathLabel(el, src)
  }
}
