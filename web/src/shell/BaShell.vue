<!--
  蔚蓝档案的外壳：加载页 → 大厅 → 内容。

  # 三层，而不是两层

  另两套外壳只有「落地 + 内容」两层。这套中间多一个**大厅**：加载页只有字标和缓慢
  走动的画面，点字标才进大厅，大厅底部那条导航才通向各个内容页。

  大厅**不是一个 `View`**。共享的 `View` 联合类型描述的是「哪个内容页」，而大厅是这套
  外壳自己的落地形态——写进 `View` 就要让另两套外壳都决定拿一个它们没有的去处怎么办。

  # 大厅的排布照小恋给的示范来

  几处之前照截图猜错、被指出来的：

  - **底栏是一条浮动圆角条**（宽 95vw、离底 16px），不是贴边通栏；它的着色只占**下半
    55%**，所以图标天然露在条外——不是给图标加白底块再往上顶
  - 卡片**左对齐**，不居中
  - 时间钉在条**自己的右下角**，在条里面
  - 左上是**一块实心深蓝面板**，头像跨两行（左头像，右上名字、右下状态），**没有字标**
  - 会话区是右下角一个**大圆**，文字压在圆的下沿

  # 素材从数据目录来

  `~/.lya/theme/ba/home/` 是加载图，`cg/` 是记忆大厅（视频）。不内嵌：CG 一个几十 MB，
  进二进制的话每换一张图都要重新构建。目录空着也能用，界面会告诉用户往哪儿放。
-->

<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'

import {
  archivedSessions,
  client,
  defaultModel,
  running,
  sessions,
} from '../app/useChat'
import BaLogo from '../ui/BaLogo.vue'
import ThemeStage from '../ui/ThemeStage.vue'
import { useThemeStage } from '../ui/useThemeStage'
import { NAV_ICONS } from './icons'
import { NAV_ITEMS, type ShellProps, type View } from './types'

const props = defineProps<ShellProps>()
const emit = defineEmits<{ navigate: [view: View] }>()

/**
 * 落地时停在加载页还是大厅。
 *
 * 只在 `view === 'home'` 时有意义——进了内容页，外壳画的是内容页的框。
 */
const landing = ref<'boot' | 'lobby'>('boot')

const atBoot = computed(() => props.view === 'home' && landing.value === 'boot')
const atLobby = computed(() => props.view === 'home' && landing.value === 'lobby')

/**
 * 加载页自动轮播加载图；记忆大厅**不自动切**，而且记住上次挑的那张——它是挑一张
 * 长期看的东西，不是走马灯。
 */
const boot = useThemeStage({ theme: 'ba', kind: 'home', autoMs: 9000 })
const cg = useThemeStage({ theme: 'ba', kind: 'cg', remember: true })

/** 小恋恋现在在做什么。计数在右上角，这里不重复。 */
const status = computed(() => {
  const model = defaultModel.value?.name ?? '未配置模型'
  return running.value ? `正在输出 · ${model}` : `空闲 · ${model}`
})

/**
 * 记忆条数。
 *
 * 会话列表 `App.vue` 启动时就拉了，记忆没有——外壳自己要一次。失败就留 0，
 * 大厅少一个数字不值得弹错误。
 */
const memoryCount = ref(0)
void client
  .memories()
  .then((list) => (memoryCount.value = list.length))
  .catch(() => {})

/** 右上角三个信息胶囊：大圆图标在左，文字在右。模型名已经在左上的状态里，这里不重复。 */
const badges = computed(() => [
  { key: 'chat', icon: 'chat' as const, text: `会话 ${sessions.value.length}` },
  { key: 'memory', icon: 'memory' as const, text: `记忆 ${memoryCount.value}` },
  { key: 'archive', icon: 'archive' as const, text: `归档 ${archivedSessions.value.length}` },
])

const clock = ref(nowText())
function nowText(): string {
  const now = new Date()
  return `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`
}
const ticker = window.setInterval(() => (clock.value = nowText()), 10_000)
onUnmounted(() => window.clearInterval(ticker))

function enterLobby(): void {
  landing.value = 'lobby'
}

/** 底栏第一格：回加载页。 */
function backToBoot(): void {
  landing.value = 'boot'
}

/** 从内容页回落地：回大厅而不是加载页——加载页是「刚打开」才该看到的。 */
function backToLanding(): void {
  landing.value = 'lobby'
  emit('navigate', 'home')
}
</script>

