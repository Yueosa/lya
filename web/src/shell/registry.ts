/**
 * 主题 → 外壳。
 *
 * 单独一张表而不是写进 `THEMES`，是为了让 `themes/` 只装 CSS 和元数据：那边被
 * 契约测试用 `node:fs` 读文件，混进 `.vue` 组件会把纯数据的部分也拖进构建。
 *
 * 表里同时登记**外壳组件**和**它在预览里的样子**。两件事放在一处是因为它们会一起变：
 * 加外壳的时候只有这一个地方要改，而漏掉预览那一半不会报错——只会让新主题在外观页里
 * 显示成一套它根本不用的排版，等到有人截图对比才发现。
 */

import type { Component } from 'vue'

import DefaultShell from './DefaultShell.vue'
import McShell from './McShell.vue'

/**
 * 外壳的导航形态，给主题预览画对应的示意。
 *
 * - `sidebar`：一列常驻侧栏，内容在右边
 * - `menu`：落地页是一屏大按钮菜单，选完才进内容
 */
export type ShellChrome = 'sidebar' | 'menu'

interface ShellEntry {
  component: Component
  chrome: ShellChrome
}

const DEFAULT_ENTRY: ShellEntry = { component: DefaultShell, chrome: 'sidebar' }

/** 需要另一种排版的主题登记在这里；没登记的用默认外壳。 */
const OVERRIDES: Record<string, ShellEntry> = {
  mc: { component: McShell, chrome: 'menu' },
}

/** 取某套主题的外壳。 */
export function shellFor(themeId: string): Component {
  return (OVERRIDES[themeId] ?? DEFAULT_ENTRY).component
}

/** 取某套主题外壳的导航形态。 */
export function chromeFor(themeId: string): ShellChrome {
  return (OVERRIDES[themeId] ?? DEFAULT_ENTRY).chrome
}
