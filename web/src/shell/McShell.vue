<!--
  Minecraft 的外壳：仿游戏主菜单。

  这份就是「外壳可换」的意义所在——它和侧边栏是**两种排版**，不是同一套布局
  换个配色能得到的。落地页是标题加一列大按钮，选了之后才进内容，内容区顶上
  留一条返回栏。

  它仍然只管导航：内容从插槽来，所以聊天那套逻辑不会被复制到这里。
-->

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

import type { Memory } from '../api/client'
import {
  archivedSessions,
  client,
  createSession,
  models,
  sessions,
} from '../app/useChat'
import {
  buildSplashLines,
  menuFootLeft,
  menuFootRight,
  pickSplash,
} from './mcMenuSplash'
import type { ShellProps, View } from './types'

defineProps<ShellProps>()
const emit = defineEmits<{ navigate: [view: View] }>()

const memories = ref<Memory[]>([])
const splash = ref('Also try 新的对话！')
let splashTimer: ReturnType<typeof setInterval> | undefined

const splashLines = computed(() =>
  buildSplashLines(sessions.value, archivedSessions.value, memories.value, models.value),
)

const footLeft = computed(() =>
  menuFootLeft(sessions.value, archivedSessions.value, memories.value),
)

const footRight = computed(() => menuFootRight(models.value))

const WHERE: Partial<Record<View, string>> = {
  chat: '对话',
  sessions: '对话列表',
  memory: '记忆',
  tools: '工具',
  models: '模型',
  theme: '外观',
  config: '设置',
  settings: '会话设置',
}

function rollSplash(): void {
  splash.value = pickSplash(splashLines.value)
}

onMounted(async () => {
  try {
    memories.value = await client.memories()
  } catch {
    memories.value = []
  }
  rollSplash()
  splashTimer = setInterval(rollSplash, 10_000)
})

onUnmounted(() => {
  if (splashTimer) clearInterval(splashTimer)
})

async function startNewChat(): Promise<void> {
  await createSession()
  emit('navigate', 'chat')
}

function go(view: View): void {
  emit('navigate', view)
}
</script>

<template>
  <div class="mc-shell">
    <!-- 标题画面 -->
    <div v-if="view === 'home'" class="mc-shell__home">
      <div class="mc-shell__panorama" aria-hidden="true" />
      <div class="mc-shell__vignette" aria-hidden="true" />

      <header class="mc-shell__top">
        <div class="mc-shell__hero">
          <h1 class="mc-shell__logo">lya</h1>
          <p class="mc-shell__edition">Minecraft</p>
        </div>
        <p class="mc-shell__splash" aria-live="polite">{{ splash }}</p>
      </header>

      <div class="mc-shell__menu-wrap">
        <div class="mc-shell__menu">
          <button class="btn btn--lg mc-shell__entry mc-shell__entry--wide" @click="startNewChat">
            新的对话
          </button>
          <button class="btn btn--lg mc-shell__entry mc-shell__entry--wide" @click="go('sessions')">
            对话列表
          </button>
          <button class="btn btn--lg mc-shell__entry" @click="go('memory')">记忆</button>
          <button class="btn btn--lg mc-shell__entry" @click="go('tools')">工具</button>
          <button class="btn btn--lg mc-shell__entry" @click="go('models')">模型</button>
          <button class="btn btn--lg mc-shell__entry" @click="go('config')">设置</button>
          <button class="btn btn--lg mc-shell__entry mc-shell__entry--wide" @click="go('theme')">
            外观
          </button>
        </div>
      </div>

      <footer class="mc-shell__foot">
        <span class="mc-shell__foot-left">{{ footLeft }}</span>
        <span class="mc-shell__foot-right">{{ footRight }}</span>
      </footer>
    </div>

    <!-- 内容页 -->
    <template v-else>
      <header class="mc-shell__bar">
        <button class="btn" @click="go('home')">‹ 返回</button>
        <span class="mc-shell__where">{{ WHERE[view] ?? view }}</span>
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
  position: relative;
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 0 20px;
  overflow: hidden;
}

.mc-shell__panorama {
  position: absolute;
  inset: -12%;
  filter: blur(10px) saturate(1.15);
  transform: scale(1.08);
  pointer-events: none;
}

.mc-shell__vignette {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse at center, transparent 35%, rgba(0, 0, 0, 0.55) 100%),
    linear-gradient(180deg, rgba(0, 0, 0, 0.35), rgba(0, 0, 0, 0.2) 40%, rgba(0, 0, 0, 0.45));
  pointer-events: none;
}

.mc-shell__top {
  position: relative;
  z-index: 1;
  flex-shrink: 0;
  width: min(680px, 100%);
  margin: 0 auto;
  padding-top: clamp(40px, 9vh, 88px);
  text-align: center;
}

.mc-shell__hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  margin-top: 12px;
  user-select: none;
}

.mc-shell__logo {
  margin: 0;
  font-size: 108px;
  line-height: 1;
  letter-spacing: 8px;
}

.mc-shell__edition {
  margin: 0;
  font-size: 12px;
  letter-spacing: 3px;
  text-shadow: var(--text-shadow);
}

/* 仿 MC 黄字 splash：斜挂在标题右侧 */
.mc-shell__splash {
  position: absolute;
  left: 58%;
  top: 72%;
  margin: 0;
  max-width: 220px;
  font-size: 12px;
  line-height: 1.4;
  animation: lya-mc-splash-bounce 850ms ease-in-out infinite;
  pointer-events: none;
}

.mc-shell__menu-wrap {
  position: relative;
  z-index: 1;
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  min-height: 0;
}

.mc-shell__menu {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  width: min(480px, 100%);
}

.mc-shell__entry {
  width: 100%;
  min-height: var(--ctl-h-lg);
}

.mc-shell__entry--wide {
  grid-column: 1 / -1;
}

.mc-shell__foot {
  position: relative;
  z-index: 1;
  flex-shrink: 0;
  width: 100%;
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 4px 16px;
  color: var(--text-faint);
  font-size: 12px;
  line-height: 1.5;
}

.mc-shell__foot-right {
  text-align: right;
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

@media (max-width: 520px) {
  .mc-shell__logo {
    font-size: 84px;
    letter-spacing: 5px;
  }

  .mc-shell__splash {
    position: static;
    margin: 14px auto 0;
    animation: lya-mc-splash-bounce-mobile 850ms ease-in-out infinite;
  }

  .mc-shell__foot {
    flex-direction: column;
    align-items: flex-start;
  }

  .mc-shell__foot-right {
    text-align: left;
  }

  @media (prefers-reduced-motion: reduce) {
    .mc-shell__splash {
      animation: none;
      transform: rotate(-12deg);
    }
  }
}

@media (prefers-reduced-motion: reduce) {
  .mc-shell__splash {
    animation: none;
    transform: rotate(-18deg);
  }
}
</style>
