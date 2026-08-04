/**
 * 聊天媒体的路径条：在图片 / 视频 / 音频下方展示它在磁盘上的位置（只读，不剪贴板）。
 *
 * 图片以前没有这条，只能点开灯箱才知道文件在哪——但「这是哪个文件」在扫一眼消息
 * 时就该看得见，不该藏在一次点击后面。灯箱仍然只有图片有，那是放大和复制用的。
 */

import { fetchMediaMeta, isSessionMediaSrc, mediaPathText } from './mediaMeta'

const PATH_BAR_CLASS = 'lya-chat-media-path'

/** 在元素后面插入或更新一条路径条。 */
function upsertPathBar(el: Element, text: string): void {
  const existing = el.nextElementSibling
  if (existing?.classList.contains(PATH_BAR_CLASS)) {
    existing.textContent = text
    existing.setAttribute('title', text)
    return
  }

  const bar = document.createElement('div')
  bar.className = PATH_BAR_CLASS
  bar.textContent = text
  bar.title = text
  el.insertAdjacentElement('afterend', bar)
}

async function attachPathLabel(el: Element, displayUrl: string): Promise<void> {
  try {
    const text = mediaPathText(await fetchMediaMeta(displayUrl))
    if (text) upsertPathBar(el, text)
  } catch {
    // 元数据拿不到就不显示路径条，媒体本身照常播放
  }
}

/** 给 Markdown 渲染出的聊天媒体绑路径条。 */
export function bindChatMediaPaths(container: HTMLElement): void {
  const selector = 'img.lya-chat-image, video.lya-chat-video, audio.lya-chat-audio'
  for (const el of Array.from(container.querySelectorAll<HTMLElement>(selector))) {
    if (el.dataset['pathBound'] === '1') continue
    const src = el.getAttribute('src')
    if (!src || !isSessionMediaSrc(src)) continue
    el.dataset['pathBound'] = '1'
    void attachPathLabel(el, src)
  }
}
