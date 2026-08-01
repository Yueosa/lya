<!--
  方块世界的外壳：仿游戏主菜单。

  这份就是「外壳可换」的意义所在——它和侧边栏是**两种排版**，不是同一套布局
  换个配色能得到的。落地页是标题加一列大按钮，选了之后才进内容，内容区顶上
  留一条返回栏。

  它仍然只管导航：内容从插槽来，所以聊天那套逻辑不会被复制到这里。
-->

<script setup lang="ts">
import { NAV_ITEMS, type ShellProps, type View } from './types'

defineProps<ShellProps>()
defineEmits<{ navigate: [view: View] }>()
</script>

<template>
  <div class="mc-shell">
    <!-- 标题画面 -->
    <div v-if="view === 'home'" class="mc-shell__home">
      <h1 class="mc-shell__title">lya</h1>
      <p class="mc-shell__sub">方块世界</p>
      <div class="mc-shell__menu">
        <button
          v-for="item in NAV_ITEMS"
          :key="item.view"
          class="btn btn--lg mc-shell__entry"
          @click="$emit('navigate', item.view)"
        >
          {{ item.label }}
        </button>
      </div>
    </div>

    <!-- 内容页 -->
    <template v-else>
      <header class="mc-shell__bar">
        <button class="btn" @click="$emit('navigate', 'home')">‹ 返回</button>
        <span class="mc-shell__where">
          {{ NAV_ITEMS.find((item) => item.view === view)?.label }}
        </span>
      </header>
      <main class="mc-shell__body">
        <slot />
      </main>
    </template>
  </div>
</template>

<style scoped>
.mc-shell {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.mc-shell__home {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  /* 主菜单铺满，像开游戏一样 */
  padding: 32px;
}

.mc-shell__title {
  margin: 0;
  font-size: 56px;
  letter-spacing: 4px;
  /* 硬投影，像素风不用模糊 */
  text-shadow: 4px 4px 0 var(--border-strong);
  color: var(--surface);
}

.mc-shell__sub {
  margin: 0 0 28px;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.mc-shell__menu {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: min(420px, 100%);
}

.mc-shell__entry {
  width: 100%;
}

.mc-shell__bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: var(--bg-sunken);
  border-bottom: var(--border-width) solid var(--border);
}

.mc-shell__where {
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.mc-shell__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
</style>
