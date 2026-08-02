import type { Directive, DirectiveBinding } from 'vue'

interface TipState {
  text: string
  show: boolean
  showTimer: number | null
  hideTimer: number | null
}

let tipNode: HTMLDivElement | null = null
const state: TipState = { text: '', show: false, showTimer: null, hideTimer: null }

const SHOW_DELAY = 320
const HIDE_DELAY = 80
const GAP = 8

function ensureNode(): HTMLDivElement {
  if (tipNode) return tipNode
  const el = document.createElement('div')
  el.className = 'lya-tip'
  el.style.position = 'fixed'
  el.style.zIndex = '9999'
  el.style.pointerEvents = 'none'
  el.style.opacity = '0'
  el.style.transition = 'opacity 0.12s ease'
  document.body.appendChild(el)
  tipNode = el
  return el
}

function position(target: HTMLElement): void {
  const node = ensureNode()
  const rect = target.getBoundingClientRect()
  const tipRect = node.getBoundingClientRect()
  const vw = window.innerWidth
  const vh = window.innerHeight

  let top = rect.bottom + GAP
  if (top + tipRect.height > vh - 4) {
    top = rect.top - tipRect.height - GAP
  }
  let left = rect.left + rect.width / 2 - tipRect.width / 2
  left = Math.max(4, Math.min(left, vw - tipRect.width - 4))
  node.style.top = `${top}px`
  node.style.left = `${left}px`
}

function showTooltip(target: HTMLElement, text: string): void {
  if (!text) return
  state.text = text
  if (state.hideTimer) {
    clearTimeout(state.hideTimer)
    state.hideTimer = null
  }
  if (state.showTimer) clearTimeout(state.showTimer)
  state.showTimer = window.setTimeout(() => {
    state.showTimer = null
    const node = ensureNode()
    node.textContent = text
    node.style.opacity = '0'
    node.style.display = 'block'
    requestAnimationFrame(() => {
      position(target)
      node.style.opacity = '1'
    })
    state.show = true
  }, SHOW_DELAY)
}

function hideTooltip(): void {
  if (state.showTimer) {
    clearTimeout(state.showTimer)
    state.showTimer = null
  }
  if (!state.show) return
  state.hideTimer = window.setTimeout(() => {
    state.hideTimer = null
    if (tipNode) {
      tipNode.style.opacity = '0'
      tipNode.style.display = 'none'
    }
    state.show = false
  }, HIDE_DELAY)
}

function getText(binding: DirectiveBinding): string {
  const v = binding.value
  if (v == null) return ''
  return String(v)
}

function onEnter(this: HTMLElement): void {
  const text = (this as HTMLElement & { __lyaTip?: string }).__lyaTip ?? ''
  if (text) showTooltip(this, text)
}

function onLeave(): void {
  hideTooltip()
}

function onDismiss(): void {
  if (state.showTimer) {
    clearTimeout(state.showTimer)
    state.showTimer = null
  }
  if (tipNode) {
    tipNode.style.opacity = '0'
    tipNode.style.display = 'none'
  }
  state.show = false
}

window.addEventListener('scroll', onLeave, true)
window.addEventListener('resize', onLeave)
window.addEventListener('mousedown', onDismiss, true)

export const vTip: Directive<HTMLElement, string | number | null | undefined> = {
  mounted(el, binding) {
    const text = getText(binding)
    ;(el as HTMLElement & { __lyaTip?: string }).__lyaTip = text
    if (text) el.setAttribute('aria-label', text)
    el.addEventListener('mouseenter', onEnter)
    el.addEventListener('mouseleave', onLeave)
    el.addEventListener('click', onDismiss)
    el.addEventListener('blur', onLeave, true)
  },
  updated(el, binding) {
    const text = getText(binding)
    ;(el as HTMLElement & { __lyaTip?: string }).__lyaTip = text
    if (text) el.setAttribute('aria-label', text)
    else el.removeAttribute('aria-label')
  },
  unmounted(el) {
    el.removeEventListener('mouseenter', onEnter)
    el.removeEventListener('mouseleave', onLeave)
    el.removeEventListener('click', onDismiss)
    el.removeEventListener('blur', onLeave, true)
  },
}
