<!--
  主题背景层：一张素材铺满窗口、缓慢平移，切换时渐变。

  两处在用：首页（加载页，自动轮播 `home/`）和大厅（手动切 `cg/`）。幻灯片壳子都挂在
  DOM 上、靠 opacity 交替淡入；**视频源只挂当前张**（淡出中的上一张多留一小会儿），
  其余卸掉——否则多切几次解码器堆满，就会卡顿、黑屏、再也播不动。
-->

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import type { StageItem } from './useThemeStage'

/** 淡出时长；略长于 CSS transition，避免卸源时淡出还没走完。 */
const FADE_MS = 1700

const props = withDefaults(
  defineProps<{
    items: StageItem[]
    index: number
    /** 量平移距离；由 `useThemeStage` 提供。 */
    measure: (el: HTMLImageElement | HTMLVideoElement) => void
    /** 这一层现在看得见吗。看不见就暂停播放，但**不卸载**，缓冲还在。 */
    active?: boolean
    /**
     * 后台预载当前素材（不必看得见）。
     *
     * 加载页要先把大厅那几十 MB 的 CG 暖好：进度条画在首页上，进大厅时已经能播，
     * 不会先闪一帧错尺寸再跳回来。和 `active` 拆开——暖的时候只缓冲，不播放。
     */
    warm?: boolean
    /**
     * 要不要自己画底部进度条。
     *
     * 大厅不需要：进厅前应在首页暖完。加载页若进度反映的是自己那层素材，用这个；
     * 若进度其实在反映另一层（首页条看大厅 CG），由外壳接 `loadProgress` 自己画。
     */
    showProgress?: boolean
    /**
     * 量尺寸之前先按哪种方式铺。
     *
     * 大厅是 `center`：没 metadata 时若没有任何 data-fit，浏览器会按原始像素乱画一帧，
     * 看起来像放大/发糊。加载图留给 measure 事后写 wide/tall。
     *
     * **只在 JS 里写 dataset，不要用 `:data-fit` 绑在模板上**——Vue 重渲时会把
     * `measure` 写好的 wide/tall 清掉，切几次之后画面就空白了。
     */
    defaultFit?: 'center'
  }>(),
  { active: true, warm: false, showProgress: false },
)

const emit = defineEmits<{
  /** 当前素材的就绪程度；外壳可拿去画在别的层上（比如首页条看大厅 CG）。 */
  loadProgress: [state: { pct: number | null; show: boolean }]
}>()

const root = ref<HTMLElement | null>(null)

/**
 * 正在淡出的那一张。交叉淡入要它暂时留着源；淡完再卸，免得解码器堆一排。
 */
const outgoing = ref<number | null>(null)
let releaseTimer = 0

/** 当前张永远占坑（回内容页再回来还能接着缓冲）；淡出张只在可见/预载时留。 */
function isHeld(at: number): boolean {
  if (at === props.index) return true
  if (outgoing.value !== null && at === outgoing.value && (props.active || props.warm)) return true
  return false
}

const heldKey = computed(
  () => `${props.index}:${outgoing.value ?? ''}:${props.active}:${props.warm}:${props.items.length}`,
)

/**
 * 卸掉不该占坑的视频源。
 *
 * 以前所有片子都挂着 `src`，`preload` 在 auto/metadata 之间切——浏览器多半不会真的
 * 把旧缓冲吐出来。首页/大厅多切几次就会卡死、黑屏、再也播不动。
 * 淡出结束后 `removeAttribute('src')` + `load()` 才是把解码器还回去。
 */
function reclaimVideos(): void {
  const slides = root.value?.querySelectorAll<HTMLElement>('.stage__slide')
  slides?.forEach((slide, at) => {
    const video = slide.querySelector('video[data-theme-stage]')
    if (!(video instanceof HTMLVideoElement)) return
    if (isHeld(at)) return
    if (!video.dataset['lyaUrl'] && video.networkState === HTMLMediaElement.NETWORK_EMPTY) return
    video.pause()
    delete video.dataset['lyaUrl']
    video.removeAttribute('src')
    video.removeAttribute('poster')
    video.load()
  })
}

