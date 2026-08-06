/**
 * 主题背景：一组素材，缓慢平移，渐变切换。
 *
 * 首页（加载页）和大厅共用这一份：前者自动轮播 `home/` 里的加载图，后者手动切
 * `cg/` 里的记忆大厅。差别只有「自动还是手动」和「取哪个分类」，逻辑没必要写两遍。
 *
 * # 平移的量为什么要量出来
 *
 * 素材按高度铺满窗口，宽度按比例溢出——「溢出多少」取决于素材的宽高比和当前窗口，
 * CSS 里算不出来。所以取到 `naturalWidth/Height`（视频是 `videoWidth/Height`）之后
 * 自己算，写进 `--local-pan`，由 CSS 动画消费。窗口尺寸变了要重算。
 *
 * 用 `transform` 而不是 `background-position`：前者走合成层，不会每帧重画一张几 MB
 * 的图。
 */

import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'

import { client } from '../app/chat/client'
import { imageBootstrap } from '../app/chat/state'

/** 一条可以直接渲染的素材。 */
export interface StageItem {
  name: string
  media: 'image' | 'video'
  /** 带令牌的取文件地址。 */
  url: string
  /** 展示名，界面上报「现在放的是哪一个」。 */
  title: string
  /** 预览图地址；视频几十 MB，没加载出来之前先拿它顶着。 */
  poster?: string
}

export interface ThemeStageOptions {
  /** 主题 id，对应 `~/.lya/theme/{id}/`。 */
  theme: string
  /** 素材分类。 */
  kind: 'home' | 'cg'
  /** 自动轮播的间隔毫秒；不给就只能手动切。 */
  autoMs?: number
}

export function useThemeStage(options: ThemeStageOptions) {
  const items = shallowRef<StageItem[]>([])
  const index = ref(0)
  /** 素材目录的绝对路径，空态提示要用。 */
  const dir = ref('')
  const loading = ref(true)
  let timer: number | null = null

  const current = computed<StageItem | null>(() => items.value[index.value] ?? null)
  const many = computed(() => items.value.length > 1)

  function stopAuto(): void {
    if (timer !== null) {
      window.clearInterval(timer)
      timer = null
    }
  }

  function startAuto(): void {
    stopAuto()
    if (!options.autoMs || items.value.length < 2) return
    timer = window.setInterval(() => go(1), options.autoMs)
  }

  /** 切到相对位置；到头绕回去。 */
  function go(delta: number): void {
    const total = items.value.length
    if (total === 0) return
    index.value = (index.value + delta + total) % total
  }

  function urlOf(name: string): string {
    const token = imageBootstrap.value?.token ?? ''
    const q = new URLSearchParams({ kind: options.kind, name, token })
    return `/api/theme/${options.theme}/asset?${q}`
  }

  async function load(): Promise<void> {
    loading.value = true
    try {
      const list = await client.themeAssets(options.theme, options.kind)
      dir.value = list.dir
      items.value = list.assets.map((asset) => ({
        name: asset.name,
        media: asset.media,
        url: urlOf(asset.name),
        title: asset.title ?? asset.name,
        ...(asset.poster ? { poster: urlOf(asset.poster) } : {}),
      }))
      index.value = 0
      startAuto()
    } catch {
      // 拿不到就当没有素材：主题在空目录下也该能用，不值得弹错误
      items.value = []
    } finally {
      loading.value = false
    }
  }

  /**
   * 量出平移距离并写进元素。
   *
   * 素材尺寸要等到 `load` / `loadedmetadata` 才知道，所以这个函数由模板上的事件调用，
   * 窗口 resize 时也要再来一遍。
   */
  function measure(el: HTMLImageElement | HTMLVideoElement): void {
    const w = el instanceof HTMLVideoElement ? el.videoWidth : el.naturalWidth
    const h = el instanceof HTMLVideoElement ? el.videoHeight : el.naturalHeight
    if (!w || !h) return
    const shown = (w / h) * window.innerHeight
    const overflow = Math.max(0, shown - window.innerWidth)
    el.style.setProperty('--local-pan', `-${Math.round(overflow)}px`)
  }

  /** resize 之后重新量：窗口变宽，可平移的余量就变少。 */
  const onResize = (): void => {
    for (const el of Array.from(document.querySelectorAll<HTMLElement>('[data-theme-stage]'))) {
      if (el instanceof HTMLImageElement || el instanceof HTMLVideoElement) measure(el)
    }
  }
  window.addEventListener('resize', onResize)

  // 令牌会在服务端重启后变（旧地址会 403），握手拿到新的就重建地址
  watch(
    () => imageBootstrap.value?.token,
    (token, was) => {
      if (token && was && token !== was) void load()
    },
  )

  onUnmounted(() => {
    stopAuto()
    window.removeEventListener('resize', onResize)
  })

  void load()

  return { items, index, current, many, dir, loading, go, measure, reload: load }
}
