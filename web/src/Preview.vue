<!--
  原件预览页（`#preview`）。

  和外观页里的 `ThemePreview` 分工：**原件清单只有一份**，就是 `ThemePreview`，两边
  都用它。这一页额外提供两样它给不了的东西——

  1. **浮层**：确认框、输入框、toast、右键菜单都要点一下才看得见，而外观页里的预览是
     个不能交互的岛。
  2. **整屏外壳**：换主题时排版会整个换掉（MC 是主菜单，其余是侧栏），这得占满窗口
     才看得出来。

  以前这一页自己抄了一套按钮、输入、气泡和代码块，和 `ThemePreview` 各写各的，两边
  都不全也不一致。抄的那份已经删掉了。
-->

<script setup lang="ts">
import { computed, ref } from 'vue'

import { shellFor } from './shell/registry'
import type { View } from './shell/types'
import { applyTheme, currentTheme, THEMES } from './themes'
import ThemePreview from './ui/ThemePreview.vue'
import UiHost from './ui/UiHost.vue'
import { openContextMenu } from './ui/useContextMenu'
import { confirm, confirmAsync, prompt } from './ui/useDialog'
import { toast } from './ui/useToast'

const theme = ref(currentTheme())
const lastResult = ref('（还没操作）')
/** 看原件还是看外壳。 */
const tab = ref<'parts' | 'shell'>('parts')
const view = ref<View>('home')

const shell = computed(() => shellFor(theme.value))

function switchTheme(id: string): void {
  theme.value = id
  applyTheme(id)
  // 换主题时回到落地页，否则从 MC 的内容页切到侧边栏外壳会看着很怪
  view.value = 'home'
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
    { label: '重新生成', icon: 'refresh', onSelect: () => toast('重新生成', 'info') },
    { label: '复制', icon: 'copy', onSelect: () => toast('已复制', 'success') },
    { label: '暂时不可用', icon: 'info', disabled: true, onSelect: () => {} },
    { separator: true },
    { label: '删除', icon: 'delete', danger: true, onSelect: () => void tryConfirm(true) },
  ])
}
</script>

<template>
  <!-- 外壳页：整屏交给主题自己的排版 -->
  <div v-if="tab === 'shell'" class="preview-shell">
    <component :is="shell" :view="view" @navigate="(next: View) => (view = next)">
      <div class="preview-shell__content">
        <p>这里是内容区。</p>
        <p class="preview__hint">
          外壳只管导航与排版，内容从插槽来——所以聊天那套逻辑只有一份实现，
          不会被复制到三套外壳里。
        </p>
      </div>
    </component>
    <div class="preview-shell__switch panel">
      <button
        v-for="item in THEMES"
        :key="item.id"
        class="btn btn--sm"
        :class="{ 'btn--primary': theme === item.id }"
        @click="switchTheme(item.id)"
      >
        {{ item.label }}
      </button>
      <button class="btn btn--sm" @click="tab = 'parts'">← 看原件</button>
    </div>
    <UiHost />
  </div>

  <div v-else class="preview">
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
      <button class="btn" @click="tab = 'shell'">看外壳 →</button>
    </header>

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

    <!-- 原件清单只有一份，和外观页共用 -->
    <ThemePreview :theme-id="theme" />

    <UiHost />
  </div>
</template>

<style scoped>
.preview-shell {
  height: 100vh;
  position: relative;
}

.preview-shell__content {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* 悬在角上的切换条，不占外壳自己的版面 */
.preview-shell__switch {
  position: fixed;
  right: 16px;
  bottom: 16px;
  z-index: 50;
  display: flex;
  gap: 6px;
  padding: 8px;
}

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
</style>
