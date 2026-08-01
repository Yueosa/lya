/**
 * 外壳：每套主题可以有自己的排版。
 *
 * # 边界在哪
 *
 * 之前我说过「换主题不该换 markup」，那句话对了一半——**对聊天视图是对的，
 * 推广到整个应用就错了**。要把消息树、HITL 表单、分支切换器写三遍，那是自找
 * 麻烦：那里的复杂度占了整个界面的九成，三份实现必然有两份是残的。
 *
 * 但「怎么走到一个会话」是另一回事。侧边栏也好、方块世界那种标题画面配一列
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

/** 应用里的几个去处。 */
export type View = 'home' | 'chat' | 'sessions' | 'settings'

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

/** 导航项，各套外壳都从这里取文案，免得三处各写一遍。 */
export const NAV_ITEMS: { view: View; label: string; icon: string }[] = [
  { view: 'chat', label: '开始对话', icon: '💬' },
  { view: 'sessions', label: '会话列表', icon: '📋' },
  { view: 'settings', label: '设置', icon: '⚙' },
]
