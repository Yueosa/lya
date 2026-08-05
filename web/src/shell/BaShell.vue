<!--
  蔚蓝档案的外壳：加载页 → 大厅 → 内容。

  # 三层，而不是两层

  另两套外壳只有「落地 + 内容」两层。这套中间多一个**大厅**：加载页只有字标和缓慢
  走动的画面，点字标才进大厅，大厅底部那条导航才通向各个内容页。

  大厅**不是一个 `View`**。共享的 `View` 联合类型描述的是「哪个内容页」，而大厅是这套
  外壳自己的落地形态——写进 `View` 就要让另两套外壳都决定拿一个它们没有的去处怎么办。
  代价是刷新之后回到加载页而不是大厅，位置记忆只记 `View`。

  # 素材从数据目录来

  `~/.lya/theme/ba/home/` 是加载图，`cg/` 是记忆大厅（视频）。不内嵌：CG 一个几十 MB，
  进二进制的话每换一张图都要重新构建。目录空着也能用，界面会告诉用户往哪儿放。
-->

<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'

import {
  archivedSessions,
  createSession,
  defaultModel,
  openSession,
  running,
  sessions,
} from '../app/useChat'
import BaLogo from '../ui/BaLogo.vue'
import Icon from '../ui/Icon.vue'
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

/** 加载页自动轮播加载图；大厅里的记忆大厅由左右箭头手动切。 */
const boot = useThemeStage({ theme: 'ba', kind: 'home', autoMs: 9000 })
const cg = useThemeStage({ theme: 'ba', kind: 'cg' })

/** 小恋恋现在在做什么。计数在右上角，这里不重复。 */
const status = computed(() => {
  const model = defaultModel.value?.name ?? '未配置模型'
  return running.value ? `正在输出 · ${model}` : `空闲 · ${model}`
})

/** 右上角的资源条。和游戏里三个货币同一个位置、同一种排法。 */
const resources = computed(() => [
  { key: 'chat', icon: 'chat' as const, label: '会话', count: sessions.value.length },
  { key: 'memory', icon: 'memory' as const, label: '记忆', count: memoryCount.value },
  { key: 'archive', icon: 'archive' as const, label: '归档', count: archivedSessions.value.length },
])

/** 记忆条数：外壳不该为它单独拉接口，用得到的地方自己填。 */
const memoryCount = ref(0)

const clock = ref(nowText())
function nowText(): string {
  const now = new Date()
  return `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`
}
const ticker = window.setInterval(() => (clock.value = nowText()), 10_000)
onUnmounted(() => window.clearInterval(ticker))

/** 最近一个会话，大厅右侧「继续上次」用。 */
const recent = computed(() => sessions.value[0] ?? null)

function enterLobby(): void {
  landing.value = 'lobby'
}

function backToBoot(): void {
  landing.value = 'boot'
}

/** 从内容页回落地：回大厅而不是加载页——加载页是「刚打开」才该看到的。 */
function backToLanding(): void {
  landing.value = 'lobby'
  emit('navigate', 'home')
}

async function open(id: string): Promise<void> {
  await openSession(id)
  emit('navigate', 'chat')
}

async function start(): Promise<void> {
  await createSession()
  emit('navigate', 'chat')
}
</script>

