<!--
  主题背景层：一张素材铺满窗口、缓慢平移，切换时渐变。

  两处在用：首页（加载页，自动轮播 `home/`）和大厅（手动切 `cg/`）。所有素材同时挂在
  DOM 上、靠 opacity 交替，是为了让切换真的能交叠淡入——只留当前一张的话，换的瞬间会
  是「消失再出现」。

  代价是多张视频会同时解码，所以视频只在当前那张上 `autoplay`，其余暂停。
-->

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'

import type { StageItem } from './useThemeStage'

const props = withDefaults(
  defineProps<{
    items: StageItem[]
    index: number
    /** 量平移距离；由 `useThemeStage` 提供。 */
    measure: (el: HTMLImageElement | HTMLVideoElement) => void
    /** 这一层现在看得见吗。看不见就暂停解码，但**不卸载**，缓冲还在。 */
    active?: boolean
  }>(),
  { active: true },
)

const root = ref<HTMLElement | null>(null)

/**
 * 只让「当前这张、且这一层看得见」的视频播。
 *
 * 两件事都要管：
 *
 * - 同层里几个 CG 同时解码，一个 1080p 就够吃掉一个核，而用户只看得见一张
 * - 去内容页之后这一层整个看不见了，还在后台解码几十 MB 纯属白烧
 *
 * 关键是**暂停而不是卸载**：元素留在 DOM 里，缓冲和解码器状态都还在，切回来是
 * 立刻接着播，不是从头下载。
 */
function syncPlayback(): void {
  const videos = root.value?.querySelectorAll<HTMLVideoElement>('video[data-theme-stage]')
  videos?.forEach((video, at) => {
    if (props.active && at === props.index) void video.play().catch(() => {})
    else video.pause()
  })
}

watch(() => [props.index, props.active, props.items.length], () => void nextTick(syncPlayback), {
  immediate: true,
})
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
        记忆大厅一个几十 MB，首帧要等好一会儿。`poster` 是创意工坊条目自带的预览图，
        几百 KB，先顶上去，视频就位了浏览器自己换掉——不给的话这段时间是一片空白。
      -->
      <video
        v-if="item.media === 'video'"
        data-theme-stage
        class="stage__media"
        :src="item.url"
        :poster="item.poster"
        preload="metadata"
        loop
        muted
        playsinline
        @loadedmetadata="measure($event.target as HTMLVideoElement)"
      />
      <img
        v-else
        data-theme-stage
        class="stage__media"
        :src="item.url"
        alt=""
        decoding="async"
        @load="measure($event.target as HTMLImageElement)"
      />
    </div>

    <!-- 顶底压暗：浮层上的字要在任何画面上都读得清 -->
    <div class="stage__scrim" aria-hidden="true" />
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
 */
.stage__media {
  position: absolute;
  max-width: none;
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

/* 素材是内容不是装饰，减少动效时保留画面但停下平移与淡入 */
@media (prefers-reduced-motion: reduce) {
  .stage__media {
    animation: none;
  }

  .stage__slide {
    transition: none;
  }
}
</style>