/**
 * 只让「当前这张、且这一层看得见」的视频播；并给该挂源的补上 src。
 *
 * 当前张的缓冲要留着：去内容页再回来是接着播，不是从头下。`warm` 只预载不播放。
 */
function syncPlayback(): void {
  if (props.defaultFit && root.value) {
    root.value.querySelectorAll<HTMLElement>('[data-theme-stage]').forEach((el) => {
      if (!el.dataset['fit']) el.dataset['fit'] = props.defaultFit
    })
  }

  const slides = root.value?.querySelectorAll<HTMLElement>('.stage__slide')
  slides?.forEach((slide, at) => {
    const video = slide.querySelector('video[data-theme-stage]')
    if (!(video instanceof HTMLVideoElement)) return
    const item = props.items[at]
    if (!item || item.media !== 'video') return

    if (isHeld(at)) {
      const want = item.url
      // `video.src` 会变成绝对地址，不能跟相对 path 直接比；用 dataset 记我们挂过的
      if (video.dataset['lyaUrl'] !== want) {
        video.dataset['lyaUrl'] = want
        video.src = want
        if (item.poster) video.setAttribute('poster', item.poster)
        else video.removeAttribute('poster')
      }
      if (props.defaultFit && !video.dataset['fit']) video.dataset['fit'] = props.defaultFit
      if (props.active && at === props.index) void video.play().catch(() => {})
      else video.pause()
    } else {
      video.pause()
    }
  })
  reclaimVideos()
}

watch(
  () => props.index,
  (now, was) => {
    if (typeof was === 'number' && was !== now && (props.active || props.warm)) {
      outgoing.value = was
      window.clearTimeout(releaseTimer)
      releaseTimer = window.setTimeout(() => {
        outgoing.value = null
        void nextTick(syncPlayback)
      }, FADE_MS)
    }
  },
)

watch(heldKey, () => void nextTick(syncPlayback))
onMounted(() => void nextTick(syncPlayback))

/* ── 加载进度 ─────────────────────────────────────────
 *
 * 刻度给调用方（自己画条，或外壳画在另一层）。大厅不画条；首页可以画自己的，
 * 也可以画大厅 CG 的——那才是「打开应用先进首页、条子在首页」该有的样子。
 */

/** 0–1；`null` 表示进度不可知，条子改走来回扫的样子。 */
const pct = ref<number | null>(0)
const showBar = ref(false)

/** 延迟登场的定时器；秒开的素材不该闪一下进度条。 */
let arming = 0
/** 走满之后淡出的定时器。 */
let fading = 0

function currentMedia(): HTMLImageElement | HTMLVideoElement | null {
  const all = root.value?.querySelectorAll<HTMLImageElement | HTMLVideoElement>('[data-theme-stage]')
  return all?.[props.index] ?? null
}

/** 正在预载或正在展示时，才有「加载中」可言。 */
function tracking(): boolean {
  return props.active || props.warm
}

/**
 * 离「能播」还有多远，1 表示到了。
 *
 * 视频不按下载字节算：一个几十 MB 的 CG 缓冲到百分之几就能开播，拿 `buffered/duration`
 * 当进度，条子会卡在 5% 然后突然消失。`readyState` 才是这件事的刻度——0 什么都没有、
 * 1 有元数据、2 有当前帧、3 能一直播下去。短素材可能反过来，缓冲比状态跑得快，所以两者
 * 取大的那个。
 *
 * 图片没有等价的刻度：`complete` 只有是和否，中间量不到。所以返回 `null`，让条子走
 * 不确定的那种。
 */
