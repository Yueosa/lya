<!--
  蔚蓝档案的外壳：加载页 → 大厅 → 内容。

  # 三层，而不是两层

  另两套外壳只有「落地 + 内容」两层。这套中间多一个**大厅**：加载页只有字标和缓慢
  走动的画面，点字标才进大厅，大厅底部那条导航才通向各个内容页。

  大厅**不是一个 `View`**。共享的 `View` 联合类型描述的是「哪个内容页」，而大厅是这套
  外壳自己的落地形态——写进 `View` 就要让另两套外壳都决定拿一个它们没有的去处怎么办。

  # 大厅默认几乎是空的

  之前这里照搬了游戏的排布：左上一块面板、右上三个计数胶囊、右下一个大圆、底下一条
  摊开的导航栏，四组东西各据一角，加起来占掉的画面比 CG 本身还多。

  可我们不是那个游戏。它底栏那些格子后面是招募、编成、商店，每一格都通向一整套玩法；
  我们后面是八个设置页。为偶尔去一次的八个页面常年切掉一条画面，不划算——而这一屏的
  内容**就是那张画**。

  所以默认只留四样：左上身份、右上时间、左下换图、右下「继续上次的对话」。八个去处
  折进右下角那个圆，点了才摊开。

  连带的两条：

  - **容器一律近白半透明**，不是天蓝实心块。天蓝腾出来只标记「这个圆能点」——屏幕上
    凡是天蓝的都可以按
  - 翻页箭头收到**左下角**。它换的是这张画，本该待在画的角上，而不是两支大箭头常年
    浮在人脸两侧

  # 素材从数据目录来

  `~/.lya/theme/ba/home/` 是加载图，`cg/` 是记忆大厅（视频）。不内嵌：CG 一个几十 MB，
  进二进制的话每换一张图都要重新构建。目录空着也能用，界面会告诉用户往哪儿放。
-->

<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'

import type { SessionMeta } from '../api/wire'
import {
  createSession,
  currentId,
  defaultModel,
  openSession,
  running,
  sessions,
} from '../app/useChat'
import { fmtBubbleTime } from '../utils/dateFormat'
import BaLogo from '../ui/BaLogo.vue'
import ThemeStage from '../ui/ThemeStage.vue'
import { useThemeStage } from '../ui/useThemeStage'
import { NAV_ICONS } from './icons'
import { openSessionMenu } from './sessionMenu'
import { useArchiveDock } from './useArchiveDock'
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
 * 加载页自动轮播加载图，每次开应用重排顺序，缓慢横移；记忆大厅反过来——**不自动切**、
 * 记住上次挑的那张、也不横移。
 *
 * 差别都来自「这两组素材是什么」：加载图是随手给你看的一屏，动起来才像在读盘；记忆
 * 大厅是用户自己挑了长期看的一张视频，它本身就在动，再叠位移只会晕。
 */
const boot = useThemeStage({ theme: 'ba', kind: 'home', autoMs: 9000, shuffle: true })
const cg = useThemeStage({ theme: 'ba', kind: 'cg', remember: true, pan: false })

/** lya 现在在做什么。 */
const status = computed(() => {
  const model = defaultModel.value?.name ?? '未配置模型'
  return running.value ? `正在输出 · ${model}` : `空闲 · ${model}`
})

/**
 * 「继续上次的对话」指向哪一条。
 *
 * 优先当前打开的那个——从内容页退回大厅，想接着的就是刚才那段；否则取列表第一条，
 * 后端按 `updated_at` 倒序给。一条都没有时是 `null`，卡片换一套说法，不编标题。
 */
const resume = computed<SessionMeta | null>(
  () => sessions.value.find((item) => item.id === currentId.value) ?? sessions.value[0] ?? null,
)

/**
 * 八个去处默认收着。
 *
 * 这一屏天天用的只有「继续对话」，另外八页是偶尔去一次的。为那几次点击常年摊一条底栏
 * 在画面下沿，等于一直切掉一块 CG——而这套皮的大厅，画面本身就是内容。
 */
const menuOpen = ref(false)

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

/** 菜单第一格：回加载页。 */
function backToBoot(): void {
  menuOpen.value = false
  landing.value = 'boot'
}

/** 去某一页，顺手收起菜单——回大厅时不该看见上次摊开的样子。 */
function go(view: View): void {
  menuOpen.value = false
  emit('navigate', view)
}

