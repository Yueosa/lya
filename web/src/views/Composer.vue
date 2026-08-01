<!--
  输入区。

  布局照搬上一代验证过的那套：**两侧摆最常改的东西，中间是胶囊输入框，末端一个
  圆形发送键**。模式和模型每轮都可能想换，放在手边；低频的收进设置页。

  发送与停止是同一个位置的两个状态，不是并排两个按钮——生成中你唯一想做的事就是
  叫停，多一个灰着的发送键只是噪音。

  文本框从一行起，跟着内容长到 150px 封顶再内部滚动。不封顶的话贴一大段进来，
  输入框会把整个对话顶出屏幕。
-->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import type { Mode } from '../api/wire'
import {
  canSend,
  meta,
  models,
  pendingHitl,
  running,
  send,
  setMode,
  setModel,
  stop,
} from '../app/useChat'
import { prefs } from '../app/usePrefs'
import { openContextMenu } from '../ui/useContextMenu'
import HitlTray from './HitlTray.vue'

const draft = ref('')
const input = ref<HTMLTextAreaElement | null>(null)

/** 文本框最高长到这里，再多就内部滚动。 */
const MAX_HEIGHT = 150

const MODES: { id: Mode; label: string; icon: string; hint: string }[] = [
  { id: 'ask', label: '问答', icon: '👁', hint: '只能看，不能改任何东西' },
  { id: 'edit', label: '编辑', icon: '✎', hint: '能读能写文件，不能执行命令' },
  { id: 'agent', label: '代理', icon: '⚙', hint: '什么都能做，含执行命令' },
]

const mode = computed(() => meta.value?.work_mode ?? 'agent')

const modelLabel = computed(() => {
  const id = meta.value?.model_id
  if (!id) return '默认模型'
  return models.value.find((model) => model.id === id)?.name ?? id
})

/** 等你答复时不给打字——后端也会拒，留个能打字发不出去的框只会让人白打。 */
const blocked = computed(() => pendingHitl.value !== null)

const placeholder = computed(() =>
  blocked.value ? '先答复上面那个再继续' : '说点什么…（Enter 发送，Shift+Enter 换行）',
)

