/**
 * 主题注册与切换。
 *
 * 切换只改 `<html data-theme>`，不换 markup。三套风格共用一份 DOM——
 * 每套一份 HTML 意味着每加一个功能要写三遍，分支树、HITL 表单这种复杂交互
 * 几乎必然写歪其中两套。
 *
 * # 逃生舱
 *
 * 九成差异靠 token。剩下一成——Minecraft 可能要 `border-image` 贴角图、要
 * `image-rendering: pixelated`，这类属性在别的主题里根本不存在，硬塞进 token
 * 很别扭——由主题自己的样式表针对语义类名补规则。所以组件的类名要稳定、要
 * 有语义，它们是主题的公开接口。
 */

import { ref } from 'vue'

import './mtf.css'
import './mc.css'
import './base.css'

import { readLocal, writeLocal } from '../utils/storage'
import { ensureMcFont } from './mcFont'

/** 一套主题。 */
export interface Theme {
  id: string
  /** 显示给用户看的名字。 */
  label: string
  /** 深色还是浅色，用于设置 `color-scheme`，让原生滚动条与表单控件跟着变。 */
  scheme: 'dark' | 'light'
}

export const THEMES: Theme[] = [
  { id: 'mtf', label: 'MTF 简约', scheme: 'light' },
  { id: 'mc', label: 'Minecraft', scheme: 'light' },
]

const STORAGE_KEY = 'lya.theme'
const DEFAULT_THEME = 'mtf'

/** 当前主题 id；认不出的一律回退到默认，免得整个界面没有颜色。 */
export function currentTheme(): string {
  const saved = readLocal(STORAGE_KEY)
  return THEMES.some((theme) => theme.id === saved) ? saved! : DEFAULT_THEME
}

/**
 * 响应式的当前主题。
 *
 * 设置页换主题时外壳（侧栏 / Minecraft 菜单）也要跟着换——只改 `data-theme` 不够，
 * `App.vue` 还得靠这个选外壳组件。
 */
export const themeId = ref(currentTheme())

/** 换主题并记住选择。 */
export function applyTheme(id: string): void {
  const theme = THEMES.find((candidate) => candidate.id === id) ?? THEMES[0]!
  document.documentElement.dataset['theme'] = theme.id
  // 让原生滚动条、下拉、日期选择器跟着深浅走，否则浅色主题里会冒出深色控件
  document.documentElement.style.colorScheme = theme.scheme
  writeLocal(STORAGE_KEY, theme.id)
  themeId.value = theme.id
  if (theme.id === 'mc') void ensureMcFont()
}

/** 启动时调用一次。 */
export function initTheme(): void {
  applyTheme(currentTheme())
}
