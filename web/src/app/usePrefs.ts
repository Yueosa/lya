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
  /** 不显示模型的思考过程。有些模型思考很长，看正文时是噪音。 */
  hideReasoning: boolean
  /** 不显示工具调用卡片。 */
  hideTools: boolean
  /** 不显示历史里已经答复过的 HITL。 */
  hideResolvedHitl: boolean
  /** 流式输出时自动滚到底。 */
  followStream: boolean
}

const KEY = 'lya.prefs'

const DEFAULTS: Prefs = {
  hideReasoning: false,
  hideTools: false,
  hideResolvedHitl: false,
  followStream: true,
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