function grow(): void {
  const el = input.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, MAX_HEIGHT)}px`
  el.style.overflowY = el.scrollHeight > MAX_HEIGHT ? 'auto' : 'hidden'
}

watch(draft, () => void nextTick(grow))

async function submit(): Promise<void> {
  const text = draft.value
  if (!text.trim() || !canSend.value) return
  draft.value = ''
  await nextTick()
  grow()
  await send(text)
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void submit()
  }
}

function pickModel(event: MouseEvent): void {
  const current = meta.value?.model_id ?? null
  openContextMenu(event, [
    { label: '默认模型', icon: current === null ? '●' : '○', onSelect: () => void setModel(null) },
    { separator: true },
    ...models.value.map((model) => ({
      label: model.api_key_placeholder ? `${model.name}（未配密钥）` : model.name,
      icon: model.id === current ? '●' : '○',
      // 密钥还是占位符的选了也跑不起来，灰掉比让人撞一次 401 强
      disabled: model.api_key_placeholder,
      onSelect: () => void setModel(model.id),
    })),
  ])
}
</script>

<template>
  <div class="composer">
    <HitlTray />

    <div class="composer__toolbar">
      <button
        class="btn btn--sm"
        :class="{ 'btn--primary': !prefs.hideReasoning }"
        @click="prefs.hideReasoning = !prefs.hideReasoning"
      >
        思考
      </button>
      <button
        class="btn btn--sm"
        :class="{ 'btn--primary': !prefs.hideTools }"
        @click="prefs.hideTools = !prefs.hideTools"
      >
        工具
      </button>
      <button
        class="btn btn--sm"
        :class="{ 'btn--primary': prefs.followStream }"
        @click="prefs.followStream = !prefs.followStream"
      >
        跟随
      </button>
    </div>

    <div class="composer__row">
      <!-- 模式：三档一目了然，比藏进下拉菜单快 -->
      <div class="seg" role="tablist">
        <button
          v-for="item in MODES"
          :key="item.id"
          class="seg__btn"
          :class="[`seg__btn--${item.id}`, { 'seg__btn--on': mode === item.id }]"
          role="tab"
          :aria-selected="mode === item.id"
          :title="item.hint"
          @click="setMode(item.id)"
        >
          <span>{{ item.icon }}</span>
          <span class="seg__label">{{ item.label }}</span>
        </button>
      </div>

      <textarea
        ref="input"
        v-model="draft"
        class="composer__input"
        rows="1"
        :placeholder="placeholder"
        :disabled="blocked"
        @keydown="onKeydown"
        @input="grow"
      />

      <!-- 模型可能很多，用菜单而不是分段控件 -->
      <button class="btn btn--sm composer__model" :title="modelLabel" @click="pickModel">
        {{ modelLabel }}
      </button>

      <button v-if="running" class="composer__orb composer__orb--stop" title="停止生成" @click="stop">
        ■
      </button>
      <button
        v-else
        class="composer__orb"
        :disabled="!canSend || !draft.trim()"
        title="发送"
        @click="submit"
      >
        ↑
      </button>
    </div>
  </div>
</template>

<style scoped>
.composer {
  flex-shrink: 0;
  padding: 10px 24px 18px;
}

.composer__toolbar {
  max-width: 1100px;
  margin: 0 auto 8px;
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}

.composer__row {
  max-width: 1100px;
  margin: 0 auto;
  display: flex;
  /* 底对齐：文本框长高时，两侧的控件跟着贴在底边 */
  align-items: flex-end;
  gap: 10px;
}

.composer__input {
  flex: 1 1 320px;
  min-width: 240px;
  padding: 11px 20px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-pill);
  background: var(--bg-sunken);
  color: var(--text);
  font: inherit;
  font-size: var(--text-md);
  line-height: 1.5;
  resize: none;
  outline: none;
  overflow-y: hidden;
  max-height: 150px;
  box-shadow: var(--shadow-card);
  transition: var(--transition);
  /* 自己长高，内部滚动条藏起来 */
  scrollbar-width: none;
}

.composer__input::-webkit-scrollbar {
  display: none;
}

.composer__input:focus {
  border-color: var(--border-strong);
  box-shadow: var(--shadow-focus);
}

.composer__input::placeholder {
  color: var(--text-faint);
}

.composer__model {
  flex-shrink: 0;
  max-width: 150px;
  margin-bottom: 3px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: block;
}

/* 分段控件 */
.seg {
  flex-shrink: 0;
  display: inline-flex;
  margin-bottom: 3px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: var(--bg-sunken);
}

.seg__btn {
  display: flex;
  align-items: center;
  gap: 4px;
  height: var(--ctl-h-md);
  padding: 0 10px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
  transition: var(--transition);
}

.seg__btn:hover {
  background: var(--surface-hover);
}

/* 三档各自的颜色：能力越宽越显眼，扫一眼就知道现在放开到哪一步 */
.seg__btn--ask.seg__btn--on {
  background: var(--info);
  color: var(--on-accent);
}

.seg__btn--edit.seg__btn--on {
  background: var(--warning);
  color: var(--on-accent);
}

.seg__btn--agent.seg__btn--on {
  background: var(--success);
  color: var(--on-accent);
}

/* 圆形发送键。发送与停止占同一个位置——生成中你唯一想做的就是叫停 */
.composer__orb {
  flex-shrink: 0;
  width: 42px;
  height: 42px;
  border-radius: 50%;
  border: var(--border-width) solid var(--accent);
  background: var(--surface);
  color: var(--accent);
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
  transition: var(--transition);
  box-shadow: var(--shadow-card);
}

.composer__orb:hover:not(:disabled) {
  background: var(--accent);
  color: var(--on-accent);
}

.composer__orb:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.composer__orb--stop {
  border-color: var(--danger);
  color: var(--danger);
  font-size: 13px;
}

.composer__orb--stop:hover {
  background: var(--danger);
  color: var(--on-accent);
}

@media (max-width: 720px) {
  .composer {
    padding: 8px 12px 14px;
  }

  /* 窄屏只留图标，文字挤不下 */
  .seg__label {
    display: none;
  }

  .composer__model {
    max-width: 88px;
  }
}
</style>