function readiness(el: HTMLImageElement | HTMLVideoElement): number | null {
  if (el.tagName === 'IMG') {
    const img = el as HTMLImageElement
    return img.complete && img.naturalWidth > 0 ? 1 : null
  }
  const video = el as HTMLVideoElement
  const stage = Math.min(video.readyState, 3) / 3
  const buffered =
    video.duration > 0 && video.buffered.length > 0
      ? video.buffered.end(video.buffered.length - 1) / video.duration
      : 0
  return Math.min(Math.max(stage, buffered), 1)
}

function publish(): void {
  emit('loadProgress', { pct: pct.value, show: showBar.value })
}

function refresh(): void {
  const el = currentMedia()
  const at = tracking() && el ? readiness(el) : 1

  if (at === 1) {
    window.clearTimeout(arming)
    arming = 0
    if (!showBar.value) {
      publish()
      return
    }
    // 先走满再退场，不然会在半路凭空消失
    pct.value = 1
    window.clearTimeout(fading)
    fading = window.setTimeout(() => {
      showBar.value = false
      publish()
    }, 420)
    publish()
    return
  }

  window.clearTimeout(fading)
  fading = 0
  pct.value = at

  if (showBar.value || arming) {
    publish()
    return
  }
  arming = window.setTimeout(() => {
    arming = 0
    const now = currentMedia()
    if (tracking() && now && readiness(now) !== 1) {
      showBar.value = true
      publish()
    }
  }, 150)
  publish()
}

// 换张图、或这一层开始暖/可见时，事件可能早就发过了（缓存命中就一个都不发），
// 所以直接照元素当下的状态重算一遍
watch(
  () => [props.index, props.active, props.warm, props.items.length],
  () => void nextTick(refresh),
  { immediate: true },
)

onUnmounted(() => {
  window.clearTimeout(arming)
  window.clearTimeout(fading)
  window.clearTimeout(releaseTimer)
})

function onMeasured(el: HTMLImageElement | HTMLVideoElement): void {
  if (props.defaultFit && !el.dataset['fit']) el.dataset['fit'] = props.defaultFit
  props.measure(el)
  refresh()
}
</script>

<template>
  <div ref="root" class="stage">
    <div
      v-for="(item, at) in items"
      :key="item.name"
      class="stage__slide"
      :class="{ 'stage__slide--on': at === index }"
    >
      <!--
        记忆大厅一个几十 MB，首帧要等好一会儿。`poster` 由 syncPlayback 挂上。

        视频的 src **不写在模板里**：只给当前张（和淡出中的上一张）挂源，其余
        removeAttribute + load() 还解码器。preload 也只在持有源时才有意义。
      -->
      <video
        v-if="item.media === 'video'"
        data-theme-stage
        class="stage__media"
        :preload="isHeld(at) && (active || warm) && at === index ? 'auto' : isHeld(at) ? 'metadata' : 'none'"
        loop
        muted
        playsinline
        @loadedmetadata="onMeasured($event.target as HTMLVideoElement)"
        @progress="refresh"
        @loadeddata="refresh"
        @canplay="refresh"
        @waiting="refresh"
      />
      <img
        v-else
        data-theme-stage
        class="stage__media"
        :src="item.url"
        alt=""
        decoding="async"
        @load="onMeasured($event.target as HTMLImageElement)"
      />
    </div>

    <!-- 顶底压暗：浮层上的字要在任何画面上都读得清 -->
    <div class="stage__scrim" aria-hidden="true" />

    <!-- 只要自己这层的进度；跨层的条子由外壳画 -->
    <div
      v-if="showProgress && showBar"
      class="stage__bar"
      :class="{ 'stage__bar--wait': pct === null, 'stage__bar--done': pct === 1 }"
      aria-hidden="true"
    >
      <div class="stage__fill" :style="pct === null ? undefined : { transform: `scaleX(${pct})` }" />
    </div>
  </div>
</template>

<style scoped>
.stage {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background: var(--bg-sunken);
}

