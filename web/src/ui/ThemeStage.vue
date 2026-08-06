<!--
  主题背景层：一张素材铺满窗口、缓慢平移，切换时渐变。

  两处在用：首页（加载页，自动轮播 `home/`）和大厅（手动切 `cg/`）。所有素材同时挂在
  DOM 上、靠 opacity 交替，是为了让切换真的能交叠淡入——只留当前一张的话，换的瞬间会
  是「消失再出现」。

  代价是多张视频会同时解码，所以视频只在当前那张上 `autoplay`，其余暂停。
-->

<script setup lang="ts">
import { watch } from 'vue'

import type { StageItem } from './useThemeStage'

const props = defineProps<{
  items: StageItem[]
  index: number
  /** 量平移距离；由 `useThemeStage` 提供。 */
  measure: (el: HTMLImageElement | HTMLVideoElement) => void
}>()

/**
 * 只让当前那张视频播。
 *
 * 不这么做的话，几个记忆大厅 CG 会同时解码——一个 1080p 视频解码就够吃掉一个核，
 * 三个并行会让整个界面掉帧，而用户只看得见一个。
 */
watch(
  () => props.index,
  () => {
    const videos = document.querySelectorAll<HTMLVideoElement>('video[data-theme-stage]')
    videos.forEach((video, at) => {
      if (at === props.index) void video.play().catch(() => {})
      else video.pause()
    })
  },
)
</script>

<template>
  <div class="stage">
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
        :autoplay="at === index"
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

/* 按高度铺满、宽度溢出；平移距离由 --local-pan 给，见 useThemeStage 的说明 */
.stage__media {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: auto;
  min-width: 100%;
  max-width: none;
  object-fit: cover;
  animation: theme-stage-pan 52s linear infinite alternate;
  will-change: transform;
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
