<!--
  聊天视图。

  **只有一份实现**（见 shell/types.ts 的边界）——外壳可以三套，消息树、折叠块、
  HITL 表单这些占了九成复杂度的东西不行。

  布局：头部 + 状态条 + 消息流 + 输入区，右侧可推出分支树。分支树做成侧栏而不是
  弹窗，是为了能一边看树一边看对话——切完分支想立刻确认切对了没有。
-->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import {
  deleteMessage,
  editAndResend,
  elapsed,
  meta,
  phase,
  readOnly,
  regenerate,
  round,
  switchBranch,
  timeline,
} from '../app/useChat'
import { prefs } from '../app/usePrefs'
import type { Block, Message } from '../model/timeline'
import { openContextMenu, type MenuEntry } from '../ui/useContextMenu'
import { confirm, confirmAsync, prompt } from '../ui/useDialog'
import BranchTree from './BranchTree.vue'
import CollapsibleBlock from './CollapsibleBlock.vue'
import Composer from './Composer.vue'
import HitlRecord from './HitlRecord.vue'
import MarkdownBody from './MarkdownBody.vue'

const scroller = ref<HTMLElement | null>(null)
const treeOpen = ref(false)

watch(
  timeline,
  async () => {
    if (!prefs.followStream) return
    await nextTick()
    const el = scroller.value
    if (el) el.scrollTop = el.scrollHeight
  },
  { deep: true },
)

/** 跑了多久，一位小数就够——再精确也没人看。 */
const elapsedText = computed(() => `${(elapsed.value / 1000).toFixed(1)}s`)

/** 按显示偏好过滤块。数据里始终是全的，这里只决定画不画。 */
function visible(blocks: Block[]): Block[] {
  return blocks.filter((block) => {
    if (block.type === 'reasoning') return !prefs.hideReasoning
    if (block.type === 'tool') return !prefs.hideTools
    if (block.type === 'hitl') return !(prefs.hideResolvedHitl && block.answer !== undefined)
    return true
  })
}

/**
 * 气泡上的操作。
 *
 * 都会改动消息树，所以只读会话里一个都不给——后端也会拒，这里不弹是为了不让人
 * 白点一下才知道。
 */
function messageMenu(event: MouseEvent, message: Message, text: string): void {
  if (readOnly.value) return
  const entries: MenuEntry[] = [
    { label: '复制', icon: '⧉', onSelect: () => void navigator.clipboard.writeText(text) },
  ]

  if (message.role === 'user') {
    entries.push({
      label: '编辑并重发',
      icon: '✎',
      onSelect: async () => {
        const next = await prompt({ title: '改一下再发', initial: text })
        if (next === null || !next.trim()) return
        // 后端会分叉到这条的父节点再追加，旧问法与旧回答留成并列分支
        await editAndResend(message.id, next.trim())
      },
    })
  }

  if (message.role === 'assistant') {
    entries.push({
      label: '换个答法',
      icon: '↻',
      onSelect: async () => {
        const ok = await confirm({
          title: '重新生成？',
          message: '会回到你上一条消息重跑。原来这条留在另一条分支上，随时能切回去。',
        })
        if (ok) await regenerate()
      },
    })
  }

  entries.push({ separator: true })
  entries.push({
    label: '删除这条',
    icon: '🗑',
    danger: true,
    onSelect: async () => {
      await confirmAsync({
        title: '删掉这条消息？',
        message: '只能删末端的消息，中间的要先删它后面的。',
        confirmText: '删除',
        danger: true,
        run: () => deleteMessage(message.id),
      })
    },
  })

  openContextMenu(event, entries)
}

/** 这条消息有没有正文可显示。 */
function hasText(blocks: Block[]): boolean {
  return blocks.some((block) => block.type === 'text')
}

/** 工具卡片的标题：名字加一句参数摘要，折起来时也知道它干了什么。 */
function toolLabel(block: Extract<Block, { type: 'tool' }>): string {
  const args = block.call.arguments
  if (args && typeof args === 'object') {
    const first = Object.values(args as Record<string, unknown>)[0]
    if (typeof first === 'string' && first) return `${block.call.name}  ${first.slice(0, 60)}`
  }
  return block.call.name
}

