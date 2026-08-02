<!--
  聊天主视图：组装顶栏、时间线、输入框与侧栏抽屉。
-->

<script setup lang="ts">
import { ref } from 'vue'

import { loading, readOnly } from '../app/useChat'
import BranchTree from './BranchTree.vue'
import Composer from './Composer.vue'
import SessionDetail from './SessionDetail.vue'
import SessionSettings from './SessionSettings.vue'
import ChatHeader from './chat/ChatHeader.vue'
import ChatSideDrawer from './chat/ChatSideDrawer.vue'
import ChatStatusBar from './chat/ChatStatusBar.vue'
import ChatTimeline from './chat/ChatTimeline.vue'
import ScrollJumpButton from './chat/ScrollJumpButton.vue'
import { useChatScroll } from './chat/useChatScroll'

import './chat/chat.css'

const scroller = ref<HTMLElement | null>(null)
const treeOpen = ref(false)
const settingsOpen = ref(false)
const detailOpen = ref(false)
const editing = ref<{ id: number; text: string } | null>(null)

const { displayTimeline, jumpState, jumpText, jumpTip, onScroll, jumpLatest } =
  useChatScroll(scroller)

function closePanels(except?: 'tree' | 'settings' | 'detail'): void {
  if (except !== 'tree') treeOpen.value = false
  if (except !== 'settings') settingsOpen.value = false
  if (except !== 'detail') detailOpen.value = false
}

function toggleTree(): void {
  closePanels('tree')
  treeOpen.value = !treeOpen.value
}

function toggleSettings(): void {
  closePanels('settings')
  settingsOpen.value = !settingsOpen.value
}

function toggleDetail(): void {
  closePanels('detail')
  detailOpen.value = !detailOpen.value
}
</script>

<template>
  <div class="chat">
    <div class="chat__main">
      <ChatHeader
        :detail-open="detailOpen"
        :settings-open="settingsOpen"
        :tree-open="treeOpen"
        @toggle-detail="toggleDetail"
        @toggle-settings="toggleSettings"
        @toggle-tree="toggleTree"
      />

      <ChatStatusBar />

      <div ref="scroller" class="chat__stream" @scroll="onScroll">
        <ChatTimeline v-model:editing="editing" :items="displayTimeline" />
      </div>

      <ScrollJumpButton
        :jump-state="jumpState"
        :jump-text="jumpText"
        :jump-tip="jumpTip"
        @jump="jumpLatest"
      />

      <Composer v-if="!readOnly" />

      <div v-if="loading" class="chat__loading" aria-live="polite">
        <span class="chat__loading-text">加载中…</span>
      </div>
    </div>

    <Transition name="lya-drawer">
      <ChatSideDrawer v-if="detailOpen" title="详情" @close="detailOpen = false">
        <SessionDetail />
      </ChatSideDrawer>
    </Transition>

    <Transition name="lya-drawer">
      <ChatSideDrawer v-if="settingsOpen" title="会话设置" @close="settingsOpen = false">
        <SessionSettings />
      </ChatSideDrawer>
    </Transition>

    <Transition name="lya-drawer">
      <BranchTree v-if="treeOpen" :open="true" @close="treeOpen = false" />
    </Transition>
  </div>
</template>
