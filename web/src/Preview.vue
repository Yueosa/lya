<!--
  原件预览页。

  两套主题并排看得见，才知道 token 层是不是真的够用——只做一套的话，很容易
  写出一堆看着像 token、实则绑死在那套风格上的变量。等真正的界面搭起来之后，
  这一页仍然留着当回归检查用。
-->

<script setup lang="ts">
import { ref } from 'vue'

import { applyTheme, currentTheme, THEMES } from './themes'
import UiHost from './ui/UiHost.vue'
import { openContextMenu } from './ui/useContextMenu'
import { confirm, confirmAsync, prompt } from './ui/useDialog'
import { toast } from './ui/useToast'

const theme = ref(currentTheme())
const lastResult = ref('（还没操作）')

function switchTheme(id: string): void {
  theme.value = id
  applyTheme(id)
}

async function tryConfirm(danger: boolean): Promise<void> {
  const ok = await confirm({
    title: danger ? '删除这个会话？' : '要继续吗？',
    message: danger ? '删掉之后没法恢复。' : '这是一次普通确认。',
    danger,
  })
  lastResult.value = ok ? '你点了确认' : '你取消了'
}

async function tryPrompt(): Promise<void> {
  const name = await prompt({ title: '给会话改个名', initial: '未命名会话' })
  lastResult.value = name === null ? '你取消了改名' : `新名字：${name}`
}

async function trySlowWork(): Promise<void> {
  let first = true
  const done = await confirmAsync({
    title: '清空全部记忆',
    message: '第一次会故意失败，好看看错误是怎么显示的。',
    danger: true,
    run: async () => {
      await new Promise((resolve) => setTimeout(resolve, 800))
      if (first) {
        first = false
        throw new Error('数据库正忙，请重试')
      }
    },
  })
  lastResult.value = done ? '清空完成' : '你放弃了'
}

function onContextMenu(event: MouseEvent): void {
  openContextMenu(event, [
    { label: '重新生成', icon: '↻', onSelect: () => toast('重新生成', 'info') },
    { label: '复制', icon: '⧉', onSelect: () => toast('已复制', 'success') },
    { label: '暂时不可用', icon: '⋯', disabled: true, onSelect: () => {} },
    { separator: true },
    { label: '删除', icon: '✕', danger: true, onSelect: () => void tryConfirm(true) },
  ])
}
</script>

<template>
  <div class="preview">
    <header class="preview__bar panel">
      <strong>主题</strong>
      <button
        v-for="item in THEMES"
        :key="item.id"
        class="btn"
        :class="{ 'btn--primary': theme === item.id }"
        @click="switchTheme(item.id)"
      >
        {{ item.label }}
      </button>
    </header>

    <section class="panel preview__card">
      <h3>按钮</h3>
      <div class="preview__row">
        <button class="btn">普通</button>
        <button class="btn btn--primary">主要</button>
        <button class="btn btn--danger">危险</button>
        <button class="btn btn--ghost">幽灵</button>
        <button class="btn" disabled>禁用</button>
      </div>
      <div class="preview__row">
        <button class="btn btn--sm">小</button>
        <button class="btn">中</button>
        <button class="btn btn--lg">大</button>
      </div>
    </section>

    <section class="panel preview__card">
      <h3>输入</h3>
      <input class="input" placeholder="在这里输入…" />
    </section>

    <section class="panel preview__card">
      <h3>浮层</h3>
      <div class="preview__row">
        <button class="btn" @click="tryConfirm(false)">确认框</button>
        <button class="btn btn--danger" @click="tryConfirm(true)">危险确认</button>
        <button class="btn" @click="tryPrompt">输入框</button>
        <button class="btn" @click="trySlowWork">异步确认</button>
      </div>
      <div class="preview__row">
        <button class="btn" @click="toast('保存成功', 'success')">成功提示</button>
        <button class="btn" @click="toast('连接失败：HTTP 401', 'error')">错误提示</button>
      </div>
      <p class="preview__hint" @contextmenu="onContextMenu">
        在这行字上点右键试试菜单（贴到窗口右下角也不会跑出去）
      </p>
      <p class="preview__result">{{ lastResult }}</p>
    </section>

    <section class="panel preview__card">
      <h3>气泡与代码</h3>
      <div class="preview__bubbles">
        <div class="bubble bubble--user">这是我说的话</div>
        <div class="bubble bubble--assistant">
          这是回复。注意气泡尾巴的形状在两套主题下不一样。
        </div>
      </div>
      <pre class="hljs preview__code"><code><span class="hljs-keyword">fn</span> <span class="hljs-function">main</span>() {
    <span class="hljs-comment">// 代码配色跟着主题走</span>
    <span class="hljs-built_in">println!</span>(<span class="hljs-string">"你好"</span>, <span class="hljs-number">42</span>);
}</code></pre>
    </section>

    <UiHost />
  </div>
</template>

<style scoped>
.preview {
  max-width: 720px;
  margin: 0 auto;
  padding: 24px 16px 64px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.preview__bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  position: sticky;
  top: 0;
  z-index: 10;
}

.preview__card {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.preview__card h3 {
  margin: 0;
  font-size: var(--text-md);
  color: var(--text-muted);
}

.preview__row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.preview__hint {
  margin: 0;
  padding: 10px;
  border: var(--border-width) dashed var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.preview__result {
  margin: 0;
  color: var(--text-faint);
  font-size: var(--text-sm);
}

.preview__bubbles {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 气泡形状全靠 token：东京夜是 14px 圆角配 4px 尖尾巴，
   MTF 把尾巴设成和圆角一样大，也就没有尾巴了 */
.bubble {
  max-width: 76%;
  padding: 9px 13px;
  border-radius: var(--bubble-radius);
  font-size: var(--text-md);
}

.bubble--user {
  align-self: flex-end;
  background: var(--accent);
  color: var(--on-accent);
  border-bottom-right-radius: var(--bubble-tail-radius);
}

.bubble--assistant {
  align-self: flex-start;
  background: var(--surface-hover);
  border: var(--border-width) solid var(--border);
  border-bottom-left-radius: var(--bubble-tail-radius);
}

.preview__code {
  margin: 0;
  padding: 12px;
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  overflow-x: auto;
}
</style>
