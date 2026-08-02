<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import {
  deleteMessage,
  editAndResend,
  elapsed,
  loading,
  meta,
  phase,
  readOnly,
  regenerate,
  round,
  running,
  switchToBranch,
  timeline,
} from '../app/useChat'
import { setSidebarCollapsed, sidebarCollapsed } from '../app/useShell'
import { prefs } from '../app/usePrefs'
import type { Block, Message, TimelineItem } from '../model/timeline'
import { fmtBubbleTime, fmtBubbleTooltip } from '../utils/dateFormat'
import { parseFormCall } from '../utils/parseFormCall'
import Icon from '../ui/Icon.vue'
import { messageStaggerDelay, useMotion } from '../ui/useMotion'
import { openContextMenu, type MenuEntry } from '../ui/useContextMenu'
import { confirm, confirmAsync } from '../ui/useDialog'
import BranchTree from './BranchTree.vue'
import ChatAvatar from './ChatAvatar.vue'
import CollapsibleBlock from './CollapsibleBlock.vue'
import Composer from './Composer.vue'
import FormPreview from './FormPreview.vue'
import HitlRecord from './HitlRecord.vue'
import MarkdownBody from './MarkdownBody.vue'
import SessionDetail from './SessionDetail.vue'
import SessionSettings from './SessionSettings.vue'

const scroller = ref<HTMLElement | null>(null)
const treeOpen = ref(false)
const settingsOpen = ref(false)
const detailOpen = ref(false)

const editing = ref<{ id: number; text: string } | null>(null)

const { motionEnabled } = useMotion()

const scrollPercent = ref(100)
const scrollable = ref(false)
let programmaticScroll = false

watch(
  timeline,
  async () => {
    if (!prefs.followStream) return
    await nextTick()
    scrollBottom()
  },
  { deep: true },
)

watch(running, (on, was) => {
  if (was && !on && prefs.followStream) lastTurnFinished.value = true
})

const lastTurnFinished = ref(false)
const elapsedText = computed(() => `${(elapsed.value / 1000).toFixed(1)}s`)

const jumpState = computed<'hidden' | 'following' | 'finished' | 'percent'>(() => {
  if (!scrollable.value) return 'hidden'
  if (running.value && prefs.followStream) return 'following'
  if (lastTurnFinished.value) return 'finished'
  if (!running.value && scrollPercent.value >= 100) return 'hidden'
  return 'percent'
})

const jumpText = computed(() => {
  if (jumpState.value === 'following') return '跟随'
  if (jumpState.value === 'finished') return '完毕'
  return `${scrollPercent.value}%`
})

const jumpTip = computed(() =>
  jumpState.value === 'following' ? '取消跟随' : '跳到最新',
)

function visible(blocks: Block[]): Block[] {
  return blocks.filter((block) => {
    if (block.type === 'reasoning') return !prefs.hideReasoning
    if (block.type === 'tool') return !prefs.hideTools
    if (block.type === 'hitl') return !(prefs.hideResolvedHitl && block.answer !== undefined)
    return true
  })
}

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

async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text)
}

function startEdit(message: Message, text: string): void {
  if (readOnly.value) return
  editing.value = { id: message.id, text }
}

async function submitEdit(): Promise<void> {
  const draft = editing.value
  if (!draft?.text.trim()) return
  await editAndResend(draft.id, draft.text.trim())
  editing.value = null
}

function cancelEdit(): void {
  editing.value = null
}

function messageOrdinal(timelineIndex: number): number {
  let count = 0
  for (let i = 0; i < timelineIndex; i++) {
    if (timeline.value[i]?.kind === 'message') count++
  }
  return count
}

function motionStyle(timelineIndex: number): Record<string, string> | undefined {
  if (!motionEnabled.value) return undefined
  return { '--local-msg-delay': messageStaggerDelay(messageOrdinal(timelineIndex)) }
}

function msgMotionClass(role: string): string | undefined {
  if (!motionEnabled.value) return undefined
  if (role === 'assistant') return 'lya-msg--assistant'
  if (role === 'user') return 'lya-msg--user'
  return undefined
}

