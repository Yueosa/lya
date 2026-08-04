<!--
  聊天消息流：时间线项渲染与编辑态。
  数据来自 useChat；滚动容器由父级提供 ref。
-->

<script setup lang="ts">
import { computed } from 'vue'

import {
  deleteMessage,
  editAndResend,
  readOnly,
  regenerate,
  switchToBranch,
  timeline,
} from '../../app/useChat'
import { prefs } from '../../app/usePrefs'
import type { Message, TimelineItem } from '../../model/timeline'
import { fmtBubbleTime, fmtBubbleTooltip } from '../../utils/dateFormat'
import Icon from '../../ui/Icon.vue'
import { messageStaggerDelay, useMotion } from '../../ui/useMotion'
import { openContextMenu, type MenuEntry } from '../../ui/useContextMenu'
import { confirm, confirmAsync } from '../../ui/useDialog'
import ChatAvatar from '../ChatAvatar.vue'
import CollapsibleBlock from '../CollapsibleBlock.vue'
import FormPreview from '../FormPreview.vue'
import HitlRecord from '../HitlRecord.vue'
import MarkdownBody from '../MarkdownBody.vue'
import {
  errorRetryable,
  formCall,
  hasText,
  isFirstToolBlockInBatch,
  lastTextBlockIndex,
  reasonLabel,
  shouldSkipToolBlock,
  toolArgsBroken,
  toolArgsText,
  toolBatchLabel,
  toolBlocksInMessage,
  toolLabel,
  visibleBlocks,
  providerSearchLabel,
} from './chatBlockHelpers'
import { state } from '../../app/chat/state'

const props = withDefaults(
  defineProps<{
  items: TimelineItem[]
  timelineOffset?: number
  motionReady?: boolean
  editing: { id: number; text: string } | null
}>(),
  { timelineOffset: 0, motionReady: true },
)

const timelineBase = computed(() => props.timelineOffset ?? 0)

const emit = defineEmits<{
  'update:editing': [value: { id: number; text: string } | null]
}>()

const { motionEnabled } = useMotion()

const prefSlice = computed(() => ({
  hideReasoning: prefs.hideReasoning,
  hideTools: prefs.hideTools,
  hideResolvedHitl: prefs.hideResolvedHitl,
}))

function messageOrdinal(timelineIndex: number): number {
  let count = 0
  for (let i = 0; i < timelineIndex; i++) {
    if (timeline.value[i]?.kind === 'message') count++
  }
  return count
}

function motionStyle(timelineIndex: number): Record<string, string> | undefined {
  if (!motionEnabled.value || !props.motionReady) return undefined
  return { '--local-msg-delay': messageStaggerDelay(messageOrdinal(timelineIndex)) }
}

function msgMotionClass(role: string): string | undefined {
  if (!motionEnabled.value || !props.motionReady) return undefined
  if (role === 'assistant') return 'lya-msg--assistant'
  if (role === 'user') return 'lya-msg--user'
  return undefined
}

function asideMotionClass(): string | undefined {
  if (!motionEnabled.value || !props.motionReady) return undefined
  return 'lya-aside-enter'
}

function timelineKey(item: TimelineItem, index: number): string {
  if (item.kind === 'message') return `msg-${item.message.id}`
  if (item.kind === 'time-gap') return `gap-${item.at}`
  if (item.kind === 'notice') return `notice-${item.at}-${index}`
  if (item.kind === 'error') return `error-${index}`
  return `item-${index}`
}

async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text)
}

function startEdit(message: Message, text: string): void {
  if (readOnly.value) return
  emit('update:editing', { id: message.id, text })
}

async function submitEdit(): Promise<void> {
  const draft = props.editing
  if (!draft?.text.trim()) return
  await editAndResend(draft.id, draft.text.trim())
  emit('update:editing', null)
}

function cancelEdit(): void {
  emit('update:editing', null)
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

function onEditKey(event: KeyboardEvent): void {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void submitEdit()
  }
  if (event.key === 'Escape') cancelEdit()
}
</script>

