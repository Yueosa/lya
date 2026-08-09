import type { ChatScrollControls } from '../../views/chat/chatScrollKey'

/** 非 ChatView 子树（分支切换、编辑重发等）触达滚动控制。 */
let controls: ChatScrollControls | null = null

export function bindChatScroll(next: ChatScrollControls | null): void {
  controls = next
}

export function armSendFollow(): void {
  controls?.armSendFollow()
}

export function nudgeIfFollowing(): void {
  controls?.nudgeIfFollowing()
}