/** 从内容页回落地：回大厅而不是加载页——加载页是「刚打开」才该看到的。 */
function backToLanding(): void {
  landing.value = 'lobby'
  emit('navigate', 'home')
}

/*
 * 摊开的菜单盖着画面，不能只留「再点一次那个圆」一条出路：手已经移到别处了，还得挪
 * 回右下角那 70px 才关得掉。点空白处由遮罩层管，键盘这条在这儿。
 */
function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') menuOpen.value = false
}
window.addEventListener('keydown', onKey)
onUnmounted(() => window.removeEventListener('keydown', onKey))

/**
 * 聊天页要不要摆成 Momotalk 的两栏。
 *
 * 左边联系人、右边对话——这是**排版**，所以归外壳管；聊天视图本身仍然只有一份实现，
 * 消息树、HITL、分支那些不会被抄进来。
 */
const atChat = computed(() => props.view === 'chat')

/** 归档抽屉，和默认外壳共用一套状态（含展开与否的记忆）。 */
const archive = useArchiveDock()

async function open(id: string): Promise<void> {
  await openSession(id)
  emit('navigate', 'chat')
}

/**
 * 会话区那个大圆：直接进 Momotalk，不再中转一层会话列表页。
 *
 * 联系人栏本身就是列表，先给一个只有列表的页面再点进对话是白走一步。手上没有会话时
 * 就开一个——进去看见空的联系人栏和空的对话区，比停在原地不动强。
 */
async function enterTalk(): Promise<void> {
  if (currentId.value === null) {
    const first = sessions.value[0]
    if (first) await openSession(first.id)
    else await createSession()
  }
  emit('navigate', 'chat')
}

async function startNew(): Promise<void> {
  await createSession()
  emit('navigate', 'chat')
}