function timeLabel(at: string): string {
  const date = new Date(at)
  const today = new Date().toDateString() === date.toDateString()
  const clock = date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  return today ? clock : `${date.toLocaleDateString('zh-CN')} ${clock}`
}

/** 一轮为什么结束，说人话。 */
function reasonLabel(reason: { kind: string; message?: string }): string {
  switch (reason.kind) {
    case 'failed':
      return `出错了：${reason.message}`
    case 'max_rounds':
      return '工具调用轮数到上限了，本轮先停下'
    case 'cancelled':
      return '已停止'
    case 'empty_response':
      return '模型什么都没说'
    default:
      return reason.kind
  }
}
</script>

<template>
  <div class="chat">
    <div class="chat__main">
      <header class="chat__head">
        <span class="chat__title">{{ meta?.title || '未命名会话' }}</span>
        <span v-if="readOnly" class="chat__tag">已归档 · 只读</span>
        <span class="chat__gap" />
        <button
          class="btn btn--sm"
          :class="{ 'btn--primary': treeOpen }"
          @click="treeOpen = !treeOpen"
        >
          ⑂ 分支
        </button>
      </header>

      <!-- 跑起来时才有，让人知道它卡在哪一步而不是干等 -->
      <div v-if="phase" class="chat__status">
        <span>{{ phase.icon }}</span>
        <span>{{ phase.text }}</span>
        <span v-if="round > 1" class="chat__dim">第 {{ round }} 轮</span>
        <span class="chat__gap" />
        <span class="chat__dim">⏱ {{ elapsedText }}</span>
      </div>

      <div ref="scroller" class="chat__stream">
        <template v-for="(item, index) in timeline" :key="index">
          <div v-if="item.kind === 'time-gap'" class="chat__divider">{{ timeLabel(item.at) }}</div>
          <div v-else-if="item.kind === 'notice'" class="chat__divider">{{ item.text }}</div>
          <div v-else-if="item.kind === 'error'" class="chat__error">
            {{ reasonLabel(item.reason) }}
          </div>

          <!--
            一条消息拆成若干行。气泡是「说出来的话」，思考和工具调用是过程——
            挤在一起的话，展开一段长输出会把气泡撑宽、上面的正文跟着重排。
          -->
          <template v-else>
            <template v-for="(block, at) in visible(item.message.blocks)" :key="at">
              <div v-if="block.type === 'reasoning'" class="chat__aside">
                <CollapsibleBlock
                  icon="💭"
                  label="思考"
                  :busy="item.message.status === 'streaming'"
                >
                  {{ block.text }}
                </CollapsibleBlock>
              </div>

              <div v-else-if="block.type === 'tool'" class="chat__aside">
                <CollapsibleBlock
                  icon="🔧"
                  :label="toolLabel(block)"
                  :busy="!block.call.result"
                  :failed="block.call.result?.ok === false"
                >
                  {{ block.call.result?.content ?? '执行中…' }}
                </CollapsibleBlock>
              </div>

              <div v-else-if="block.type === 'hitl'" class="chat__aside">
                <HitlRecord :hitl="block.hitl" :answer="block.answer" />
              </div>

              <div v-else class="chat__row" :class="`chat__row--${item.message.role}`">
                <div class="chat__msg">
                  <div
                    class="bubble"
                    :class="`bubble--${item.message.role}`"
                    @contextmenu.prevent="messageMenu($event, item.message, block.text)"
                  >
                    <!-- 用户消息不走 Markdown：你打的字应当原样显示，
                         不该因为随手用了 * 或 # 就变了样 -->
                    <div v-if="item.message.role === 'user'" class="chat__text">
                      {{ block.text }}
                    </div>
                    <MarkdownBody v-else :text="block.text" />
                    <span v-if="item.message.status === 'streaming'" class="chat__caret" />
                    <span v-if="item.message.status === 'interrupted'" class="chat__dim">
                      （已中断）
                    </span>
                  </div>

                  <!-- 这里分过叉。没有这个切换器，树就退化成了列表 -->
                  <div v-if="item.message.branch" class="chat__branch">
                    <button
                      :disabled="item.message.branch.index === 0"
                      @click="switchBranch(item.message.branch.siblingIds[item.message.branch.index - 1]!)"
                    >
                      ‹
                    </button>
                    <span>{{ item.message.branch.index + 1 }}/{{ item.message.branch.total }}</span>
                    <button
                      :disabled="item.message.branch.index === item.message.branch.total - 1"
                      @click="switchBranch(item.message.branch.siblingIds[item.message.branch.index + 1]!)"
                    >
                      ›
                    </button>
                  </div>
                </div>
              </div>
            </template>

            <!-- 刚开始生成、还没有一个字的时候也要有东西转着，
                 否则从发出到第一个 token 之间界面是空的 -->
            <div
              v-if="item.message.status === 'streaming' && !hasText(item.message.blocks)"
              class="chat__row"
            >
              <div class="bubble bubble--assistant"><span class="chat__caret" /></div>
            </div>
          </template>
        </template>
      </div>

      <!-- 归档的会话不显示输入区。只读只是不能发消息——折叠、切分支、看树照常 -->
      <Composer v-if="!readOnly" />
    </div>

    <BranchTree :open="treeOpen" @close="treeOpen = false" />
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
}

