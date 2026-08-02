/**
 * 主题 → 外壳。
 *
 * 单独一张表而不是写进 `THEMES`，是为了让 `themes/` 只装 CSS 和元数据：那边被
 * 契约测试用 `node:fs` 读文件，混进 `.vue` 组件会把纯数据的部分也拖进构建。
 */

import type { Component } from 'vue'

import DefaultShell from './DefaultShell.vue'
import McShell from './McShell.vue'
import TokyoShell from './TokyoShell.vue'

/** 需要另一种排版的主题登记在这里；没登记的用默认外壳。 */
const OVERRIDES: Record<string, Component> = {
  mc: McShell,
  'tokyo-night': TokyoShell,
}

/** 取某套主题的外壳。 */
export function shellFor(themeId: string): Component {
  return OVERRIDES[themeId] ?? DefaultShell
}