.stage__slide {
  position: absolute;
  inset: 0;
  opacity: 0;
  transition: opacity 1.6s ease;
}

.stage__slide--on {
  opacity: 1;
}

/*
 * 两种铺法，由 measure() 量完之后写 data-fit 决定：
 *
 * - `wide`：素材比窗口宽（按高度铺满之后还有富余）→ 高度撑满、宽度顺其自然，横向平移
 * - `tall`：素材比窗口窄 → 宽度撑满、高度顺其自然，居中，没有可平移的余量
 *
 * 之前是 `min-width: 100%` 配 `object-fit: cover` 想一招通吃，结果 tall 那种情况下
 * 元素被强行拉到窗口宽、cover 再裁着填满——那就是**画面被放大**的由来。
 *
 * 还没 data-fit 的帧不画：否则浏览器按原始像素先糊一屏，metadata 到了再跳到 cover。
 */
.stage__media {
  position: absolute;
  max-width: none;
  opacity: 0;
}

.stage__media[data-fit] {
  opacity: 1;
}

.stage__media[data-fit='wide'] {
  top: 0;
  left: 0;
  height: 100%;
  width: auto;
  animation: theme-stage-pan 52s linear infinite alternate;
}

.stage__media[data-fit='tall'] {
  top: 50%;
  left: 0;
  width: 100%;
  height: auto;
  transform: translateY(-50%);
}

/*
 * 不平移的那一层：居中铺满，多出来的两边裁掉。
 *
 * 视频不跟着横移——它本身每帧都在变，再叠一个位移就要每帧重新合成一整屏 1080p，
 * 这就是记忆大厅「莫名其妙地横向移动而且非常卡」的由来。
 */
.stage__media[data-fit='center'] {
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/*
 * 只给看得见的那张开合成层。
 *
 * 五个记忆大厅同时挂 will-change 和平移动画，就是五层 1080p 以上的合成层一起抢显存
 * ——**画面闪烁**就是这么来的。隐藏的那些既不该动，也不该占层。
 */
.stage__slide--on .stage__media {
  will-change: transform;
}

.stage__slide:not(.stage__slide--on) .stage__media {
  animation-play-state: paused;
}

@keyframes theme-stage-pan {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(var(--local-pan, 0px));
  }
}

.stage__scrim {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    linear-gradient(180deg, rgba(14, 32, 54, 0.4) 0%, transparent 20%),
    linear-gradient(0deg, rgba(14, 32, 54, 0.45) 0%, transparent 24%);
}

/* ── 底部进度条 ───────────────────────────────── */

.stage__bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 5px;
  overflow: hidden;
  background: rgba(9, 26, 44, 0.34);
  transition: opacity var(--duration-normal) ease;
  z-index: 2;
}

.stage__bar--done {
  opacity: 0;
}

/* 用 scaleX 而不是 width：宽度每帧都要重排，缩放只在合成层上走 */
.stage__fill {
  width: 100%;
  height: 100%;
  transform: scaleX(0);
  transform-origin: left center;
  background: var(--accent);
  transition: transform 240ms ease;
}

/* 图片量不到中间进度，就别假装知道——来回扫，只表示「在忙」 */
.stage__bar--wait .stage__fill {
  width: 30%;
  transform: none;
  transition: none;
  animation: theme-stage-wait 1.1s ease-in-out infinite;
}

@keyframes theme-stage-wait {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(333%);
  }
}

/* 素材是内容不是装饰，减少动效时保留画面但停下平移与淡入 */
@media (prefers-reduced-motion: reduce) {
  .stage__media {
    animation: none;
  }

  .stage__slide {
    transition: none;
  }

  /* 来回扫的那条改成静止的一段，仍然表示「在忙」，但不动 */
  .stage__bar--wait .stage__fill {
    width: 100%;
    animation: none;
    opacity: 0.5;
  }
}
</style>