.chat__head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 24px;
  border-bottom: var(--border-width) solid var(--border);
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
  padding: 5px 24px;
  background: var(--bg-sunken);
  border-bottom: var(--border-width) solid var(--border);
  font-size: var(--text-xs);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.chat__dim {
  color: var(--text-faint);
}

.chat__stream {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 18px 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
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

/* 消息与输入区共用同一个宽度上限，视线不会左右跳 */
.chat__row,
.chat__aside {
  width: min(1100px, 100%);
  margin: 0 auto;
  padding: 0 24px;
}

.chat__row {
  display: flex;
}

.chat__row--user {
  justify-content: flex-end;
}

.chat__msg {
  display: flex;
  flex-direction: column;
  max-width: 78%;
  /* flex 子项默认 min-width:auto，不肯收缩到比内容更窄——一段长代码就能把它
     撑破 max-width，连带整个页面横向溢出。这一行是那个坑的解药 */
  min-width: 0;
}

.chat__row--user .chat__msg {
  align-items: flex-end;
}

/* 助手那侧宽一些：代码块和表格最需要横向空间，而用户消息通常就一两句 */
.chat__row--assistant .chat__msg {
  max-width: 88%;
}

.bubble {
  min-width: 0;
  padding: 10px 14px;
  border-radius: var(--bubble-radius);
  font-size: var(--text-md);
}

.bubble--user {
  background: var(--accent);
  color: var(--on-accent);
  border-bottom-right-radius: var(--bubble-tail-radius);
}

.bubble--assistant {
  background: var(--surface);
  border: var(--border-width) solid var(--border);
  border-bottom-left-radius: var(--bubble-tail-radius);
}

.chat__text {
  white-space: pre-wrap;
  word-break: break-word;
}

/* 流式中的光标，让人知道还在写 */
.chat__caret {
  display: inline-block;
  width: 7px;
  height: 1em;
  background: currentColor;
  vertical-align: text-bottom;
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}

.chat__branch {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 3px;
  color: var(--text-faint);
  font-size: var(--text-xs);
}

.chat__branch button {
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  padding: 0 2px;
  cursor: pointer;
}

.chat__branch button:disabled {
  opacity: 0.35;
  cursor: default;
}

@media (max-width: 720px) {
  .chat__head,
  .chat__status {
    padding-left: 12px;
    padding-right: 12px;
  }

  .chat__row,
  .chat__aside {
    padding: 0 12px;
  }

  .chat__msg {
    max-width: 92%;
  }
}
</style>
