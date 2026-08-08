/**
 * 确认框与输入框。
 *
 * # 为什么是命令式单例
 *
 * 这些东西会从任意地方被调起——事件处理函数里、组合式函数里、异步回调里。做成
 * 需要挂在模板上的组件，就得把一个 ref 一路透传下去，或者每个用到的地方各挂
 * 一份。所以是「全局一份状态 + 挂一次宿主组件 + 命令式调用」。
 *
 * # 一条硬规矩
 *
 * **不允许出现原生 `confirm` / `alert` / `prompt`。** 上一代有了统一弹窗之后，
 * 两个设置页仍然在用 `window.confirm`，侧边栏还把右键菜单的 markup 抄了一遍。
 * 原生弹窗不受主题控制，长得和界面完全两样。`no-native-dialogs.test.ts` 会检查。
 */

import { reactive, readonly } from 'vue'

import { errorText } from '../api/client'

/** 确认框参数。 */
export interface ConfirmOptions {
  title: string
  message?: string
  confirmText?: string
  cancelText?: string
  /** 破坏性操作，确认按钮显示为危险色。 */
  danger?: boolean
}

/** 输入框参数。 */
export interface PromptOptions extends ConfirmOptions {
  /** 初始值。 */
  initial?: string
  placeholder?: string
}

/** 带异步执行的确认框参数。 */
export interface ConfirmAsyncOptions extends ConfirmOptions {
  /**
   * 用户点确认后要跑的事情。
   *
   * 跑的过程中弹窗**不关闭**，按钮转圈；抛错就把错误显示在弹窗里，让用户能
   * 直接重试。关掉再弹一个报错的话，用户已经失去上下文了。
   */
  run: () => Promise<void>
}

interface DialogState {
  open: boolean
  kind: 'confirm' | 'prompt'
  title: string
  message: string
  confirmText: string
  cancelText: string
  danger: boolean
  placeholder: string
  value: string
  /** 异步执行中，按钮转圈且不可再点。 */
  busy: boolean
  /** 异步执行失败时的提示。 */
  error: string
}

const state = reactive<DialogState>({
  open: false,
  kind: 'confirm',
  title: '',
  message: '',
  confirmText: '确认',
  cancelText: '取消',
  danger: false,
  placeholder: '',
  value: '',
  busy: false,
  error: '',
})

/** 当前这次调用的兑现函数。 */
let settle: ((value: unknown) => void) | null = null
/** 点确认后要跑的异步任务。 */
let pending: (() => Promise<void>) | null = null

/** 给宿主组件读的只读状态。 */
export const dialogState = readonly(state)

function reset(options: ConfirmOptions): void {
  state.title = options.title
  state.message = options.message ?? ''
  state.confirmText = options.confirmText ?? '确认'
  state.cancelText = options.cancelText ?? '取消'
  state.danger = options.danger ?? false
  state.busy = false
  state.error = ''
  state.open = true
}

/** 问一个是非题。 */
export function confirm(options: ConfirmOptions): Promise<boolean> {
  // 前一个还开着就当它被取消了，否则那个 Promise 永远悬着
  close(false)
  return new Promise<boolean>((resolve) => {
    settle = resolve as (value: unknown) => void
    pending = null
    state.kind = 'confirm'
    state.value = ''
    reset(options)
  })
}

/** 要一段文本；取消返回 `null`。 */
export function prompt(options: PromptOptions): Promise<string | null> {
  close(false)
  return new Promise<string | null>((resolve) => {
    settle = resolve as (value: unknown) => void
    pending = null
    state.kind = 'prompt'
    state.value = options.initial ?? ''
    state.placeholder = options.placeholder ?? ''
    reset(options)
  })
}

/**
 * 确认后当场执行，成功才关闭。
 *
 * 返回是否真的做完了。用在删除、清空这类「点了之后还要等一会儿」的操作上。
 */
export function confirmAsync(options: ConfirmAsyncOptions): Promise<boolean> {
  close(false)
  return new Promise<boolean>((resolve) => {
    settle = resolve as (value: unknown) => void
    pending = options.run
    state.kind = 'confirm'
    state.value = ''
    reset(options)
  })
}

/** 用户按了确认。 */
export async function accept(): Promise<void> {
  if (state.busy) return

  if (pending) {
    state.busy = true
    state.error = ''
    try {
      await pending()
    } catch (error) {
      // 留在弹窗里报错，用户不用重新走一遍才能重试
      state.busy = false
      state.error = errorText(error)
      return
    }
    state.busy = false
  }
  close(state.kind === 'prompt' ? state.value : true)
}

/** 用户取消，或按了 Escape。 */
export function cancel(): void {
  // 异步跑到一半不给关：底下的操作已经在做了，关掉只会让人以为没发生
  if (state.busy) return
  close(state.kind === 'prompt' ? null : false)
}

function close(result: unknown): void {
  if (!state.open && !settle) return
  state.open = false
  pending = null
  const resolve = settle
  settle = null
  resolve?.(result)
}

/** 输入框的双向绑定入口。 */
export function setValue(value: string): void {
  state.value = value
}