function asideMotionClass(): string | undefined {
  return motionEnabled.value ? 'lya-aside-enter' : undefined
}

function timelineKey(item: TimelineItem, index: number): string {
  if (item.kind === 'message') return `msg-${item.message.id}`
  if (item.kind === 'time-gap') return `gap-${item.at}`
  if (item.kind === 'notice') return `notice-${item.at}-${index}`
  if (item.kind === 'error') return `error-${index}`
  return `item-${index}`
}

async function regen(): Promise<void> {
  const ok = await confirm({
    title: '重新生成',
    message: '会回到上一条用户消息重跑，当前回复保留在另一分支。',
  })
  if (ok) await regenerate()
}

function messageMenu(event: MouseEvent, message: Message, text: string): void {
  if (readOnly.value) return
  const entries: MenuEntry[] = [
    { label: '复制', icon: 'copy', onSelect: () => void copyText(text) },
  ]
  if (message.role === 'user') {
    entries.push({ label: '编辑', icon: 'edit', onSelect: () => startEdit(message, text) })
  }
  if (message.role === 'assistant') {
    entries.push({ label: '重生成', icon: 'refresh', onSelect: () => void regen() })
  }
  entries.push({ separator: true })
  entries.push({
    label: '删除',
    icon: 'delete',
    danger: true,
    onSelect: async () => {
      await confirmAsync({
        title: '删除消息',
        message: '只能删末端消息。',
        confirmText: '删除',
        danger: true,
        run: () => deleteMessage(message.id),
      })
    },
  })
  openContextMenu(event, entries)
}

function hasText(blocks: Block[]): boolean {
  return blocks.some((block) => block.type === 'text')
}

function formCall(block: Extract<Block, { type: 'tool' }>) {
  if (block.call.name !== 'form') return null
  return parseFormCall(block.call.arguments)
}

function toolLabel(block: Extract<Block, { type: 'tool' }>): string {
  const form = formCall(block)
  if (form) return `form  ${form.title}`
  const args = block.call.arguments
  if (args && typeof args === 'object') {
    const first = Object.values(args as Record<string, unknown>)[0]
    if (typeof first === 'string' && first) return `${block.call.name}  ${first.slice(0, 60)}`
  }
  return block.call.name
}

function reasonLabel(reason: { kind: string; message?: string }): string {
  switch (reason.kind) {
    case 'failed':
      return `出错了：${reason.message ?? ''}`
    case 'max_rounds':
      return '工具轮数到上限'
    case 'cancelled':
      return '已停止'
    case 'empty_response':
      return '空回复'
    default:
      return reason.kind
  }
}

function onScroll(): void {
  const el = scroller.value
  if (!el) return
  const max = el.scrollHeight - el.clientHeight
  scrollable.value = max > 8
  if (max <= 0) {
    scrollPercent.value = 100
  } else {
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 2
    scrollPercent.value = atBottom ? 100 : Math.min(100, Math.round((el.scrollTop / max) * 100))
  }
  if (programmaticScroll) return
  if (running.value && scrollPercent.value < 92) {
    prefs.followStream = false
  }
  if (scrollPercent.value >= 100) lastTurnFinished.value = false
}

function scrollBottom(): void {
  nextTick(() => {
    const el = scroller.value
    if (!el) return
    programmaticScroll = true
    el.scrollTop = el.scrollHeight
    setTimeout(() => {
      programmaticScroll = false
    }, 80)
  })
}

function jumpLatest(): void {
  if (jumpState.value === 'following') {
    prefs.followStream = false
    return
  }
  if (running.value) prefs.followStream = true
  lastTurnFinished.value = false
  scrollBottom()
}

function onEditKey(event: KeyboardEvent): void {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void submitEdit()
  }
  if (event.key === 'Escape') cancelEdit()
}

onMounted(() => {
  nextTick(onScroll)
})
</script>

