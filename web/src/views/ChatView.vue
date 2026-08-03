<!--
  聊天主视图：组装顶栏、时间线、输入框与侧栏抽屉。
-->

<script setup lang="ts">
import { ref } from 'vue'

import { loading, hydrating, readOnly } from '../app/useChat'
import BranchTree from './BranchTree.vue'
import Composer from './Composer.vue'
import SessionPanel from './session/SessionPanel.vue'
import ChatHeader from './chat/ChatHeader.vue'
import ChatSideDrawer from './chat/ChatSideDrawer.vue'
import ChatStatusBar from './chat/ChatStatusBar.vue'
import ChatTimeline from './chat/ChatTimeline.vue'
import ScrollJumpButton from './chat/ScrollJumpButton.vue'
import { useChatScroll } from './chat/useChatScroll'

import './chat/chat.css'

const scroller = ref<HTMLElement | null>(null)
const treeOpen = ref(false)
const sessionOpen = ref(false)
const editing = ref<{ id: number; text: string } | null>(null)

const { displayTimeline, timelineOffset, timelineReady, sessionEnterMotion, jumpState, jumpText, jumpTip, onScroll, jumpLatest } =
  useChatScroll(scroller)

function closePanels(except?: 'tree' | 'session'): void {
  if (except !== 'tree') treeOpen.value = false
  if (except !== 'session') sessionOpen.value = false
}

function toggleTree(): void {
  closePanels('tree')
  treeOpen.value = !treeOpen.value
}

function toggleSession(): void {
  closePanels('session')
  sessionOpen.value = !sessionOpen.value
}
</script>

<template>
  <div class="chat">
    <div class="chat__main">
      <ChatHeader
        :session-open="sessionOpen"
        :tree-open="treeOpen"
        @toggle-session="toggleSession"
        @toggle-tree="toggleTree"
      />

      <ChatStatusBar />

      <div
        ref="scroller"
        class="chat__stream"
        :class="{ 'chat__stream--loading': loading || hydrating || !timelineReady }"
        @scroll="onScroll"
      >
        <ChatTimeline
          v-model:editing="editing"
          :items="displayTimeline"
          :timeline-offset="timelineOffset"
          :motion-ready="sessionEnterMotion"
        />
      </div>

      <ScrollJumpButton
        :jump-state="jumpState"
        :jump-text="jumpText"
        :jump-tip="jumpTip"
        @jump="jumpLatest"
      />

      <Composer v-if="!readOnly" />

      <div v-if="loading || hydrating || !timelineReady" class="chat__loading" aria-live="polite">
        <span class="chat__loading-spinner" aria-hidden="true" />
        <span class="chat__loading-text">加载中…</span>
      </div>
    </div>

    <Transition name="lya-drawer">
      <ChatSideDrawer v-if="sessionOpen" wide title="会话" @close="sessionOpen = false">
        <SessionPanel layout="drawer" />
      </ChatSideDrawer>
    </Transition>

    <Transition name="lya-drawer">
      <BranchTree v-if="treeOpen" :open="true" @close="treeOpen = false" />
    </Transition>
  </div>
</template>
