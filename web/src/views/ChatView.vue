<!--
  聊天视图。

  这一版**只渲染纯文本**，没有 Markdown、没有折叠块、没有 HITL。目的不是好看，
  是先把管道跑通：store 和时间线模型此前只对着 wire dump 写过，从没跑过真实的
  SSE 流。事件到达顺序、流式节奏、落库时机这些只有连上真后端才知道，那里有偏差
  的话，建立在上面的渲染工作全得返工。

  **视图只有一份实现**（见 shell/types.ts 的边界说明）——外壳可以三套，这里不行。
-->

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'

import { canSend, running, send, stop, timeline } from '../app/useChat'

const draft = ref('')
const scroller = ref<HTMLElement | null>(null)

// 有新内容就滚到底。真正的「是否跟随」逻辑等做那个跳到最新的按钮时再补
watch(
  timeline,
  async () => {
    await nextTick()
    const el = scroller.value
    if (el) el.scrollTop = el.scrollHeight
  },
  { deep: true },
)

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

        <div v-else class="chat__row" :class="`chat__row--${item.message.role}`">
          <div class="bubble" :class="`bubble--${item.message.role}`">
            <template v-for="(block, at) in item.message.blocks" :key="at">
              <!-- 思考与工具先原样摊开，折叠留到下一轮 -->
              <div v-if="block.type === 'reasoning'" class="chat__aside">
                💭 {{ block.text }}
              </div>
              <div v-else-if="block.type === 'tool'" class="chat__aside">
                🔧 {{ block.call.name }}
                <template v-if="block.call.result">
                  → {{ block.call.result.content.slice(0, 200) }}
                </template>
                <template v-else>执行中…</template>
              </div>
              <div v-else-if="block.type === 'hitl'" class="chat__aside">
                ✋ 需要你决定（{{ block.hitl.type }}），界面还没做
              </div>
              <div v-else class="chat__text">{{ block.text }}</div>
            </template>
            <span v-if="item.message.status === 'streaming'" class="chat__caret" />
            <span v-if="item.message.status === 'interrupted'" class="chat__interrupted">
              （已中断）
            </span>
          </div>
        </div>
      </template>
    </div>

    <form class="chat__composer" @submit.prevent="submit">
      <textarea
        v-model="draft"
        class="input chat__input"
        rows="2"
        placeholder="说点什么…（回车发送，Shift+回车换行）"
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

.chat__aside {
  margin: 4px 0;
  padding: 6px 8px;
  border-left: 3px solid var(--info);
  background: var(--bg-sunken);
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  white-space: pre-wrap;
  word-break: break-word;
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