<template>
  <div class="chat">
    <div class="chat__main">
      <header class="chat__head" :class="{ 'chat__head--sidebar-collapsed': sidebarCollapsed }">
        <button
          v-if="sidebarCollapsed"
          class="btn btn--ghost chat__sidebar-btn"
          v-tip="'展开侧栏'"
          @click="setSidebarCollapsed(false)"
        >
          <Icon name="menu" size="sm" />
        </button>
        <span class="chat__title">{{ meta?.title || '未命名会话' }}</span>
        <span v-if="readOnly" class="chat__tag">已归档</span>
        <span class="chat__gap" />
        <button class="btn btn--sm" :class="{ 'btn--on': detailOpen }" @click="toggleDetail">
          <Icon name="info" size="sm" />
          <span>详情</span>
        </button>
        <button class="btn btn--sm" :class="{ 'btn--on': settingsOpen }" @click="toggleSettings">
          <Icon name="settings" size="sm" />
          <span>设置</span>
        </button>
        <button class="btn btn--sm" :class="{ 'btn--on': treeOpen }" @click="toggleTree">
          <Icon name="branch" size="sm" />
          <span>分支</span>
        </button>
      </header>

      <div v-if="phase" class="chat__status">
        <span>{{ phase.text }}</span>
        <span v-if="round > 1" class="chat__dim">第 {{ round }} 轮</span>
        <span class="chat__gap" />
        <span class="chat__dim">{{ elapsedText }}</span>
      </div>

      <div ref="scroller" class="chat__stream" @scroll="onScroll">
        <template v-for="(item, index) in timeline" :key="timelineKey(item, index)">
          <div v-if="item.kind === 'time-gap'" class="msg-time-separator">
            <span>{{ item.text }}</span>
          </div>
          <div v-else-if="item.kind === 'notice' && !prefs.hideNotices" class="chat__divider">
            {{ item.text }}
          </div>
          <div v-else-if="item.kind === 'error'" class="chat__error">
            {{ reasonLabel(item.reason) }}
          </div>

          <template v-else-if="item.kind === 'message'">
            <template v-for="(block, at) in visible(item.message.blocks)" :key="at">
              <div v-if="block.type === 'reasoning'" class="chat__aside" :class="asideMotionClass()" :style="motionStyle(index)">
                <CollapsibleBlock icon="reasoning" label="思考" :busy="item.message.status === 'streaming'">
                  {{ block.text }}
                </CollapsibleBlock>
              </div>

              <div v-else-if="block.type === 'tool'" class="chat__aside" :class="asideMotionClass()" :style="motionStyle(index)">
                <CollapsibleBlock
                  icon="tool"
                  :label="toolLabel(block)"
                  :busy="!block.call.result"
                  :failed="block.call.result?.ok === false"
                >
                  <FormPreview
                    v-if="formCall(block)"
                    :form="formCall(block)!"
                    :pending="!block.call.result"
                  />
                  <template v-else>{{ block.call.result?.content ?? '执行中…' }}</template>
                </CollapsibleBlock>
              </div>

              <div v-else-if="block.type === 'hitl'" class="chat__aside" :class="asideMotionClass()" :style="motionStyle(index)">
                <HitlRecord :hitl="block.hitl" :answer="block.answer" />
              </div>

              <div
                v-else
                class="chat__row"
                :class="[
                  `chat__row--${item.message.role}`,
                  msgMotionClass(item.message.role),
                ]"
                :style="motionStyle(index)"
              >
                <ChatAvatar v-if="item.message.role === 'assistant'" role="assistant" />
                <div class="chat__msg">
                  <div
                    class="bubble"
                    :class="[
                      `bubble--${item.message.role}`,
                      { 'bubble--interrupted': item.message.status === 'interrupted' },
                    ]"
                    @contextmenu.prevent="messageMenu($event, item.message, block.text)"
                  >
                    <textarea
                      v-if="editing?.id === item.message.id && item.message.role === 'user'"
                      v-model="editing.text"
                      class="chat__edit"
                      rows="3"
                      @keydown="onEditKey"
                    />
                    <MarkdownBody v-else :text="block.text" />
                    <span v-if="item.message.status === 'streaming'" class="chat__caret" />
                  </div>

                  <div
                    v-if="editing?.id === item.message.id"
                    class="chat__edit-bar"
                  >
                    <button class="btn btn--sm btn--primary" @click="submitEdit">发送</button>
                    <button class="btn btn--sm" @click="cancelEdit">取消</button>
                  </div>

                  <div
                    v-else-if="item.message.status !== 'streaming'"
                    class="chat__foot"
                    :class="`chat__foot--${item.message.role}`"
                  >
                    <span
                      class="chat__time"
                      v-tip="fmtBubbleTooltip(item.message.createdAt)"
                    >
                      {{ fmtBubbleTime(item.message.createdAt) }}
                    </span>
                    <div class="chat__actions">
                      <button class="chat__action" v-tip="'复制'" @click="copyText(block.text)">
                        <Icon name="copy" size="sm" />
                      </button>
                      <button
                        v-if="!readOnly && item.message.role === 'user'"
                        class="chat__action"
                        v-tip="'编辑并重发'"
                        @click="startEdit(item.message, block.text)"
                      >
                        <Icon name="edit" size="sm" />
                      </button>
                      <button
                        v-if="!readOnly && item.message.role === 'assistant'"
                        class="chat__action"
                        v-tip="'重新生成'"
                        @click="regen"
                      >
                        <Icon name="refresh" size="sm" />
                      </button>
                    </div>
                  </div>

                  <div v-if="item.message.branch" class="chat__branch">
                    <button
                      :disabled="item.message.branch.index === 0"
                      @click="switchToBranch(item.message.branch.siblingIds[item.message.branch.index - 1]!)"
                    >
                      <Icon name="chevronLeft" size="sm" />
                    </button>
                    <span>{{ item.message.branch.index + 1 }}/{{ item.message.branch.total }}</span>
                    <button
                      :disabled="item.message.branch.index === item.message.branch.total - 1"
                      @click="switchToBranch(item.message.branch.siblingIds[item.message.branch.index + 1]!)"
                    >
                      <Icon name="chevronRight" size="sm" />
                    </button>
                  </div>
                </div>
                <ChatAvatar v-if="item.message.role === 'user'" role="user" />
              </div>
            </template>

            <div
              v-if="item.message.status === 'streaming' && !hasText(item.message.blocks)"
              class="chat__row chat__row--assistant"
              :class="msgMotionClass('assistant')"
              :style="motionStyle(index)"
            >
              <ChatAvatar role="assistant" />
              <div class="chat__msg">
                <div class="bubble bubble--assistant"><span class="chat__caret" /></div>
              </div>
            </div>
          </template>
        </template>
      </div>

      <button
        v-if="jumpState !== 'hidden'"
        class="chat__jump"
        :class="{
          'chat__jump--follow': jumpState === 'following',
          'chat__jump--done': jumpState === 'finished',
        }"
        v-tip="jumpTip"
        @click="jumpLatest"
      >
        {{ jumpText }}
      </button>

      <Composer v-if="!readOnly" />

      <div v-if="loading" class="chat__loading" aria-live="polite">
        <span class="chat__loading-text">加载中…</span>
      </div>
    </div>

    <Transition name="lya-drawer">
      <aside v-if="detailOpen" class="chat__side">
        <header class="chat__side-head">
          <strong>详情</strong>
          <span class="chat__gap" />
          <button class="btn btn--sm btn--ghost" @click="detailOpen = false">
            <Icon name="chevronRight" size="sm" />
          </button>
        </header>
        <div class="chat__side-body"><SessionDetail /></div>
      </aside>
    </Transition>

    <Transition name="lya-drawer">
      <aside v-if="settingsOpen" class="chat__side">
        <header class="chat__side-head">
          <strong>会话设置</strong>
          <span class="chat__gap" />
          <button class="btn btn--sm btn--ghost" @click="settingsOpen = false">
            <Icon name="chevronRight" size="sm" />
          </button>
        </header>
        <div class="chat__side-body"><SessionSettings /></div>
      </aside>
    </Transition>

    <Transition name="lya-drawer">
      <BranchTree v-if="treeOpen" :open="true" @close="treeOpen = false" />
    </Transition>
  </div>
