/**
 * 动效开关：MTF / 东京夜启用；MC token 时长为 0；系统「减少动效」时关闭。
 */

import { computed } from 'vue'

import { themeId } from '../themes'

const MOTION_THEMES = new Set(['mtf', 'tokyo-night'])

function prefersReducedMotion(): boolean {
  try {
    return globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
  } catch {
    return false
  }
}

/** 当前主题是否播放过渡动效。 */
export function useMotion() {
  const motionEnabled = computed(
    () => MOTION_THEMES.has(themeId.value) && !prefersReducedMotion(),
  )
  return { motionEnabled }
}

/** 会话消息条目的错峰 delay（仅进入时由 ChatView 写入 style）。 */
export function messageStaggerDelay(messageIndex: number, cap = 28, stepMs = 42): string {
  return `${Math.min(Math.max(messageIndex, 0), cap) * stepMs}ms`
}

/** 进入会话时错峰 + 滑入动画的最长等待（ms）。 */
export function sessionEnterMotionMs(cap = 28, stepMs = 42, animationMs = 320): number {
  return cap * stepMs + animationMs
}
