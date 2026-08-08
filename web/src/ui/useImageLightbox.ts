/**
 * 聊天图片灯箱：放大、复制图片、复制路径/URL、保存。
 *
 * 遮罩与工具栏那层外壳在 `lightbox.ts`，这里只管图片特有的那几个动作。
 */

import { openLightbox, type LightboxAction } from './lightbox'
import { fetchMediaMeta, mediaOriginText, mediaRetainText, type MediaMeta } from './mediaMeta'
import { toast } from './useToast'

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

function copyAction(label: string, text: string): LightboxAction {
  return {
    label,
    onSelect: async () => {
      try {
        await navigator.clipboard.writeText(text)
        toast('已复制', 'success')
      } catch {
        toast('复制失败', 'error')
      }
    },
  }
}

/** 打开灯箱。 */
export async function openImageLightbox(displayUrl: string, alt = ''): Promise<void> {
  let meta: MediaMeta | null = null
  try {
    meta = await fetchMediaMeta(displayUrl)
  } catch {
    // 元数据拿不到仍可以放大
  }

  const actions: LightboxAction[] = [
    {
      label: '复制图片',
      onSelect: async () => {
        try {
          await copyImageBlob(displayUrl)
          toast('图片已复制', 'success')
        } catch {
          toast('复制图片失败', 'error')
        }
      },
    },
  ]

  const originText = meta ? mediaOriginText(meta) : null
  if (originText) {
    actions.push(copyAction(meta?.kind === 'web' ? '复制链接' : '复制路径', originText))
  }

  // 远程图片留在本地的那一份，只有这里能复制到
  const retainText = meta ? mediaRetainText(meta) : null
  if (retainText) {
    actions.push(copyAction('复制本地路径', retainText))
  }

  actions.push({
    label: '保存',
    onSelect: async () => {
      try {
        await saveBlob(displayUrl, meta?.filename ?? 'image')
        toast('已开始下载', 'success')
      } catch {
        toast('保存失败', 'error')
      }
    },
  })

  const img = document.createElement('img')
  img.className = 'lya-lightbox__img'
  img.src = displayUrl
  img.alt = alt

  openLightbox({ actions, body: img })
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