</template>

<style scoped>
.chat {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.chat__main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  position: relative;
}

.chat__head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: var(--border-width) solid var(--border);
}

.chat__head--sidebar-collapsed {
  padding-left: 12px;
}

.chat__sidebar-btn {
  color: var(--accent);
  padding: 4px 8px;
}

.chat__title {
  font-size: var(--text-md);
  font-weight: 600;
}

.chat__tag {
  padding: 1px 8px;
  border-radius: var(--radius-pill);
  background: var(--surface-active);
  color: var(--text-muted);
  font-size: var(--text-xs);
}

.chat__gap {
  flex: 1;
}

.chat__status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 20px;
  background: var(--bg-sunken);
  border-bottom: var(--border-width) solid var(--border);
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.chat__dim {
  color: var(--text-faint);
}

.chat__stream {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 16px 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  scrollbar-width: none;
}

.chat__stream::-webkit-scrollbar {
  display: none;
}

.chat__divider {
  align-self: center;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.chat__error {
  align-self: center;
  padding: 6px 12px;
  border: var(--border-width) solid var(--danger);
  border-radius: var(--radius-sm);
  background: var(--danger-soft);
  color: var(--danger);
  font-size: var(--text-sm);
}

.chat__row,
.chat__aside {
  width: min(1320px, 100%);
  margin: 0 auto;
  padding: 0 20px;
}

.chat__row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.chat__row--user {
  justify-content: flex-end;
}

.chat__row--assistant {
  justify-content: flex-start;
}

.chat__msg {
  display: flex;
  flex-direction: column;
  max-width: 78%;
  min-width: 0;
}

.chat__row--user .chat__msg {
  align-items: flex-end;
}

.chat__row--assistant .chat__msg {
  max-width: 88%;
}

.chat__edit {
  width: 100%;
  min-width: 240px;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: inherit;
  line-height: 1.5;
  resize: vertical;
  outline: none;
}

.chat__edit-bar {
  display: flex;
  gap: 6px;
  margin-top: 6px;
}

.chat__caret {
  display: inline-block;
  width: 7px;
  height: 1em;
  background: currentColor;
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}

.chat__foot {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  min-height: 28px;
}

.chat__foot--user {
  flex-direction: row-reverse;
}

.chat__time {
  color: var(--text-faint);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.chat__msg:hover .chat__time,
.chat__foot:focus-within .chat__time {
  opacity: 1;
}

.chat__actions {
  display: flex;
  gap: 6px;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.chat__msg:hover .chat__actions,
.chat__foot:focus-within .chat__actions {
  opacity: 1;
}

.chat__action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  min-height: 28px;
  padding: 4px 10px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text-muted);
  cursor: pointer;
}

.chat__action:hover {
  color: var(--text);
  border-color: var(--border-strong);
  background: var(--surface-hover);
}

.chat__branch {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.chat__branch button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: inherit;
  padding: 0 4px;
  cursor: pointer;
}

.chat__branch button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.chat__loading {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--bg) 72%, transparent);
  pointer-events: none;
}

.chat__loading-text {
  padding: 8px 16px;
  border-radius: var(--radius-pill);
  background: var(--surface);
  border: var(--border-width) solid var(--border);
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.chat__jump {
  position: absolute;
  right: 20px;
  bottom: 88px;
  z-index: 20;
  padding: 6px 14px;
  border-radius: 20px;
  border: var(--border-width) solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  cursor: pointer;
}

.chat__jump--follow {
  background: var(--success);
  border-color: var(--success);
  color: var(--on-accent);
  animation: lya-jump-pulse 1.4s ease infinite;
}

.chat__jump--done {
  background: var(--info);
  border-color: var(--info);
  color: var(--on-accent);
}

.chat__side {
  flex-shrink: 0;
  width: min(380px, 42vw);
  display: flex;
  flex-direction: column;
  background: var(--bg-sunken);
  border-left: var(--border-width) solid var(--border);
}

.chat__side-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px;
  border-bottom: var(--border-width) solid var(--border);
}

.chat__side-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

@media (max-width: 720px) {
  .chat__row,
  .chat__aside {
    padding: 0 12px;
  }

  .chat__msg {
    max-width: 92%;
  }

  .chat__actions,
  .chat__time {
    opacity: 1;
  }
}
</style>
