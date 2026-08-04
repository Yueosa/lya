/**
 * 显示偏好。
 *
 * 一条硬约束：**这些只影响渲染，不影响数据**。状态里始终保留全部块，隐藏是在
 * 画的时候跳过——否则关掉「显示思考」再打开，之前那些思考内容已经没了，得重新
 * 拉一遍整棵树。
 *
 * # 两组，两个作用域
 *
 * 「跟随流式输出」「代码块自动换行」问的是**我这台机器上想怎么看**，换个会话没道理
 * 重置。「隐藏思考」「隐藏工具调用」这些问的是**这个会话想怎么看**：一个满屏 bash
 * 输出的会话想把工具块折起来，另一个纯聊天的会话没这个需求，共用一份只会互相打架。
 *
 * 两组都在 localStorage（换设备不同步，这些不是会话的属性，不该占后端字段），
 * 会话级那份按会话 id 分键；会话删掉时一起清。
 */

import { reactive, ref, watch } from 'vue'

/** 跟着这台机器走的偏好。 */
export interface MachinePrefs {
  followStream: boolean
  /** 代码块内长行自动换行（默认横向滚动 + 行号）。 */
  codeBlockWrap: boolean
}

/** 跟着会话走的偏好。 */
export interface SessionPrefs {
  hideReasoning: boolean
  hideTools: boolean
  hideResolvedHitl: boolean
  /** 隐藏系统通知（含模式变更）。 */
  hideNotices: boolean
  /** 思考块流式结束后自动收起；工具块默认收起，不受此项影响。 */
  autoCollapseAside: boolean
}

/** 一组显示偏好。 */
export type Prefs = MachinePrefs & SessionPrefs

const MACHINE_KEY = 'lya.prefs'

function sessionKey(id: string): string {
  return `lya.prefs.${id}`
}

const MACHINE_DEFAULTS: MachinePrefs = {
  followStream: true,
  codeBlockWrap: false,
}

const SESSION_DEFAULTS: SessionPrefs = {
  hideReasoning: false,
  hideTools: false,
  hideResolvedHitl: false,
  hideNotices: false,
  autoCollapseAside: true,
}

/** 供设置界面分组用。 */
export const MACHINE_PREF_KEYS = Object.keys(MACHINE_DEFAULTS) as (keyof MachinePrefs)[]
export const SESSION_PREF_KEYS = Object.keys(SESSION_DEFAULTS) as (keyof SessionPrefs)[]

function read<T extends object>(key: string, defaults: T): T {
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? '{}') as Partial<T>
    // 逐字段合并而不是整体替换：以后加了新偏好，老用户存的那份不会缺字段
    return { ...defaults, ...saved }
  } catch {
    return { ...defaults }
  }
}

function write(key: string, value: object): void {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // 隐私模式或配额满了写不进去就算了，内存里的值仍然有效
  }
}

function pick<T extends object, K extends keyof T>(source: T, keys: K[]): Pick<T, K> {
  const out = {} as Pick<T, K>
  for (const key of keys) out[key] = source[key]
  return out
}

export const prefs = reactive<Prefs>({
  ...read(MACHINE_KEY, MACHINE_DEFAULTS),
  ...SESSION_DEFAULTS,
})

/** 会话级偏好当前存到哪个会话名下；为空时（首页等）不落盘。 */
const boundSession = ref<string | null>(null)

watch(
  prefs,
  (value) => {
    write(MACHINE_KEY, pick(value, MACHINE_PREF_KEYS))
    const id = boundSession.value
    if (id) write(sessionKey(id), pick(value, SESSION_PREF_KEYS))
  },
  { deep: true },
)

/**
 * 切换会话级偏好的归属。
 *
 * 打开会话时调用。传 `null`（回首页、会话被删）时恢复默认值，这样下一个会话不会
 * 莫名继承上一个会话的隐藏设置。
 */
export function bindSessionPrefs(id: string | null): void {
  if (boundSession.value === id) return
  // 先换归属再改值，否则 watch 回调会把新会话的设置写到旧会话名下
  boundSession.value = id
  Object.assign(prefs, id ? read(sessionKey(id), SESSION_DEFAULTS) : SESSION_DEFAULTS)
}

/** 会话删了，它那份显示偏好也没有留下的意义。 */
export function forgetSessionPrefs(id: string): void {
  try {
    localStorage.removeItem(sessionKey(id))
  } catch {
    // 同上，读不到写不进都不影响使用
  }
}

/** 恢复默认。会话级那部分只影响当前会话。 */
export function resetPrefs(): void {
  Object.assign(prefs, MACHINE_DEFAULTS, SESSION_DEFAULTS)
}
