/**
 * 聊天图片灯箱：放大、复制图片、复制路径/URL、保存。
 */

import { fetchMediaMeta, mediaOriginText, mediaRetainText, type MediaMeta } from './mediaMeta'
import { toast } from './useToast'

let overlay: HTMLDivElement | null = null

function close(): void {
  overlay?.remove()
  overlay = null
}

async function copyImageBlob(url: string): Promise<void> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${response.status}`)
  const blob = await response.blob()
  await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })])
}

async function saveBlob(url: string, filename: string): Promise<void> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${response.status}`)
  const blob = await response.blob()
  const objectUrl = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = objectUrl
  anchor.download = filename
  anchor.click()
  URL.revokeObjectURL(objectUrl)
}

function makeButton(label: string, onClick: () => void | Promise<void>): HTMLButtonElement {
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'lya-lightbox__btn'
  button.textContent = label
  button.addEventListener('click', (event) => {
    event.stopPropagation()
    void onClick()
  })
  return button
}

/** 打开灯箱。 */
export async function openImageLightbox(displayUrl: string, alt = ''): Promise<void> {
  close()

  let meta: MediaMeta | null = null
  try {
    meta = await fetchMediaMeta(displayUrl)
  } catch {
    // 元数据拿不到仍可以放大
  }

  overlay = document.createElement('div')
  overlay.className = 'lya-lightbox'
  overlay.setAttribute('role', 'dialog')
  overlay.setAttribute('aria-modal', 'true')

  const panel = document.createElement('div')
  panel.className = 'lya-lightbox__panel'

  const toolbar = document.createElement('div')
  toolbar.className = 'lya-lightbox__bar'

  const closeBtn = document.createElement('button')
  closeBtn.type = 'button'
  closeBtn.className = 'lya-lightbox__close'
  closeBtn.setAttribute('aria-label', '关闭')
  closeBtn.textContent = '×'
  closeBtn.addEventListener('click', (event) => {
    event.stopPropagation()
    close()
  })
  toolbar.appendChild(closeBtn)

  toolbar.appendChild(
    makeButton('复制图片', async () => {
      try {
        await copyImageBlob(displayUrl)
        toast('图片已复制', 'success')
      } catch {
        toast('复制图片失败', 'error')
      }
    }),
  )

  const originText = meta ? mediaOriginText(meta) : null
  if (originText) {
    toolbar.appendChild(
      makeButton(meta?.kind === 'web' ? '复制链接' : '复制路径', async () => {
        try {
          await navigator.clipboard.writeText(originText)
          toast('已复制', 'success')
        } catch {
          toast('复制失败', 'error')
        }
      }),
    )
  }

  // 远程图片留在本地的那一份，只有这里能复制到
  const retainText = meta ? mediaRetainText(meta) : null
  if (retainText) {
    toolbar.appendChild(
      makeButton('复制本地路径', async () => {
        try {
          await navigator.clipboard.writeText(retainText)
          toast('已复制', 'success')
        } catch {
          toast('复制失败', 'error')
        }
      }),
    )
  }

  toolbar.appendChild(
    makeButton('保存', async () => {
      try {
        await saveBlob(displayUrl, meta?.filename ?? 'image')
        toast('已开始下载', 'success')
      } catch {
        toast('保存失败', 'error')
      }
    }),
  )

  const img = document.createElement('img')
  img.className = 'lya-lightbox__img'
  img.src = displayUrl
  img.alt = alt

  panel.appendChild(toolbar)
  panel.appendChild(img)
  overlay.appendChild(panel)
  overlay.addEventListener('click', close)
  panel.addEventListener('click', (event) => event.stopPropagation())

  document.body.appendChild(overlay)
}

function onKeydown(event: KeyboardEvent): void {
  if (overlay && event.key === 'Escape') close()
}

/** 注册全局 Esc 关闭。在 App 启动时调一次即可。 */
export function setupImageLightbox(): () => void {
  document.addEventListener('keydown', onKeydown)
  return () => document.removeEventListener('keydown', onKeydown)
}

/** 给 Markdown 渲染出的聊天图片绑点击。 */
export function bindChatImages(container: HTMLElement): void {
  for (const img of Array.from(container.querySelectorAll<HTMLImageElement>('img.lya-chat-image'))) {
    if (img.dataset['bound'] === '1') continue
    img.dataset['bound'] = '1'
    img.style.cursor = 'zoom-in'
    img.addEventListener('click', (event) => {
      event.preventDefault()
      event.stopPropagation()
      const url = img.getAttribute('src')
      if (!url) return
      void openImageLightbox(url, img.alt)
    })
  }
}