<template>
  <template v-for="(item, index) in items" :key="timelineKey(item, index)">
    <div v-if="item.kind === 'time-gap'" class="msg-time-separator">
      <span>{{ item.text }}</span>
    </div>
    <div v-else-if="item.kind === 'notice' && !prefs.hideNotices" class="chat__divider">
      {{ item.text }}
    </div>
    <div v-else-if="item.kind === 'error'" class="chat__error">
      <span>{{ reasonLabel(item.reason) }}</span>
      <button v-if="!readOnly && errorRetryable(item.reason)" class="btn btn--sm" @click="regen">
        重试
      </button>
    </div>

    <template v-else-if="item.kind === 'message'">
      <template v-for="(block, at) in visibleBlocks(item.message.blocks, prefSlice)" :key="at">
        <div v-if="block.type === 'reasoning'" class="chat__aside" :class="asideMotionClass()" :style="motionStyle(timelineBase + index)">
          <CollapsibleBlock
            icon="reasoning"
            label="思考"
            streaming
            :busy="item.message.status === 'streaming'"
            :auto-collapse="prefs.autoCollapseAside"
          >
            {{ block.text }}
          </CollapsibleBlock>
        </div>

        <div
          v-else-if="block.type === 'provider_search'"
          class="chat__aside chat__provider-search"
          :class="asideMotionClass()"
          :style="motionStyle(timelineBase + index)"
        >
          <span class="chat__provider-search-icon">🔍</span>
          <span>{{ providerSearchLabel(block) }}</span>
        </div>

        <div
          v-else-if="block.type === 'tool' && !shouldSkipToolBlock(item.message, at, item.message.blocks)"
          class="chat__aside"
          :class="asideMotionClass()"
          :style="motionStyle(timelineBase + index)"
        >
          <CollapsibleBlock
            v-if="item.message.toolBatch && isFirstToolBlockInBatch(item.message, at, item.message.blocks)"
            icon="tool"
            :label="toolBatchLabel(item.message.toolBatch, state.messages)"
            :busy="toolBlocksInMessage(item.message.blocks).some((tb) => !tb.call.result)"
            :auto-collapse="prefs.autoCollapseAside"
          >
            <div class="chat__tool-batch">
              <CollapsibleBlock
                v-for="(tb, ti) in toolBlocksInMessage(visibleBlocks(item.message.blocks, prefSlice))"
                :key="ti"
                icon="tool"
                :label="toolLabel(tb)"
                :busy="!tb.call.result"
                :failed="tb.call.result?.ok === false"
                :auto-collapse="prefs.autoCollapseAside"
              >
                <FormPreview
                  v-if="formCall(tb)"
                  :form="formCall(tb)!"
                  :pending="!tb.call.result"
                />
                <template v-else>
                  <div v-if="toolArgsText(tb.call)" class="chat__tool-args">
                    <span class="chat__tool-args-head" :class="{ 'chat__tool-args-head--bad': toolArgsBroken(tb.call) }">
                      参数
                    </span>
                    <pre class="chat__tool-args-body">{{ toolArgsText(tb.call) }}</pre>
                  </div>
                  <div v-if="tb.call.result" class="chat__tool-args">
                    <span class="chat__tool-args-head">结果</span>
                    <pre class="chat__tool-args-body">{{ tb.call.result.content }}</pre>
                  </div>
                  <template v-else>执行中…</template>
                </template>
              </CollapsibleBlock>
            </div>
          </CollapsibleBlock>
          <CollapsibleBlock
            v-else-if="!item.message.toolBatch"
            icon="tool"
            :label="toolLabel(block)"
            :busy="!block.call.result"
            :failed="block.call.result?.ok === false"
            :auto-collapse="prefs.autoCollapseAside"
          >
            <FormPreview
              v-if="formCall(block)"
              :form="formCall(block)!"
              :pending="!block.call.result"
            />
            <template v-else>
              <div v-if="toolArgsText(block.call)" class="chat__tool-args">
                <span class="chat__tool-args-head" :class="{ 'chat__tool-args-head--bad': toolArgsBroken(block.call) }">
                  参数
                </span>
                <pre class="chat__tool-args-body">{{ toolArgsText(block.call) }}</pre>
              </div>
              <div v-if="block.call.result" class="chat__tool-args">
                <span class="chat__tool-args-head">结果</span>
                <pre class="chat__tool-args-body">{{ block.call.result.content }}</pre>
              </div>
              <template v-else>执行中…</template>
            </template>
          </CollapsibleBlock>
        </div>

        <div v-else-if="block.type === 'hitl'" class="chat__aside" :class="asideMotionClass()" :style="motionStyle(timelineBase + index)">
          <HitlRecord :hitl="block.hitl" :answer="block.answer" />
        </div>

        <div
          v-else-if="block.type === 'text'"
          class="chat__row"
          :class="[`chat__row--${item.message.role}`, msgMotionClass(item.message.role)]"
          :style="motionStyle(timelineBase + index)"
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
                :value="editing.text"
                class="chat__edit"
                rows="3"
                @input="emit('update:editing', { id: item.message.id, text: ($event.target as HTMLTextAreaElement).value })"
                @keydown="onEditKey"
              />
              <MarkdownBody v-else :text="block.text" />
              <span v-if="item.message.status === 'streaming'" class="chat__caret" />
            </div>

            <div v-if="editing?.id === item.message.id" class="chat__edit-bar">
              <button class="btn btn--sm btn--primary" @click="submitEdit">发送</button>
              <button class="btn btn--sm" @click="cancelEdit">取消</button>
            </div>

            <div
              v-else-if="item.message.status !== 'streaming'"
              class="chat__foot"
              :class="`chat__foot--${item.message.role}`"
            >
              <span class="chat__time" v-tip="fmtBubbleTooltip(item.message.createdAt)">
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
              <div
                v-if="
                  item.message.branch
                  && block.type === 'text'
                  && at === lastTextBlockIndex(item.message.blocks, prefSlice)
                "
                class="chat__branch"
              >
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
          </div>
          <ChatAvatar v-if="item.message.role === 'user'" role="user" />
        </div>
      </template>

      <div
        v-if="item.message.status === 'streaming' && !hasText(item.message.blocks)"
        class="chat__row chat__row--assistant"
        :class="msgMotionClass('assistant')"
        :style="motionStyle(timelineBase + index)"
      >
        <ChatAvatar role="assistant" />
        <div class="chat__msg">
          <div class="bubble bubble--assistant"><span class="chat__caret" /></div>
        </div>
      </div>
    </template>
  </template>
</template>
