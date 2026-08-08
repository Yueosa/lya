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
import { readLocal, writeLocal } from '../utils/storage'

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
  /**
   * 记住用户选的是哪一张。
   *
   * 记忆大厅是**挑一张长期看**的东西，每次回大厅都重置到第一张说不过去。按**文件名**
   * 记而不是下标：加删素材之后下标会指到别的东西上，名字不会。
   */
  remember?: boolean
  /**
   * 每次进来打乱顺序。
   *
   * 加载图是一屏「随便给你看点什么」，固定顺序意味着每次开应用都是同一张开场；
   * 记忆大厅相反，那是用户自己挑的，不能动。
   */
  shuffle?: boolean
  /**
   * 缓慢横移。
   *
   * 加载页要这个：一张静止的图配上字标就是张壁纸，动起来才像在读盘。视频不要——
   * 它自己就在动，再叠一层每帧重合成的位移只会又卡又晕。
   */
  pan?: boolean
}

/** 原地无关的洗牌；只在加载图上用，不改调用方拿到的原数组。 */
function shuffled<T>(list: T[]): T[] {
  const out = [...list]
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    ;[out[i], out[j]] = [out[j] as T, out[i] as T]
  }
  return out
}

export function useThemeStage(options: ThemeStageOptions) {
  const pickKey = `lya.stage.${options.theme}.${options.kind}`
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
    if (options.remember) writeLocal(pickKey, items.value[index.value]?.name ?? null)
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
      const mapped = list.assets.map((asset) => ({
        name: asset.name,
        media: asset.media,
        url: urlOf(asset.name),
        title: asset.title ?? asset.name,
        ...(asset.poster ? { poster: urlOf(asset.poster) } : {}),
      }))
      items.value = options.shuffle ? shuffled(mapped) : mapped
      // 恢复上次挑的那张。素材可能已经被删掉，找不到就回到第一张
      const saved = options.remember ? readLocal(pickKey) : null
      const at = saved ? items.value.findIndex((item) => item.name === saved) : -1
      index.value = at >= 0 ? at : 0
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
    // 不平移的那一层直接居中铺满，没有「溢出多少」可言
    if (options.pan === false) {
      el.dataset['fit'] = 'center'
      return
    }

    const w = el instanceof HTMLVideoElement ? el.videoWidth : el.naturalWidth
    const h = el instanceof HTMLVideoElement ? el.videoHeight : el.naturalHeight
    if (!w || !h) return

    // 按高度铺满之后有多宽？比窗口宽才有得平移，比窗口窄就得换一种铺法，
    // 否则拉宽 + 裁切 = 画面被放大
    const shown = (w / h) * window.innerHeight
    const overflow = shown - window.innerWidth
    el.dataset['fit'] = overflow > 1 ? 'wide' : 'tall'
    el.style.setProperty('--local-pan', `-${Math.round(Math.max(0, overflow))}px`)
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
