<!--
  聊天视图。

  **视图只有一份实现**（见 shell/types.ts 的边界说明）——外壳可以三套，这里不行：
  消息树、折叠块、以后的 HITL 表单占了整个界面九成的复杂度，写三遍必然有两份是残的。

  显示偏好（不看思考、不看工具）只在这里生效，数据层始终保留全部块，见 usePrefs。
-->

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'

import { canSend, pendingHitl, readOnly, running, send, stop, timeline } from '../app/useChat'
import { prefs } from '../app/usePrefs'
import type { Block } from '../model/timeline'
import CollapsibleBlock from './CollapsibleBlock.vue'
import HitlRecord from './HitlRecord.vue'
import HitlTray from './HitlTray.vue'
import MarkdownBody from './MarkdownBody.vue'

const draft = ref('')
const scroller = ref<HTMLElement | null>(null)

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

/** 按显示偏好过滤块。数据里始终是全的，这里只决定画不画。 */
function visible(blocks: Block[]): Block[] {
  return blocks.filter((block) => {
    if (block.type === 'reasoning') return !prefs.hideReasoning
    if (block.type === 'tool') return !prefs.hideTools
    if (block.type === 'hitl') return !(prefs.hideResolvedHitl && block.answer !== undefined)
    return true
  })
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

async function submit(): Promise<void> {
  const text = draft.value
  if (!text.trim() || !canSend.value) return
  draft.value = ''
  await send(text)
}

/** 回车发送，Shift+回车换行。 */
function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void submit()
  }
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
    <div ref="scroller" class="chat__stream">
      <template v-for="(item, index) in timeline" :key="index">
        <div v-if="item.kind === 'time-gap'" class="chat__divider">
          {{ timeLabel(item.at) }}
        </div>

        <div v-else-if="item.kind === 'notice'" class="chat__divider">
          {{ item.text }}
        </div>

        <div v-else-if="item.kind === 'error'" class="chat__error">
          {{ reasonLabel(item.reason) }}
        </div>

        <!--
          一条消息拆成若干行，而不是全塞进一个气泡。

          气泡是「说出来的话」，思考和工具调用不是——它们是过程。挤在一起的话，
          展开一段长输出会把气泡撑宽、上面的正文跟着重排；工具输出又是等宽的、
          常常很宽，压在 76% 里也难读。
        -->
        <template v-else>
          <template v-for="(block, at) in visible(item.message.blocks)" :key="at">
            <div v-if="block.type === 'reasoning'" class="chat__aside-row">
              <CollapsibleBlock
                icon="💭"
                label="思考"
                :busy="item.message.status === 'streaming'"
              >
                {{ block.text }}
              </CollapsibleBlock>
            </div>

            <div v-else-if="block.type === 'tool'" class="chat__aside-row">
              <CollapsibleBlock
                icon="🔧"
                :label="toolLabel(block)"
                :busy="!block.call.result"
                :failed="block.call.result?.ok === false"
              >
                {{ block.call.result?.content ?? '执行中…' }}
              </CollapsibleBlock>
            </div>

            <!-- 未决的那条由底部托盘接管，历史里只留一条已答复的记录 -->
            <div v-else-if="block.type === 'hitl'" class="chat__aside-row">
              <HitlRecord :hitl="block.hitl" :answer="block.answer" />
            </div>

            <div v-else class="chat__row" :class="`chat__row--${item.message.role}`">
              <div class="bubble" :class="`bubble--${item.message.role}`">
                <!-- 用户消息不走 Markdown：你打的字应当原样显示，
                     不该因为随手用了 * 或 # 就变了样 -->
                <div v-if="item.message.role === 'user'" class="chat__text">{{ block.text }}</div>
                <MarkdownBody v-else :text="block.text" />
                <span v-if="item.message.status === 'streaming'" class="chat__caret" />
                <span v-if="item.message.status === 'interrupted'" class="chat__interrupted">
                  （已中断）
                </span>
              </div>
            </div>
          </template>

          <!-- 刚开始生成、还没有一个字的时候也要有个东西转着，
               否则从发出到第一个 token 之间界面是空的 -->
          <div
            v-if="item.message.status === 'streaming' && !hasText(item.message.blocks)"
            class="chat__row"
          >
            <div class="bubble bubble--assistant">
              <span class="chat__caret" />
            </div>
          </div>
        </template>
      </template>
    </div>

    <HitlTray />

    <!-- 归档的会话只能回看。后端也会拒绝写入，这里收掉输入框是为了不让人白打字 -->
    <div v-if="readOnly" class="chat__archived">
      这个会话已归档，只能回看。想继续聊的话先在列表里把它取回。
    </div>

    <form v-else class="chat__composer" @submit.prevent="submit">
      <textarea
        v-model="draft"
        class="input chat__input"
        rows="2"
        :placeholder="pendingHitl ? '先答复上面那个再继续' : '说点什么…（回车发送，Shift+回车换行）'"
        :disabled="!!pendingHitl"
        @keydown="onKeydown"
      />
      <button v-if="running" type="button" class="btn btn--danger" @click="stop">停止</button>
      <button v-else type="submit" class="btn btn--primary" :disabled="!canSend">发送</button>
    </form>
  </div>
</template>

<style scoped>
.chat {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.chat__stream {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 20px 5%;
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

.chat__row {
  display: flex;
}

.chat__row--user {
  justify-content: flex-end;
}

.bubble {
  max-width: 76%;
  padding: 9px 13px;
  border-radius: var(--bubble-radius);
  font-size: var(--text-md);
}

.bubble--user {
  background: var(--accent);
  color: var(--on-accent);
  border-bottom-right-radius: var(--bubble-tail-radius);
}

.bubble--assistant,
.bubble--hitl,
.bubble--tool {
  background: var(--surface);
  border: var(--border-width) solid var(--border);
  border-bottom-left-radius: var(--bubble-tail-radius);
}

.chat__text {
  white-space: pre-wrap;
  word-break: break-word;
}

/* 过程类的块占整宽，这样展开时不会去挤气泡，长输出也有地方摊开 */
.chat__aside-row {
  width: 100%;
  min-width: 0;
}

.chat__todo {
  padding: 6px 10px;
  border: var(--border-width) dashed var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: var(--text-sm);
}

/* 流式中的光标，让人知道还在写 */
.chat__caret {
  display: inline-block;
  width: 7px;
  height: 1em;
  background: var(--accent);
  vertical-align: text-bottom;
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}

.chat__interrupted {
  color: var(--text-faint);
  font-size: var(--text-sm);
}

.chat__archived {
  padding: 14px 5%;
  border-top: var(--border-width) solid var(--border);
  background: var(--bg-sunken);
  color: var(--text-muted);
  font-size: var(--text-sm);
  text-align: center;
}

.chat__composer {
  display: flex;
  gap: 8px;
  align-items: flex-end;
  padding: 12px 5%;
  border-top: var(--border-width) solid var(--border);
  background: var(--bg-sunken);
}

.chat__input {
  flex: 1;
  height: auto;
  padding: 8px 12px;
  resize: vertical;
  font-family: inherit;
  line-height: var(--leading);
}
</style>
