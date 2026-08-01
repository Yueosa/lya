/**
 * 右键菜单。
 *
 * 和弹窗一样是全局单例：菜单从任意元素的 `contextmenu` 事件调起，同时只可能
 * 开一个。上一代有现成的菜单组件，侧边栏却把同样的 markup 又抄了一遍——
 * 两份实现从此各自演化。这里只留这一个入口。
 */

import { reactive, readonly } from 'vue'

import { placeNear, type Size } from './placement'

/** 菜单里的一项。 */
export interface MenuItem {
  /** 显示文案。 */
  label: string
  /** 前置图标，可以是 emoji。 */
  icon?: string
  /** 破坏性操作，显示为危险色。 */
  danger?: boolean
  /** 灰掉不可点。 */
  disabled?: boolean
  /** 选中时执行。 */
  onSelect: () => void
}

/** 分隔线。 */
export interface MenuSeparator {
  separator: true
}

/** 菜单条目。 */
export type MenuEntry = MenuItem | MenuSeparator

/** 是不是分隔线。 */
export function isSeparator(entry: MenuEntry): entry is MenuSeparator {
  return 'separator' in entry
}

/**
 * 模板用的行。
 *
 * 直接把 [`MenuEntry`] 交给模板的话，Vue 的模板编译器无法通过类型守卫函数收窄
 * 联合类型，访问 `entry.label` 会报错。所以这里换成带判别字段的形状——判别式
 * 比较（`row.kind === 'item'`）它是认得的。
 */
export type MenuRow =
  | { kind: 'separator'; key: number }
  | { kind: 'item'; key: number; item: MenuItem }

/** 把条目转成模板能直接消费的行。 */
export function toRows(entries: readonly MenuEntry[]): MenuRow[] {
  return entries.map((entry, key) =>
    isSeparator(entry) ? { kind: 'separator' as const, key } : { kind: 'item' as const, key, item: entry },
  )
}

interface MenuState {
  open: boolean
  left: number
  top: number
  entries: MenuEntry[]
}

const state = reactive<MenuState>({ open: false, left: 0, top: 0, entries: [] })

/** 给宿主组件读的只读状态。 */
export const menuState = readonly(state)

/**
 * 在鼠标位置打开菜单。
 *
 * 会 `preventDefault`，所以调用方不用自己拦浏览器默认菜单——忘了拦的话两个
 * 菜单会叠在一起。
 */
export function openContextMenu(event: MouseEvent, entries: MenuEntry[]): void {
  event.preventDefault()
  event.stopPropagation()
  state.entries = entries
  // 先按鼠标位置摆上，等宿主组件量出真实尺寸再校正——菜单没渲染出来之前
  // 量不到宽高，而等一帧再显示会闪
  state.left = event.clientX
  state.top = event.clientY
  state.open = true
}

/** 量到真实尺寸后校正位置，避免菜单跑出视口。 */
export function reposition(size: Size, viewport: Size): void {
  const placed = placeNear({ left: state.left, top: state.top }, size, viewport)
  state.left = placed.left
  state.top = placed.top
}

/** 关掉菜单。 */
export function closeContextMenu(): void {
  state.open = false
  state.entries = []
}

/** 点某一项。 */
export function selectItem(item: MenuItem): void {
  if (item.disabled) return
  // 先关再执行：动作可能弹确认框，菜单还开着会压在上面
  closeContextMenu()
  item.onSelect()
}
