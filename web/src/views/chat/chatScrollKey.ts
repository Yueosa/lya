import type { InjectionKey } from 'vue'

/** 聊天区滚动控制，由 ChatView 提供、Composer 等在发送前调用。 */
export interface ChatScrollControls {
  /** 发送消息前调用：重新开启跟随并滚到底。 */
  armSendFollow: () => void
  /** 输入框扩行等布局变化时：已在跟随才补滚，不强行改偏好。 */
  nudgeIfFollowing: () => void
}

export const chatScrollKey: InjectionKey<ChatScrollControls> = Symbol('chatScroll')