<template>
  <div class="ba">
    <!-- ── 加载页 ────────────────────────────────── -->
    <template v-if="atBoot">
      <ThemeStage :items="boot.items.value" :index="boot.index.value" :measure="boot.measure" />

      <div class="ba__boot">
        <button class="ba__brand" type="button" v-tip="'进入大厅'" @click="enterLobby">
          <BaLogo class="ba__logo ba__logo--big" left="lya" right="Archive" />
        </button>

        <div class="ba__boot-foot">
          <p class="ba__tip">
            <template v-if="boot.items.value.length">点字标进入大厅</template>
            <template v-else>
              把加载图放进 <code>{{ boot.dir.value }}</code>，点字标进入大厅
            </template>
          </p>
          <div v-if="boot.many.value" class="ba__dots">
            <i
              v-for="(item, at) in boot.items.value"
              :key="item.name"
              class="ba__dot"
              :class="{ 'ba__dot--on': at === boot.index.value }"
            />
          </div>
        </div>
      </div>
    </template>

    <!-- ── 大厅 ──────────────────────────────────── -->
    <template v-else-if="atLobby">
      <ThemeStage :items="cg.items.value" :index="cg.index.value" :measure="cg.measure" />

      <div class="ba__tl">
        <button class="ba__brand" type="button" v-tip="'回加载页'" @click="backToBoot">
          <BaLogo class="ba__logo" left="lya" right="Archive" />
        </button>

        <div class="ba__who">
          <span class="ba__face">
            <img src="/icon.png" alt="" @error="($event.target as HTMLElement).style.visibility = 'hidden'" />
          </span>
          <span class="ba__who-text">
            <span class="ba__who-name">小恋恋</span>
            <span class="ba__who-state">
              <i class="ba__lamp" :class="{ 'ba__lamp--busy': running }" />
              {{ status }}
            </span>
          </span>
        </div>
      </div>

      <div class="ba__tr">
        <span v-for="res in resources" :key="res.key" class="ba__res" v-tip="res.label">
          <span class="ba__res-icon" v-html="NAV_ICONS[res.icon]" />
          <span class="ba__res-num">{{ res.count }}</span>
        </span>
      </div>

      <template v-if="cg.many.value">
        <button class="ba__arrow ba__arrow--prev" type="button" v-tip="'上一张'" @click="cg.go(-1)">
          <Icon name="chevronLeft" size="sm" />
        </button>
        <button class="ba__arrow ba__arrow--next" type="button" v-tip="'下一张'" @click="cg.go(1)">
          <Icon name="chevronRight" size="sm" />
        </button>
      </template>

      <div v-if="recent" class="ba__guide">
        <button class="ba__guide-btn" type="button" @click="open(recent.id)">
          <span class="ba__face ba__face--sm">
            <img src="/icon.png" alt="" @error="($event.target as HTMLElement).style.visibility = 'hidden'" />
          </span>
          <span class="ba__guide-text">
            <span class="ba__guide-tag">继续上次</span>
            <span class="ba__guide-title">{{ recent.title || '未命名' }}</span>
          </span>
        </button>
      </div>

      <p v-if="!cg.items.value.length && !cg.loading.value" class="ba__cg-empty">
        记忆大厅放进 <code>{{ cg.dir.value }}</code>
      </p>

      <!-- 底部横栏：图标浮在上沿之外，栏内只放文字 -->
      <nav class="ba__dock">
        <div class="ba__dock-bar" />
        <div class="ba__tabs">
          <button
            v-for="item in NAV_ITEMS"
            :key="item.view"
            class="ba__tab"
            type="button"
            @click="emit('navigate', item.view)"
          >
            <span class="ba__tab-glyph" v-html="NAV_ICONS[item.icon]" />
            <span class="ba__tab-label">{{ item.label }}</span>
          </button>
        </div>
        <div class="ba__dock-right">
          <span class="ba__clock">{{ clock }}</span>
          <button class="ba__biz" type="button" @click="emit('navigate', 'sessions')">
            <span class="ba__biz-icon" v-html="NAV_ICONS.chat" />
            会话列表
          </button>
        </div>
        <div class="ba__dock-left">
          <button class="ba__mini" type="button" v-tip="'新对话'" @click="start">
            <Icon name="plus" size="sm" />
          </button>
        </div>
      </nav>
    </template>

    <!-- ── 内容页 ────────────────────────────────── -->
    <template v-else>
      <header class="ba__bar">
        <button class="ba__back" type="button" @click="backToLanding">‹ 大厅</button>
        <button class="ba__brand ba__brand--bar" type="button" v-tip="'回加载页'" @click="emit('navigate', 'home')">
          <BaLogo class="ba__logo ba__logo--sm" left="lya" right="Archive" />
        </button>
      </header>
      <main class="ba__content">
        <slot />
      </main>
    </template>
  </div>
</template>

<style scoped>
.ba {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: var(--bg);
}