/** 联系人卡的副标题。列表接口没有最后一句话，就不编，显示真有的东西。 */
function subtitle(session: SessionMeta): string {
  if (session.id === currentId.value) return running.value ? '正在输入…' : '正在对话'
  return fmtBubbleTime(session.updated_at)
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

    <!--
      ── 大厅 ──────────────────────────────────────

      默认屏上只有四样东西：身份、时间、继续、菜单。其余八个去处折进右下角那个圆里，
      点了才摊开。这一屏的内容就是那张画，chrome 越少越好。
    -->
    <template v-else-if="atLobby">
      <!-- 左上：谁在，忙不忙 -->
      <div class="ba__me">
        <span class="ba__me-face">
          <img src="/icon.png" alt="" @error="($event.target as HTMLElement).style.visibility = 'hidden'" />
        </span>
        <span class="ba__me-text">
          <span class="ba__me-name">lya</span>
          <span class="ba__me-state">
            <i class="ba__lamp" :class="{ 'ba__lamp--busy': running }" />{{ status }}
          </span>
        </span>
      </div>

      <!-- 右上：只有时间。裸字压在画面上，不给底牌 -->
      <p class="ba__clock">{{ clock }}</p>

      <!-- 左下：换一张。翻页是「换这张画」，待在画的角上，不横在人脸旁边 -->
      <div v-if="cg.many.value" class="ba__cgs">
        <button class="ba__cg-btn" type="button" v-tip="'上一张'" aria-label="上一张" @click="cg.go(-1)">
          <span v-html="NAV_ICONS.chevronLeft" />
        </button>
        <button class="ba__cg-btn" type="button" v-tip="'下一张'" aria-label="下一张" @click="cg.go(1)">
          <span v-html="NAV_ICONS.chevronRight" />
        </button>
      </div>

      <p v-if="!cg.items.value.length && !cg.loading.value" class="ba__cg-empty">
        记忆大厅放进 <code>{{ cg.dir.value }}</code>
      </p>

      <!-- 摊开时点画面任意处收起来 -->
      <div v-if="menuOpen" class="ba__scrim" @click="menuOpen = false" />

      <!--
        面板一直在 DOM 里，靠 class 收放。用 v-if 的话每次都要重建八个节点，
        那段从圆里长出来的缩放就没有起点，只会硬闪一下。
      -->
      <nav class="ba__panel" :class="{ 'ba__panel--open': menuOpen }" :aria-hidden="!menuOpen">
        <button class="ba__tile" type="button" :tabindex="menuOpen ? 0 : -1" @click="backToBoot">
          <span class="ba__tile-icon" v-html="NAV_ICONS.home" />
          <p>主页</p>
        </button>
        <button
          v-for="item in NAV_ITEMS"
          :key="item.view"
          class="ba__tile"
          type="button"
          :tabindex="menuOpen ? 0 : -1"
          @click="go(item.view)"
        >
          <span class="ba__tile-icon" v-html="NAV_ICONS[item.icon]" />
          <p>{{ item.label }}</p>
        </button>
      </nav>

      <!-- 右下：主行动 + 菜单开关 -->
      <div class="ba__corner">
        <!--
          主行动是一张会说话的卡，不是一个光秃秃的圆：这一屏唯一真有价值的信息就是
          「上次聊到哪」，写出来才省得进去翻。
        -->
        <button class="ba__resume" type="button" @click="enterTalk">
          <span class="ba__resume-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"
                 stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M21 11.5a8.38 8.38 0 01-.9 3.8 8.5 8.5 0 01-7.6 4.7 8.38 8.38 0 01-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 01-.9-3.8 8.5 8.5 0 014.7-7.6 8.38 8.38 0 013.8-.9h.5a8.48 8.48 0 018 8v.5z" />
              <path d="M8.5 10.5h7M8.5 14h4.5" />
            </svg>
          </span>
          <span class="ba__resume-text">
            <small>{{ resume ? '继续上次的对话' : '还没有对话' }}</small>
            <strong>{{ resume ? resume.title || '未命名' : '开始第一次对话' }}</strong>
          </span>
        </button>

        <button
          class="ba__menu"
          type="button"
          :aria-expanded="menuOpen"
          v-tip="menuOpen ? '收起' : '更多'"
          aria-label="更多"
          @click="menuOpen = !menuOpen"
        >
          <span v-html="NAV_ICONS.menu" />
        </button>
      </div>
    </template>

    <!-- ── 内容页 ────────────────────────────────── -->
    <template v-else>
      <!--
        只有没有联系人栏的页面才需要这条顶栏。聊天页的返回和字标都收进联系人栏的表头，
        多一条通栏 header 会把两栏从顶上切断，Momotalk 的左栏是**顶到天**的。
      -->
      <header v-if="!atChat" class="ba__bar">
        <button class="ba__back" type="button" @click="backToLanding">‹ 大厅</button>
        <BaLogo class="ba__logo--sm" left="lya" right="Archive" />
      </header>

      <!-- 聊天页摆成 Momotalk 的两栏：左联系人、右对话 -->
      <div class="ba__body" :class="{ 'ba__body--talk': atChat }">
        <aside v-if="atChat" class="ba__roster">
          <!-- 返回与字标就是这一栏的表头，不另起标题 -->
          <div class="ba__roster-head">
            <button class="ba__back ba__back--tight" type="button" v-tip="'回大厅'" @click="backToLanding">
              ‹
            </button>
            <BaLogo class="ba__logo--sm" left="lya" right="Archive" />
            <button class="ba__roster-new" type="button" v-tip="'新对话'" @click="startNew">＋</button>
          </div>
          <div class="ba__roster-list">
            <button
              v-for="session in sessions"
              :key="session.id"
              class="ba__contact"
              :class="{ 'ba__contact--on': session.id === currentId }"
              type="button"
              @click="open(session.id)"
              @contextmenu.prevent="openSessionMenu($event, session)"
            >
              <span class="ba__contact-face">
                <img src="/icon.png" alt="" @error="($event.target as HTMLElement).style.visibility = 'hidden'" />
              </span>
              <span class="ba__contact-text">
                <span class="ba__contact-name">{{ session.title || '未命名' }}</span>
                <span class="ba__contact-sub">{{ subtitle(session) }}</span>
              </span>
            </button>
            <p v-if="!sessions.length" class="ba__roster-empty">还没有对话</p>
          </div>

          <!--
            归档钉在栏底，收起来只占一行。混进上面那列的话它们和普通对话长得一样，
            分不出来；而这套外壳之前干脆没遍历 archivedSessions，归档过的对话在这里
            是**完全看不到**的。
          -->
          <div class="ba__archive" :class="{ 'ba__archive--open': archive.open.value }">
            <button class="ba__archive-head" type="button" @click="archive.open.value = !archive.open.value">
              <span class="ba__archive-icon" v-html="NAV_ICONS.archive" />
              <span class="ba__archive-label">归档</span>
              <span v-if="archive.count.value" class="ba__archive-count">{{ archive.count.value }}</span>
              <span class="ba__archive-chevron">›</span>
            </button>
            <div v-if="archive.open.value" class="ba__archive-list">
              <button
                v-for="session in archive.items.value"
                :key="session.id"
                class="ba__contact ba__contact--archived"
                :class="{ 'ba__contact--on': session.id === currentId }"
                type="button"
                @click="open(session.id)"
                @contextmenu.prevent="openSessionMenu($event, session)"
              >
                <span class="ba__contact-face">
                  <img src="/icon.png" alt="" @error="($event.target as HTMLElement).style.visibility = 'hidden'" />
                </span>
                <span class="ba__contact-text">
                  <span class="ba__contact-name">{{ session.title || '未命名' }}</span>
                  <span class="ba__contact-sub">{{ subtitle(session) }}</span>
                </span>
              </button>
              <p v-if="!archive.count.value" class="ba__roster-empty">暂无归档</p>
            </div>
          </div>
        </aside>

        <main class="ba__content">
          <slot />
        </main>
      </div>
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

/* ── 左上：身份 ───────────────────────────────── */

/* 一枚胶囊，不是一块面板：面板本身是一大块色斑，压在 CG 上等于挖掉一角 */
.ba__me {
  position: absolute;
  top: 24px;
  left: 26px;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 7px 20px 7px 7px;
  border-radius: var(--radius-pill);
}

.ba__me-face {
  display: block;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

.ba__me-face img {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  background: var(--bg-sunken);
}

.ba__me-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ba__me-name {
  font-size: 14px;
  font-weight: 800;
  line-height: 1.2;
}

.ba__me-state {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11.5px;
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

/* ── 右上：时间 ───────────────────────────────── */

.ba__clock {
  position: absolute;
  top: 26px;
  right: 30px;
  z-index: 2;
  margin: 0;
  font-size: 34px;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.03em;
  line-height: 1;
}

/* ── 左下：换一张 CG ──────────────────────────── */

.ba__cgs {
  position: absolute;
  left: 26px;
  bottom: 28px;
  z-index: 2;
  display: flex;
  gap: 9px;
}

.ba__cg-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  transition: transform var(--duration-fast) ease;
}

.ba__cg-btn:hover {
  transform: scale(1.08);
}

.ba__cg-btn :deep(svg) {
  width: 22px;
  height: 22px;
  stroke-width: 2.6;
}

/* ── 右下：主行动 + 菜单 ──────────────────────── */

.ba__corner {
  position: absolute;
  right: 30px;
  bottom: 28px;
  z-index: 3;
  display: flex;
  align-items: flex-end;
  gap: 12px;
}

.ba__resume {
  display: flex;
  align-items: center;
  gap: 13px;
  max-width: 350px;
  padding: 10px 20px 10px 10px;
  border: none;
  border-radius: var(--radius-pill);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: transform var(--duration-fast) ease;
}

.ba__resume:hover {
  transform: translateY(-3px);
}

.ba__resume-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 50px;
  height: 50px;
  flex-shrink: 0;
  border-radius: 50%;
}

.ba__resume-icon svg {
  width: 26px;
  height: 26px;
}

.ba__resume-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.ba__resume small {
  font-size: 11.5px;
  line-height: 1.2;
}

.ba__resume strong {
  overflow: hidden;
  max-width: 210px;
  font-size: 15px;
  font-weight: 800;
  line-height: 1.25;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.ba__menu {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 70px;
  height: 70px;
  flex-shrink: 0;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  transition: transform var(--duration-normal) ease;
}

.ba__menu[aria-expanded='true'] {
  transform: rotate(90deg);
}

.ba__menu :deep(svg) {
  width: 30px;
  height: 30px;
  stroke-width: 2.2;
}

/* ── 菜单面板 ─────────────────────────────────── */

.ba__scrim {
  position: absolute;
  inset: 0;
  z-index: 2;
}

/* 从右下那个圆里长出来，所以变换原点钉在自己的右下角 */
.ba__panel {
  position: absolute;
  right: 30px;
  bottom: 112px;
  z-index: 3;
  display: grid;
  grid-template-columns: repeat(4, 84px);
  gap: 6px;
  padding: 16px;
  border-radius: 28px;
  transform-origin: bottom right;
  transform: scale(0.82) translateY(12px);
  opacity: 0;
  pointer-events: none;
  transition:
    transform var(--duration-normal) cubic-bezier(0.34, 1.4, 0.64, 1),
    opacity var(--duration-fast) ease;
}

.ba__panel--open {
  transform: none;
  opacity: 1;
  pointer-events: auto;
}

.ba__tile {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 7px;
  padding: 10px 4px;
  border: none;
  border-radius: 20px;
  background: transparent;
  font: inherit;
  cursor: pointer;
  transition: background var(--transition);
}

.ba__tile-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  transition: transform var(--duration-fast) ease;
}

.ba__tile-icon :deep(svg) {
  width: 23px;
  height: 23px;
}

.ba__tile:hover .ba__tile-icon {
  transform: translateY(-3px);
}

.ba__tile p {
  margin: 0;
  font-size: 12.5px;
  font-weight: 600;
  line-height: 1.2;
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

.ba__body {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

/*
 * 装视图的那一格必须自己是定位上下文。
 *
 * 加载遮罩是 `position: absolute; inset: 0`，挂在视图旁边。这里不定位的话它会一路
 * 找到 `.ba`，于是换个会话就用近乎不透明的底色把**整屏**（连联系人栏）盖住再放开
 * ——看上去就是整页重载了一遍。另两套外壳都有这一行。
 */
.ba__content {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* ── 聊天页：Momotalk 两栏 ─────────────────────── */

.ba__roster {
  width: var(--split-list-width);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: var(--border-width) solid var(--border);
}

.ba__roster-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding: 8px 10px;
}

/* 字标占掉中间的空当，把加号顶到右边 */
.ba__roster-head .ba__logo--sm {
  flex: 1;
  min-width: 0;
}

.ba__back--tight {
  padding: 0;
  width: var(--ctl-h-sm);
  height: var(--ctl-h-sm);
  flex-shrink: 0;
  border-radius: 50%;
  font-size: 20px;
  line-height: 1;
}

.ba__roster-new {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  width: var(--ctl-h-sm);
  height: var(--ctl-h-sm);
  border: none;
  border-radius: 50%;
  font: inherit;
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
}

.ba__roster-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 8px 10px;
}

/* 联系人卡：圆头像在左，名字与状态在右——Momotalk 的行就是这么排的 */
.ba__contact {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 10px;
  margin-bottom: 3px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: background var(--transition);
}

.ba__contact-face {
  width: 42px;
  height: 42px;
  flex-shrink: 0;
}

.ba__contact-face img {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
}

.ba__contact-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.ba__contact-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-sm);
  font-weight: 700;
}

