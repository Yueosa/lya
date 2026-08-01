/**
 * 轻提示。
 *
 * 用于「保存成功」这类不需要用户回应的反馈。需要回应的用 [`confirm`]，
 * 需要用户读完的错误也别用它——几秒就消失的东西不适合承载「为什么失败」。
 */

import { reactive, readonly } from 'vue'

/** 提示的性质。 */
export type ToastKind = 'success' | 'error' | 'info'

/** 一条提示。 */
export interface Toast {
  id: number
  message: string
  kind: ToastKind
}

/** 停留多久。错误留久一点，读起来更从容。 */
const DURATION: Record<ToastKind, number> = {
  success: 2500,
  info: 2500,
  error: 6000,
}

const state = reactive<{ items: Toast[] }>({ items: [] })
let nextId = 1

/** 给宿主组件读的只读状态。 */
export const toastState = readonly(state)

/** 弹一条提示，返回它的 id。 */
export function toast(message: string, kind: ToastKind = 'info'): number {
  const id = nextId++
  state.items.push({ id, message, kind })
  setTimeout(() => dismissToast(id), DURATION[kind])
  return id
}

/** 手动关掉一条。 */
export function dismissToast(id: number): void {
  const index = state.items.findIndex((item) => item.id === id)
  if (index >= 0) state.items.splice(index, 1)
}

/** 清空，切换会话之类的场景用。 */
export function clearToasts(): void {
  state.items.splice(0)
}
