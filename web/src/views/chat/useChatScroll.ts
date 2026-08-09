/**
 * 聊天区滚动：贴底跟随、跳转按钮、进入会话时的尾部渐显与位置恢复。
 *
 * # 别用 timeline 当「该滚了」的信号
 *
 * 时间线变了不等于高度定了：视频、图片在那一刻还不知道自己多高，此时的
 * `scrollHeight` 不含它们，滚过去落不到真正的底部。反过来，高度没变的时间线变化
 * 也不该动位置——打开分支树面板会重新拉一次树，`tree` 一赋值 `timeline` 就重算，
 * 而用户只是开了个侧栏，阅读位置不该被冲到底。
 *
 * 所以「该不该重新归位」只认一个信号：内容容器的高度变了。常驻 `ResizeObserver`
 * 盯着它，谁让内容长高都一样处理，媒体加载完也算。
 *
 * # 别从 scroll 事件反推「这一下是谁滚的」
 *
 * 这是「进会话落不到底」反复修不好的根。原先每次程序化滚动都要举一个
 * `programmaticScroll` 标志，让 `onScroll` 认出来别当成用户意图。这条路走不通：
 * scroll 事件是异步派发的，而进会话时几次归位又常常叠在一起跑，标志总有撤早的时候
 * ——一撤早，我们自己造的滚动就被记成「用户往上翻了」，跟随状态当场丢掉，之后图片
 * 再撑高也没人管，页面就停在半路。加代次、加阈值都只是让它更难触发，没有拆掉前提。
 *
 * 现在**只认真实输入事件**：`wheel`、`touchmove`、键盘/page 键是用户，除此以外的
 * 位置变化一律是我们自己干的。在用户第一次动手之前，内容每长高一次就重新归位一次，
 * 落到进来时记下的那个位置（多数情况就是底部）——也就是「一直往下滚，直到用户打断」。
 *
 * # 本轮跟随 vs 机器偏好
 *
 * `prefs.followStream` 是设置里的默认值；`turnFollow` 是**这一轮**要不要跟。
 * 误滚一下只关 `turnFollow`，不会把偏好写进 localStorage。
 */

import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Ref } from 'vue'

import { currentId, hydrating, running, timeline } from '../../app/useChat'
import { prefs } from '../../app/usePrefs'
import { sessionEnterMotionMs } from '../../ui/useMotion'
import type { ChatScrollControls } from './chatScrollKey'

/**
 * 进会话时先只挂时间线尾部这么多条，落位后再分块揭开。
 */
const INITIAL_TAIL = 48

/** 常态下最多挂载的时间线条数；更早的按需加载。 */
const MAX_VISIBLE = 120

/** 每次「加载更早」或进会话扩窗时多露出多少条。 */
const LOAD_CHUNK = 60

/** 计算时间线窗口：offset = 上方隐藏条数，start = slice 起点。 */
export function computeTimelineWindow(
  length: number,
  renderTail: number | null,
  shownFrom: number | null,
): { offset: number; start: number } {
  if (shownFrom !== null) {
    return { offset: shownFrom, start: shownFrom }
  }
  if (renderTail !== null && length > renderTail) {
    const start = length - renderTail
    return { offset: start, start }
  }
  if (length > MAX_VISIBLE) {
    const start = length - MAX_VISIBLE
    return { offset: start, start }
  }
  return { offset: 0, start: 0 }
}

/** loadEarlier 的下一窗口起点。 */
export function nextShownFrom(offset: number, chunk = LOAD_CHUNK): number {
  return Math.max(0, offset - chunk)
}

/** 离底多少像素以内算「精确贴着底」。用于百分比显示和位置记忆。 */
const BOTTOM_EPS = 2

/**
 * 离底多少像素以内仍然算「在跟随」。
 *
 * 判定跟随/取消跟随、贴底补滚**只用这一阈值**，不再混用 scroll 百分比。
 */
const FOLLOW_EPS = 48

