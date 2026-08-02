/**
 * 聊天 video/audio：播放器下方展示路径或 URL（只读，不剪贴板）。
 */

interface MediaMeta {
  kind: 'local' | 'web'
  filename: string
  copy_path: string | null
  copy_url: string | null
  display_url: string
}

async function fetchMediaMeta(displayUrl: string): Promise<MediaMeta> {
  const url = `${displayUrl}${displayUrl.includes('?') ? '&' : '?'}meta=1`
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${response.status}`)
  return response.json() as Promise<MediaMeta>
}

function isSessionMediaSrc(src: string): boolean {
  return src.includes('/api/sessions/') && src.includes('/media/')
}

async function attachPathLabel(el: HTMLMediaElement, displayUrl: string): Promise<void> {
  const meta = await fetchMediaMeta(displayUrl)
  const text = meta.copy_path ?? meta.copy_url
  if (!text) return

  const existing = el.nextElementSibling
  if (existing?.classList.contains('lya-chat-media-path')) {
    existing.textContent = text
    existing.setAttribute('title', text)
    return
  }

  const bar = document.createElement('div')
  bar.className = 'lya-chat-media-path'
  bar.textContent = text
  bar.title = text
  el.insertAdjacentElement('afterend', bar)
}

/** 给 Markdown 渲染出的 video/audio 绑路径条。 */
export function bindChatMediaPaths(container: HTMLElement): void {
  for (const el of Array.from(
    container.querySelectorAll<HTMLMediaElement>('video.lya-chat-video, audio.lya-chat-audio'),
  )) {
    if (el.dataset['pathBound'] === '1') continue
    const src = el.getAttribute('src')
    if (!src || !isSessionMediaSrc(src)) continue
    el.dataset['pathBound'] = '1'
    void attachPathLabel(el, src)
  }
}
