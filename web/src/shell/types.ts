/**
 * 外壳：每套主题可以有自己的排版。
 *
 * # 边界在哪
 *
 * 之前我说过「换主题不该换 markup」，那句话对了一半——**对聊天视图是对的，
 * 推广到整个应用就错了**。要把消息树、HITL 表单、分支切换器写三遍，那是自找
 * 麻烦：那里的复杂度占了整个界面的九成，三份实现必然有两份是残的。
 *
 * 但「怎么走到一个会话」是另一回事。侧边栏也好、Minecraft 那种标题画面配一列
 * 大按钮也好，都只是导航，各写一份也就百来行，而风格差异恰恰体现在这里。
 *
 * 所以边界是：
 *
 * - **外壳可换**——布局、导航、落地页
 * - **视图不可换**——聊天、记忆、设置的内容区，只有一份实现
 *
 * 外壳拿到当前视图和跳转函数，把内容放进插槽。它不知道内容是什么，也就没机会
 * 把聊天逻辑抄进自己那份。
 */

import type { NavIconKey } from './icons'

/** 应用里的几个去处。 */
export type View =
  | 'home'
  | 'chat'
  | 'sessions'
  | 'settings'
  | 'memory'
  | 'tools'
  | 'models'
  | 'theme'
  | 'prompt'
  | 'config'
  | 'storage'

/** 外壳组件的入参。 */
export interface ShellProps {
  /** 当前在哪。 */
  view: View
}

/** 外壳组件抛出的事件。 */
export interface ShellEmits {
  /** 用户要去别处。 */
  (event: 'navigate', view: View): void
}

/**
 * 侧栏 / 主菜单导航。
 *
 * 会话列表已经嵌在默认外壳侧栏里，会话设置挂在聊天页头部——这两项不再占一级入口。
 * `sessions` / `settings` 视图类型还留着，给 Minecraft 外壳或深链用。
 *
 * 人设和存储各自独立成一级：人设是天天要改的正文，存储是纯只读的观测面板，
 * 都塞进「设置」里只会让那一页变成什么都有的杂物间。留在「设置」下的是
 * 名副其实的配置——全局默认值和它们背后的 TOML 原文。
 *
 * **所有外壳都遍历这张表**，别再手写一遍按钮：漏掉一项的外壳会静悄悄少一个入口。
 */
export const NAV_ITEMS: { view: View; label: string; icon: NavIconKey }[] = [
  { view: 'memory', label: '记忆', icon: 'memory' },
  { view: 'tools', label: '工具', icon: 'tools' },
  { view: 'models', label: '模型', icon: 'models' },
  { view: 'theme', label: '外观', icon: 'theme' },
  { view: 'prompt', label: '提示词', icon: 'user' },
  { view: 'config', label: '设置', icon: 'config' },
  { view: 'storage', label: '存储', icon: 'storage' },
]