/**
 * 每个会话记一个「离底多远」。
 *
 * 记距底距离而不是 `scrollTop`：会话是往下长的，上面的媒体加载完会把整体撑高，
 * 那时候「离最新消息多远」才是用户真正记得的位置。贴底这个最常见的情况也刚好
 * 就是 0，恢复起来没有误差。
 */
const savedOffsets = new Map<string, number>()

/** 会话删了就把它的位置忘掉，否则这个 map 只增不减。 */
export function forgetScrollPosition(id: string): void {
  savedOffsets.delete(id)
}

function sleepFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

export function useChatScroll(scroller: Ref<HTMLElement | null>, content: Ref<HTMLElement | null>) {
  const renderTail = ref<number | null>(null)
  /** `null` = 自动窗口；数字 = 用户点过「加载更早」后的起点（含 0 = 全文）。 */
  const shownFrom = ref<number | null>(null)
  const timelineReady = ref(false)
  /** 仅进入会话时短暂为 true；SSE 流式更新不再播放入场动画。 */
  const sessionEnterMotion = ref(false)
  let enterMotionTimer: number | null = null

  const sessionAtEnter = currentId.value
  /** 进会话时抄一份；发送/跟随时会清零，避免冻在中间挡流式输出。 */
  const liveRestoreOffset = ref(
    sessionAtEnter !== null ? (savedOffsets.get(sessionAtEnter) ?? 0) : 0,
  )

  const timelineOffset = computed(() => {
    return computeTimelineWindow(
      timeline.value.length,
      renderTail.value,
      shownFrom.value,
    ).offset
  })

  const hiddenCount = computed(() => timelineOffset.value)

  const displayTimeline = computed(() => {
    const items = timeline.value
    const { start } = computeTimelineWindow(
      items.length,
      renderTail.value,
      shownFrom.value,
    )
    return start > 0 ? items.slice(start) : items
  })

  const scrollPercent = ref(0)
  const scrollable = ref(false)
  const atScrollTop = ref(true)
  const atScrollBottom = ref(true)
  const lastTurnFinished = ref(false)
  /** 内容长高时要不要跟着贴底。用户往上翻就 false，翻回底部就 true。 */
  const stuckToBottom = ref(true)
  /** 本轮是否跟着流式输出（误滚只关这个，不动 prefs.followStream）。 */
  const turnFollow = ref(prefs.followStream)
  /**
   * 用户是否已经自己动过滚动条。
   *
   * 在这之前位置归我们管，`onScroll` 读到的一切都是我们自己造成的，不代表任何意图。
   */
  const userTookOver = ref(false)
  let layoutObserver: ResizeObserver | null = null
  let scrollerObserver: ResizeObserver | null = null

  function shouldStickToLiveBottom(): boolean {
    if (running.value && turnFollow.value) return true
    return !userTookOver.value && liveRestoreOffset.value <= BOTTOM_EPS
  }

  /** 发送消息、点「跳到最新」、切分支前调用：这一趟默认跟着流式输出走。 */
  function armSendFollow(): void {
    userTookOver.value = false
    stuckToBottom.value = true
    turnFollow.value = true
    liveRestoreOffset.value = 0
    lastTurnFinished.value = false
    scrollBottom()
  }

  function nudgeIfFollowing(): void {
    stickIfFollowing()
  }

  function stickIfFollowing(): void {
    const el = scroller.value
    if (!el) return
    if (shouldStickToLiveBottom()) {
      el.scrollTop = el.scrollHeight
      return
    }
    if (!userTookOver.value) {
      el.scrollTop = anchorTop(el)
      return
    }
    if (!stuckToBottom.value) return
    if (running.value && !turnFollow.value) return
    el.scrollTop = el.scrollHeight
  }

  watch(
    () => timeline.value.length,
    async (len, prevLen) => {
      if (len > INITIAL_TAIL && prevLen === 0 && hydrating.value) {
        renderTail.value = INITIAL_TAIL
        await nextTick()
        restoreScroll()
      }
    },
  )

  function stopEnterMotion(): void {
    sessionEnterMotion.value = false
    if (enterMotionTimer !== null) {
      window.clearTimeout(enterMotionTimer)
      enterMotionTimer = null
    }
  }

  function startEnterMotion(): void {
    stopEnterMotion()
    sessionEnterMotion.value = true
    enterMotionTimer = window.setTimeout(stopEnterMotion, sessionEnterMotionMs())
  }

  async function expandRenderTail(): Promise<void> {
    const len = timeline.value.length
    if (len <= INITIAL_TAIL) {
      renderTail.value = null
      return
    }
    let tail = renderTail.value ?? INITIAL_TAIL
    const cap = Math.min(len, MAX_VISIBLE)
    while (tail < cap) {
      tail = Math.min(tail + LOAD_CHUNK, cap)
      renderTail.value = tail
      await nextTick()
      restoreScroll()
      await sleepFrame()
    }
    renderTail.value = null
  }

  async function revealTimeline(): Promise<void> {
    if (timeline.value.length > INITIAL_TAIL && renderTail.value === null) {
      renderTail.value = INITIAL_TAIL
    }
    await nextTick()
    restoreScroll()
    await expandRenderTail()
    await nextTick()
    restoreScroll()
    timelineReady.value = true
    startEnterMotion()
  }

  watch(hydrating, async (on, wasOn) => {
    if (on) {
      stopEnterMotion()
      timelineReady.value = false
      return
    }

    if (wasOn === undefined) {
      await revealTimeline()
      return
    }

    if (!wasOn) return
    await revealTimeline()
  }, { immediate: true })

  watch(running, (on, was) => {
    if (on && !was) {
      userTookOver.value = false
      stuckToBottom.value = true
      turnFollow.value = prefs.followStream
      lastTurnFinished.value = false
      if (turnFollow.value) scrollBottom()
    }
    if (was && !on && turnFollow.value) lastTurnFinished.value = true
  })

  watch(
    () => [running.value, turnFollow.value, hydrating.value] as const,
    ([isRunning, follow, isHydrating]) => {
      if (isHydrating || !isRunning || !follow) return
      void nextTick(() => scrollBottom())
    },
    { immediate: true },
  )

  const jumpState = computed<'hidden' | 'following' | 'finished' | 'percent'>(() => {
    if (!scrollable.value) return 'hidden'
    if (running.value && turnFollow.value) return 'following'
    if (lastTurnFinished.value) return 'finished'
    if (!running.value && (atScrollTop.value || atScrollBottom.value)) return 'hidden'
    return 'percent'
  })

  const jumpText = computed(() => {
    if (jumpState.value === 'following') return '跟随'
    if (jumpState.value === 'finished') return '完毕'
    return `${scrollPercent.value}%`
  })

  const jumpTip = computed(() =>
    jumpState.value === 'following' ? '取消跟随' : '跳到最新',
  )

  function onScroll(): void {
    const el = scroller.value
    if (!el) return
    const max = el.scrollHeight - el.clientHeight
    scrollable.value = max > 8
    let nearBottom = true
    if (max <= 0) {
      scrollPercent.value = 100
      atScrollTop.value = true
      atScrollBottom.value = true
    } else {
      const fromBottom = max - el.scrollTop
      atScrollTop.value = el.scrollTop <= BOTTOM_EPS
      atScrollBottom.value = fromBottom <= BOTTOM_EPS
      nearBottom = fromBottom <= FOLLOW_EPS
      scrollPercent.value = atScrollBottom.value
        ? 100
        : Math.min(100, Math.round((el.scrollTop / max) * 100))
    }

    const id = currentId.value
    if (id) savedOffsets.set(id, Math.max(0, max - el.scrollTop))

    if (!userTookOver.value) return
    stuckToBottom.value = nearBottom
    if (running.value && !nearBottom) turnFollow.value = false
    if (atScrollBottom.value) lastTurnFinished.value = false
  }

  /** 用户第一次自己动滚动条：从这里开始位置归他。 */
  function takeOver(): void {
    if (userTookOver.value) return
    userTookOver.value = true
    onScroll()
  }

  function onStreamKeydown(event: KeyboardEvent): void {
    if (
      event.key === 'PageUp' ||
      event.key === 'PageDown' ||
      event.key === 'Home' ||
      event.key === 'End' ||
      event.key === 'ArrowUp' ||
      event.key === 'ArrowDown' ||
      (event.key === ' ' && !event.shiftKey)
    ) {
      takeOver()
    }
  }

  function anchorTop(el: HTMLElement): number {
    const max = Math.max(0, el.scrollHeight - el.clientHeight)
    if (shouldStickToLiveBottom()) return max
    return liveRestoreOffset.value <= BOTTOM_EPS
      ? max
      : Math.max(0, max - liveRestoreOffset.value)
  }

  function settle(place: (el: HTMLElement) => void): void {
    const el = scroller.value
    if (!el) return
    nextTick(() => {
      place(el)
      requestAnimationFrame(() => {
        place(el)
        requestAnimationFrame(() => {
          place(el)
          onScroll()
        })
      })
    })
  }

  function scrollBottom(): void {
    stuckToBottom.value = true
    settle((el) => {
      el.scrollTop = el.scrollHeight
    })
  }

  /** 回到进入会话时记下的位置。 */
  function restoreScroll(): void {
    settle((el) => {
      el.scrollTop = anchorTop(el)
    })
  }

  function jumpLatest(): void {
    if (jumpState.value === 'following') {
      turnFollow.value = false
      return
    }
    armSendFollow()
  }

  function loadEarlier(): void {
    const offset = timelineOffset.value
    if (offset <= 0) return
    const el = scroller.value
    const prevHeight = content.value?.scrollHeight ?? 0
    renderTail.value = null
    shownFrom.value = nextShownFrom(offset)
    nextTick(() => {
      if (!el || !content.value) return
      el.scrollTop += content.value.scrollHeight - prevHeight
      onScroll()
    })
  }

  onMounted(() => {
    if (content.value) {
      let resizeRaf: number | null = null
      layoutObserver = new ResizeObserver(() => {
        if (resizeRaf !== null) return
        resizeRaf = requestAnimationFrame(() => {
          resizeRaf = null
          stickIfFollowing()
        })
      })
      layoutObserver.observe(content.value)
    }

    if (scroller.value) {
      let scrollerRaf: number | null = null
      scrollerObserver = new ResizeObserver(() => {
        if (scrollerRaf !== null) return
        scrollerRaf = requestAnimationFrame(() => {
          scrollerRaf = null
          stickIfFollowing()
        })
      })
      scrollerObserver.observe(scroller.value)
    }

    const el = scroller.value
    el?.addEventListener('wheel', takeOver, { passive: true })
    el?.addEventListener('touchmove', takeOver, { passive: true })
    el?.addEventListener('keydown', onStreamKeydown)
    el?.addEventListener('pointerdown', takeOver)

    nextTick(() => {
      onScroll()
      restoreScroll()
    })
  })

  onUnmounted(() => {
    layoutObserver?.disconnect()
    layoutObserver = null
    scrollerObserver?.disconnect()
    scrollerObserver = null
    const el = scroller.value
    el?.removeEventListener('wheel', takeOver)
    el?.removeEventListener('touchmove', takeOver)
    el?.removeEventListener('keydown', onStreamKeydown)
    el?.removeEventListener('pointerdown', takeOver)
    stopEnterMotion()
  })

  return {
    displayTimeline,
    timelineOffset,
    hiddenCount,
    loadEarlier,
    timelineReady,
    sessionEnterMotion,
    jumpState,
    jumpText,
    jumpTip,
    onScroll,
    jumpLatest,
    armSendFollow,
    nudgeIfFollowing,
  } satisfies ChatScrollControls & Record<string, unknown>
}