<template>
  <div class="ba">
    <!--
      两个背景层挂在 v-if 之外，靠 v-show 控制可见。
      放进 v-if 里的话，去内容页再回来会把 <video> 整个销毁重建——几十 MB 从头下载，
      画面先黑一下再慢慢出来。这就是「从其他页面返回时 cg 出问题」的原因。
      不可见时暂停解码，见 ThemeStage 的 active。
    -->
    <div v-show="atBoot" class="ba__layer" data-layer="boot">
      <ThemeStage
        :items="boot.items.value"
        :index="boot.index.value"
        :measure="boot.measure"
        :active="atBoot"
      />
    </div>
    <div v-show="atLobby" class="ba__layer" data-layer="cg">
      <ThemeStage
        :items="cg.items.value"
        :index="cg.index.value"
        :measure="cg.measure"
        :active="atLobby"
      />
    </div>

    <!-- ── 加载页 ────────────────────────────────── -->
    <template v-if="atBoot">
      <div class="ba__boot">
        <button class="ba__boot-brand" type="button" @click="enterLobby">
          <BaLogo class="ba__logo--big" left="lya" right="Archive" />
        </button>

        <!-- 只在真的没素材时说一句：有图的时候画面自己会说话，不需要提示 -->
        <p v-if="!boot.items.value.length && !boot.loading.value" class="ba__empty">
          把加载图放进 <code>{{ boot.dir.value }}</code>，点字标进入大厅
        </p>
      </div>
    </template>

    <!-- ── 大厅 ──────────────────────────────────── -->
    <template v-else-if="atLobby">
      <!-- 左上：一块实心面板，头像跨两行 -->
      <div class="ba__account">
        <div class="ba__account-grid">
          <span class="ba__face">
            <img src="/icon.png" alt="" @error="($event.target as HTMLElement).style.visibility = 'hidden'" />
          </span>
          <p class="ba__account-name">小恋恋</p>
          <p class="ba__account-state">
            <i class="ba__lamp" :class="{ 'ba__lamp--busy': running }" />{{ status }}
          </p>
        </div>
      </div>

      <!-- 右上：大圆图标在左，文字在右 -->
      <div class="ba__badges">
        <div v-for="badge in badges" :key="badge.key" class="ba__badge">
          <span class="ba__badge-icon" v-html="NAV_ICONS[badge.icon]" />
          <p>{{ badge.text }}</p>
        </div>
      </div>

      <!-- 左右翻页：轻微上下浮动 -->
      <template v-if="cg.many.value">
        <button class="ba__side ba__side--prev" type="button" aria-label="上一张" @click="cg.go(-1)" />
        <button class="ba__side ba__side--next" type="button" aria-label="下一张" @click="cg.go(1)" />
      </template>

      <p v-if="!cg.items.value.length && !cg.loading.value" class="ba__cg-empty">
        记忆大厅放进 <code>{{ cg.dir.value }}</code>
      </p>

      <!-- 右下：大圆 + 压在下沿的文字 -->
      <button class="ba__session" type="button" @click="emit('navigate', 'sessions')">
        <span class="ba__session-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
               stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M21 11.5a8.38 8.38 0 01-.9 3.8 8.5 8.5 0 01-7.6 4.7 8.38 8.38 0 01-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 01-.9-3.8 8.5 8.5 0 014.7-7.6 8.38 8.38 0 013.8-.9h.5a8.48 8.48 0 018 8v.5z" />
            <path d="M8.5 10.5h7M8.5 14h4.5" />
          </svg>
        </span>
        <p>会话区</p>
      </button>

      <!-- 底栏：浮动圆角条，着色只占下半，卡片左对齐 -->
      <nav class="ba__dock">
        <button class="ba__card" type="button" @click="backToBoot">
          <span class="ba__card-icon" v-html="NAV_ICONS.home" />
          <p>主页</p>
        </button>
        <button
          v-for="item in NAV_ITEMS"
          :key="item.view"
          class="ba__card"
          type="button"
          @click="emit('navigate', item.view)"
        >
          <span class="ba__card-icon" v-html="NAV_ICONS[item.icon]" />
          <p>{{ item.label }}</p>
        </button>
        <span class="ba__time">{{ clock }}</span>
      </nav>
    </template>

    <!-- ── 内容页 ────────────────────────────────── -->
    <template v-else>
      <header class="ba__bar">
        <button class="ba__back" type="button" @click="backToLanding">‹ 大厅</button>
        <BaLogo class="ba__logo--sm" left="lya" right="Archive" />
      </header>
      <main class="ba__content">
        <slot />
      </main>
    </template>
  </div>
</template>

<style scoped>
/* 背景层：铺满、压在所有浮层下面 */
.ba__layer {
  position: absolute;
  inset: 0;
  z-index: 0;
}

.ba {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: var(--bg);
}

/* ── 加载页 ───────────────────────────────────── */

.ba__boot {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 20px 24px;
}

/* 字标本身就是按钮，不要 hover 底块——它是一块招牌，不是一个控件 */
.ba__boot-brand {
  align-self: flex-start;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.ba__logo--big {
  font-size: clamp(28px, 4.2vw, 40px);
}

.ba__logo--sm {
  font-size: 20px;
}

/* ── 左上：账号面板 ───────────────────────────── */

.ba__account {
  position: absolute;
  top: 20px;
  left: 24px;
  z-index: 2;
  padding: 16px 28px 16px 16px;
  min-width: 260px;
  border-radius: 24px;
}

/* 头像占左边一整列，名字与状态各占右边一行 */
.ba__account-grid {
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-areas:
    'avatar name'
    'avatar status';
  align-items: center;
  column-gap: 16px;
  row-gap: 4px;
}

.ba__account-name {
  grid-area: name;
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  line-height: 1.2;
}

.ba__account-state {
  grid-area: status;
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: 13px;
  line-height: 1.2;
}

.ba__lamp {
  width: 7px;
  height: 7px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--success);
}