.ba__contact-sub {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-xs);
}

.ba__roster-empty {
  margin: 10px;
  font-size: var(--text-xs);
}

/* ── 联系人栏底部的归档抽屉 ───────────────────── */

.ba__archive {
  flex-shrink: 0;
  /* 展开时最多吃掉一半高度，剩下的还给上面那列活跃对话 */
  max-height: 50%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.ba__archive-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  width: 100%;
  padding: 10px 14px;
  border: none;
  background: transparent;
  font: inherit;
  font-size: var(--text-xs);
  font-weight: 700;
  text-align: left;
  cursor: pointer;
}

.ba__archive-icon {
  display: flex;
  align-items: center;
}

.ba__archive-icon :deep(svg) {
  width: 15px;
  height: 15px;
}

.ba__archive-label {
  flex: 1;
}

.ba__archive-count {
  padding: 1px 8px;
  border-radius: var(--radius-pill);
  font-size: var(--text-xs);
  font-weight: 700;
}

.ba__archive-chevron {
  display: inline-block;
  font-size: 15px;
  line-height: 1;
  transition: transform var(--transition);
}

.ba__archive--open .ba__archive-chevron {
  transform: rotate(90deg);
}

.ba__archive-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 8px 8px;
}

@media (max-width: 720px) {
  /* 四列 386px 装不下窄屏，退成三列 */
  .ba__panel {
    right: 16px;
    bottom: 96px;
    grid-template-columns: repeat(3, 84px);
  }

  .ba__corner {
    right: 16px;
    bottom: 18px;
    gap: 9px;
  }

  /* 标题先让位：认得出是哪段对话就够，挤掉菜单按钮就没法导航了 */
  .ba__resume {
    max-width: 60vw;
  }

  .ba__resume strong {
    max-width: 34vw;
  }

  .ba__menu {
    width: 58px;
    height: 58px;
  }

  .ba__clock {
    font-size: 26px;
  }
}
</style>
