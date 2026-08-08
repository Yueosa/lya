/**
 * 灯箱外壳：遮罩、工具栏、关闭。
 *
 * 装什么由调用方给——图片给 `<img>`，图表给一个能缩放拖动的容器。抽出来是因为
 * 「同时开着两个灯箱」这种事必须从结构上排除掉：全局只有一个 `overlay`，开新的
 * 先关旧的，Esc 也只有一处在听。两套各写一遍的话，迟早会出现一个盖住另一个、
 * 而 Esc 只关得掉其中一个的局面。
 */

let overlay: HTMLDivElement | null = null

/** 工具栏上的一个按钮。 */
export interface LightboxAction {
  label: string
  onSelect: () => void | Promise<void>
}

export interface LightboxOptions {
  /** 工具栏按钮，从左到右排在关闭按钮后面。 */
  actions: LightboxAction[]
  /** 主体内容。 */
  body: HTMLElement
  /** 额外挂在 panel 上的 class，用来给不同内容调尺寸。 */
  panelClass?: string
  /** 关掉时收尾，比如摘掉自己加的监听。 */
  onClose?: () => void
}

let onCloseHook: (() => void) | undefined

/** 关掉当前灯箱；没开着就什么都不做。 */
export function closeLightbox(): void {
  overlay?.remove()
  overlay = null
  const hook = onCloseHook
  onCloseHook = undefined
  hook?.()
}

function actionButton(action: LightboxAction): HTMLButtonElement {
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'lya-lightbox__btn'
  button.textContent = action.label
  button.addEventListener('click', (event) => {
    // 不拦的话会冒泡到遮罩，点一下按钮灯箱就没了
    event.stopPropagation()
    void action.onSelect()
  })
  return button
}

/** 打开灯箱。 */
export function openLightbox(options: LightboxOptions): void {
  closeLightbox()
  onCloseHook = options.onClose

  overlay = document.createElement('div')
  overlay.className = 'lya-lightbox'
  overlay.setAttribute('role', 'dialog')
  overlay.setAttribute('aria-modal', 'true')

  const panel = document.createElement('div')
  panel.className = 'lya-lightbox__panel'
  if (options.panelClass) panel.classList.add(options.panelClass)

  const toolbar = document.createElement('div')
  toolbar.className = 'lya-lightbox__bar'

  const closeBtn = document.createElement('button')
  closeBtn.type = 'button'
  closeBtn.className = 'lya-lightbox__close'
  closeBtn.setAttribute('aria-label', '关闭')
  closeBtn.textContent = '×'
  closeBtn.addEventListener('click', (event) => {
    event.stopPropagation()
    closeLightbox()
  })
  toolbar.appendChild(closeBtn)

  for (const action of options.actions) toolbar.appendChild(actionButton(action))

  panel.appendChild(toolbar)
  panel.appendChild(options.body)
  overlay.appendChild(panel)
  overlay.addEventListener('click', closeLightbox)
  panel.addEventListener('click', (event) => event.stopPropagation())

  document.body.appendChild(overlay)
}

function onKeydown(event: KeyboardEvent): void {
  if (overlay && event.key === 'Escape') closeLightbox()
}

/** 注册全局 Esc 关闭。在 App 启动时调一次即可。 */
export function setupLightbox(): () => void {
  document.addEventListener('keydown', onKeydown)
  return () => document.removeEventListener('keydown', onKeydown)
}