.ba__lamp--busy {
  animation: ba-blink 1.3s ease infinite;
}

@keyframes ba-blink {
  50% {
    opacity: 0.35;
  }
}

.ba__face {
  grid-area: avatar;
  display: block;
  width: 56px;
  height: 56px;
}

.ba__face img {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  background: var(--bg-sunken);
}

/* ── 右上：信息胶囊 ───────────────────────────── */

.ba__badges {
  position: absolute;
  top: 20px;
  right: 24px;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 10px;
}

.ba__badge {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 18px 8px 8px;
  border-radius: 999px;
}

.ba__badge-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border-radius: 50%;
}

.ba__badge-icon :deep(svg) {
  width: 19px;
  height: 19px;
}

.ba__badge p {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  line-height: 1.2;
  white-space: nowrap;
}

/* ── 左右翻页箭头 ─────────────────────────────── */

/* 图形用 SVG 背景，颜色在 themes/ba.css 里换（data URI 里的色值改不动） */
.ba__side {
  position: fixed;
  top: 50%;
  z-index: 3;
  width: 32px;
  height: 64px;
  border: none;
  background-color: transparent;
  background-repeat: no-repeat;
  background-position: center;
  background-size: contain;
  opacity: 0.55;
  cursor: pointer;
  animation: ba-side-float 3.2s ease-in-out infinite;
}

.ba__side:hover {
  opacity: 0.9;
}

.ba__side--prev {
  left: 16px;
  transform: translateY(-50%);
}

.ba__side--next {
  right: 16px;
  transform: translateY(-50%);
  animation-delay: 0.6s;
}

@keyframes ba-side-float {
  0%,
  100% {
    transform: translateY(-50%);
  }
  50% {
    transform: translateY(calc(-50% - 8px));
  }
}

/* ── 右下：会话区大圆 ─────────────────────────── */

.ba__session {
  position: absolute;
  right: calc(2.5vw + 8px);
  bottom: 132px;
  z-index: 2;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.ba__session-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 88px;
  height: 88px;
  border-radius: 50%;
  transition: transform var(--duration-fast) ease;
}

.ba__session-icon svg {
  width: 40px;
  height: 40px;
}

.ba__session:hover .ba__session-icon {
  transform: scale(1.05);
}

/* 标签是一块独立的牌，不是压在圆上的字——压上去要靠投影才看得清，那就显得脏 */
.ba__session p {
  position: relative;
  margin: -8px 0 0;
  padding: 3px 14px;
  border-radius: var(--radius-pill);
  width: max-content;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 700;
  line-height: 1.4;
}

/* ── 底栏 ─────────────────────────────────────── */

/*
  一条浮动的圆角条：宽 95vw、离底 16px。着色由 ::before 铺**下半 55%**，所以卡片的
  圆形图标天然露在条外——这是那条栏的关键，给图标加白底块再往上顶是另一回事。
*/
.ba__dock {
  position: absolute;
  left: 50%;
  bottom: 16px;
  transform: translateX(-50%);
  width: 95vw;
  padding: 24px 24px 16px 40px;
  border-radius: 24px;
  display: flex;
  justify-content: flex-start;
  align-items: center;
  gap: 28px;
  overflow: hidden;
}

.ba__dock::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 55%;
  border-radius: 24px;
  z-index: 0;
}

.ba__dock > * {
  position: relative;
  z-index: 1;
}

.ba__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.ba__card-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  transition: transform var(--duration-fast) ease;
}

.ba__card-icon :deep(svg) {
  width: 24px;
  height: 24px;
}

.ba__card:hover .ba__card-icon {
  transform: translateY(-4px);
}

.ba__card p {
  margin: 0;
  font-size: 14px;
}

/* 时间钉在条自己的右下角 */
.ba__time {
  position: absolute;
  right: 24px;
  bottom: 12px;
  z-index: 1;
  font-size: 18px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.05em;
}

.ba__empty,
.ba__cg-empty {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  margin: 0;
  padding: 9px 16px;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  backdrop-filter: blur(4px);
}

.ba__empty code,
.ba__cg-empty code {
  font-family: var(--font-mono);
}

/* ── 内容页 ───────────────────────────────────── */

.ba__bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
  padding: 6px 14px;
  background: var(--surface);
  border-bottom: var(--border-width) solid var(--border);
}

.ba__back {
  padding: 0 var(--ctl-pad-x-md);
  height: var(--ctl-h-md);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-weight: 600;
  cursor: pointer;
}

.ba__back:hover {
  background: var(--surface-hover);
  color: var(--accent);
}

.ba__content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

@media (max-width: 720px) {
  .ba__dock {
    gap: 16px;
    padding: 20px 16px 14px 20px;
    overflow-x: auto;
  }

  .ba__time,
  .ba__session {
    display: none;
  }
}
</style>