/* ── 字标 ─────────────────────────────────────── */

.ba__brand {
  display: inline-block;
  padding: 6px 10px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  cursor: pointer;
  transition: background var(--transition);
}

.ba__brand:hover {
  background: rgba(255, 255, 255, 0.22);
}

.ba__brand--bar:hover {
  background: var(--surface-hover);
}

.ba__logo {
  font-size: 30px;
}

.ba__logo--big {
  font-size: clamp(38px, 7vw, 62px);
}

.ba__logo--sm {
  font-size: 20px;
}

/* ── 加载页 ───────────────────────────────────── */

.ba__boot {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 20px 24px 28px;
}

.ba__boot > .ba__brand {
  align-self: flex-start;
}

.ba__boot-foot {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.ba__tip {
  margin: 0;
  padding: 7px 15px;
  border-radius: var(--radius-pill);
  background: rgba(30, 54, 82, 0.55);
  color: var(--on-accent);
  font-size: var(--text-xs);
  backdrop-filter: blur(4px);
}

.ba__tip code {
  font-family: var(--font-mono);
  opacity: 0.9;
}

.ba__dots {
  display: flex;
  gap: 6px;
  align-items: center;
}

.ba__dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.5);
  transition: background var(--transition), width var(--transition);
}

.ba__dot--on {
  width: 20px;
  border-radius: var(--radius-pill);
  background: var(--surface);
}

/* ── 大厅：四角 ───────────────────────────────── */

.ba__tl {
  position: absolute;
  top: 14px;
  left: 14px;
  display: flex;
  flex-direction: column;
  gap: 9px;
  align-items: flex-start;
}

.ba__who {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 14px 7px 8px;
  min-width: 208px;
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.95);
  box-shadow: var(--shadow-card);
}

.ba__who-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.ba__who-name {
  font-size: var(--text-sm);
  font-weight: 700;
}

.ba__who-state {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.ba__lamp {
  width: 7px;
  height: 7px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--success);
}

.ba__lamp--busy {
  background: var(--border-strong);
  animation: ba-blink 1.3s ease infinite;
}

@keyframes ba-blink {
  50% {
    opacity: 0.35;
  }
}

/* 头像圆形 + 椭圆光环。光环在 themes/ba.css 里画，这里只管形状与尺寸 */
.ba__face {
  position: relative;
  width: 42px;
  height: 42px;
  flex-shrink: 0;
}

.ba__face--sm {
  width: 34px;
  height: 34px;
}

.ba__face img {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  background: var(--bg-sunken);
}

/* 右上：资源条 */
.ba__tr {
  position: absolute;
  top: 14px;
  right: 14px;
  display: flex;
  gap: 8px;
}

.ba__res {
  display: flex;
  align-items: center;
  height: 34px;
  border-radius: var(--radius-pill);
  background: rgba(30, 54, 82, 0.6);
  color: var(--on-accent);
  backdrop-filter: blur(4px);
  overflow: hidden;
}

.ba__res-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--surface);
  color: var(--accent);
}

.ba__res-icon :deep(svg) {
  width: 18px;
  height: 18px;
}

