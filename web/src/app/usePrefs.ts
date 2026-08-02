/**
 * 显示偏好。
 *
 * 一条硬约束：**这些只影响渲染，不影响数据**。状态里始终保留全部块，隐藏是在
 * 画的时候跳过——否则关掉「显示思考」再打开，之前那些思考内容已经没了，得重新
 * 拉一遍整棵树。
 *
 * 存在 localStorage，换设备不同步；这些是「我这台机器上想怎么看」，不是会话的
 * 属性，不该占后端的字段。
 */

import { reactive, watch } from 'vue'

/** 一组显示偏好。 */
export interface Prefs {
  hideReasoning: boolean
  hideTools: boolean
  hideResolvedHitl: boolean
  /** 隐藏系统通知（含模式变更）。 */
  hideNotices: boolean
  followStream: boolean
  /** 流式结束后自动收起思考/工具块。 */
  autoCollapseAside: boolean
  /** 超过此行数的侧栏块默认折叠（流式中仍展开）。 */
  asideFoldLineThreshold: number
}

const KEY = 'lya.prefs'

const DEFAULTS: Prefs = {
  hideReasoning: false,
  hideTools: false,
  hideResolvedHitl: false,
  hideNotices: false,
  followStream: true,
  autoCollapseAside: true,
  asideFoldLineThreshold: 16,
}

function load(): Prefs {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<Prefs>
    // 逐字段合并而不是整体替换：以后加了新偏好，老用户存的那份不会缺字段
    return { ...DEFAULTS, ...saved }
  } catch {
    return { ...DEFAULTS }
  }
}

export const prefs = reactive<Prefs>(load())

watch(
  prefs,
  (value) => {
    localStorage.setItem(KEY, JSON.stringify(value))
  },
  { deep: true },
)

/** 恢复默认。 */
export function resetPrefs(): void {
  Object.assign(prefs, DEFAULTS)
}