.ba__res-num {
  padding: 0 13px;
  font-size: var(--text-sm);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

/* 左右边缘：切记忆大厅 */
.ba__arrow {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 52px;
  border: none;
  background: rgba(30, 54, 82, 0.34);
  color: var(--on-accent);
  cursor: pointer;
  backdrop-filter: blur(3px);
  transition: background var(--transition);
}

.ba__arrow:hover {
  background: rgba(30, 54, 82, 0.56);
}

.ba__arrow--prev {
  left: 0;
  border-radius: 0 var(--radius-md) var(--radius-md) 0;
}

.ba__arrow--next {
  right: 0;
  border-radius: var(--radius-md) 0 0 var(--radius-md);
}

/* 右侧：继续上次 */
.ba__guide {
  position: absolute;
  right: 42px;
  top: 50%;
  transform: translateY(-50%);
}

.ba__guide-btn {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 8px 14px 8px 9px;
  max-width: 244px;
  border: none;
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.95);
  box-shadow: var(--shadow-card);
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.ba__guide-btn:hover {
  background: var(--surface);
  box-shadow: var(--shadow-float);
}

.ba__guide-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.ba__guide-tag {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.ba__guide-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 168px;
  font-size: var(--text-sm);
  font-weight: 700;
}

.ba__cg-empty {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  margin: 0;
  padding: 9px 16px;
  border-radius: var(--radius-md);
  background: rgba(30, 54, 82, 0.55);
  color: var(--on-accent);
  font-size: var(--text-xs);
  backdrop-filter: blur(4px);
}

.ba__cg-empty code {
  font-family: var(--font-mono);
}

/* ── 底部横栏 ─────────────────────────────────── */

/*
  图标坐在横栏**上沿之外**，只有下半截压进栏里，文字在栏内——视觉上是「一排牌子立在
  台子上」，不是「一排按钮嵌在条里」。这是游戏里那条栏的关键特征，嵌进去就不像了。
*/
.ba__dock {
  --local-dock-h: 48px;
  --local-dock-lift: 18px;
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: calc(var(--local-dock-h) + var(--local-dock-lift));
  pointer-events: none;
}

.ba__dock-bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: var(--local-dock-h);
  background: linear-gradient(180deg, rgba(244, 250, 255, 0.93), rgba(226, 240, 252, 0.97));
  border-top: 2px solid rgba(255, 255, 255, 0.85);
  box-shadow: 0 -2px 12px rgba(14, 32, 54, 0.18);
}

.ba__tabs {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  gap: 2px;
  padding: 0 150px 3px;
  pointer-events: auto;
}

.ba__tab {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  width: 74px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-xs);
  font-weight: 700;
  cursor: pointer;
}

.ba__tab-glyph {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 38px;
  height: 38px;
  border-radius: 11px;
  background: var(--surface);
  color: var(--accent);
  box-shadow: 0 2px 5px rgba(14, 32, 54, 0.2);
  transition: transform var(--duration-fast) ease, box-shadow var(--duration-fast) ease;
}

.ba__tab-glyph :deep(svg) {
  width: 20px;
  height: 20px;
}

.ba__tab:hover .ba__tab-glyph {
  transform: translateY(-3px);
  box-shadow: 0 4px 9px rgba(14, 32, 54, 0.26);
}

.ba__tab:active .ba__tab-glyph {
  transform: translateY(0);
}

.ba__dock-right {
  position: absolute;
  right: 14px;
  bottom: 6px;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
  pointer-events: auto;
}

.ba__clock {
  font-size: var(--text-xs);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  letter-spacing: 0.04em;
}

.ba__biz {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 7px 14px;
  border: none;
  border-radius: var(--radius-md);
  /* 渐变在 themes/ba.css 上色：写死就等于给未来的深色变体埋一处特判 */
  color: var(--on-accent);
  font: inherit;
  font-size: var(--text-xs);
  font-weight: 700;
  cursor: pointer;
  box-shadow: var(--shadow-button);
}

.ba__biz:hover {
  filter: brightness(1.08);
}

.ba__biz-icon {
  display: flex;
  width: 15px;
  height: 15px;
}

.ba__biz-icon :deep(svg) {
  width: 100%;
  height: 100%;
}

.ba__dock-left {
  position: absolute;
  left: 14px;
  bottom: 7px;
  display: flex;
  gap: 6px;
  pointer-events: auto;
}

.ba__mini {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 9px;
  background: var(--surface);
  color: var(--accent);
  cursor: pointer;
  box-shadow: 0 1px 4px rgba(14, 32, 54, 0.2);
}

.ba__mini:hover {
  background: var(--accent);
  color: var(--on-accent);
}

/* ── 内容页 ───────────────────────────────────── */

.ba__bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
  padding: 6px 12px;
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
  .ba__tabs {
    padding: 0 8px 3px;
    overflow-x: auto;
    justify-content: flex-start;
  }

  .ba__dock-left,
  .ba__dock-right {
    display: none;
  }

  .ba__guide {
    display: none;
  }
}
</style>
