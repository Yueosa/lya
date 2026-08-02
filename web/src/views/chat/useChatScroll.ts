/** 聊天区滚动：跟随流式、跳转按钮、hydration 尾部渲染。 */

import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Ref } from 'vue'

import { hydrating, running, timeline } from '../../app/useChat'
import { prefs } from '../../app/usePrefs'
import { sessionEnterMotionMs } from '../../ui/useMotion'

const INITIAL_TAIL = 48

export function useChatScroll(scroller: Ref<HTMLElement | null>) {
  const renderTail = ref<number | null>(null)
  const timelineReady = ref(false)
  /** 仅进入会话时短暂为 true；SSE 流式更新不再播放入场动画。 */
  const sessionEnterMotion = ref(false)
  let enterMotionTimer: number | null = null

  const timelineOffset = computed(() => {
    const items = timeline.value
    const tail = renderTail.value
    if (tail === null || items.length <= tail) return 0
    return items.length - tail
  })

  const displayTimeline = computed(() => {
    const items = timeline.value
    const tail = renderTail.value
    if (tail === null || items.length <= tail) return items
    return items.slice(items.length - tail)
  })

  const scrollPercent = ref(0)
  const scrollable = ref(false)
  const atScrollTop = ref(true)
  const atScrollBottom = ref(true)
  const lastTurnFinished = ref(false)
  let programmaticScroll = false
  let layoutObserver: ResizeObserver | null = null

  watch(
    () => timeline.value.length,
    async (len, prevLen) => {
      if (len > INITIAL_TAIL && prevLen === 0 && hydrating.value) {
        renderTail.value = INITIAL_TAIL
        await nextTick()
        scrollBottom()
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

  async function revealTimeline(): Promise<void> {
    if (timeline.value.length > INITIAL_TAIL && renderTail.value === null) {
      renderTail.value = INITIAL_TAIL
    }
    await nextTick()
    scrollBottom()

    const finish = (): void => {
      requestAnimationFrame(() => {
        scrollBottom()
        timelineReady.value = true
        startEnterMotion()
      })
    }

    if (renderTail.value !== null) {
      requestAnimationFrame(() => {
        renderTail.value = null
        nextTick(() => {
          scrollBottom()
          finish()
        })
      })
    } else {
      finish()
    }

    window.setTimeout(stopLayoutScroll, 600)
  }

  watch(hydrating, async (on, wasOn) => {
    if (on) {
      stopEnterMotion()
      timelineReady.value = false
      startLayoutScroll()
      return
    }

    // 已在 hydrate 结束后才挂载 ChatView（shell 先 await openSession 再 navigate）
    if (wasOn === undefined) {
      await revealTimeline()
      return
    }

    if (!wasOn) return
    await revealTimeline()
  }, { immediate: true })

  watch(
    timeline,
    async () => {
      if (!prefs.followStream || !scroller.value) return
      await nextTick()
      scrollBottom()
    },
    { deep: true },
  )

  watch(running, (on, was) => {
    if (was && !on && prefs.followStream) lastTurnFinished.value = true
  })

  const jumpState = computed<'hidden' | 'following' | 'finished' | 'percent'>(() => {
    if (!scrollable.value) return 'hidden'
    if (running.value && prefs.followStream) return 'following'
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
    if (max <= 0) {
      scrollPercent.value = 100
      atScrollTop.value = true
      atScrollBottom.value = true
    } else {
      atScrollTop.value = el.scrollTop <= 2
      atScrollBottom.value = el.scrollTop + el.clientHeight >= el.scrollHeight - 2
      scrollPercent.value = atScrollBottom.value
        ? 100
        : Math.min(100, Math.round((el.scrollTop / max) * 100))
    }
    if (programmaticScroll) return
    if (running.value && scrollPercent.value < 92) {
      prefs.followStream = false
    }
    if (atScrollBottom.value) lastTurnFinished.value = false
  }

  function scrollBottom(): void {
    const el = scroller.value
    if (!el) return
    programmaticScroll = true
    const attempt = (): void => {
      el.scrollTop = el.scrollHeight
    }
    nextTick(() => {
      attempt()
      requestAnimationFrame(() => {
        attempt()
        requestAnimationFrame(() => {
          attempt()
          programmaticScroll = false
          onScroll()
        })
      })
    })
  }

  function startLayoutScroll(): void {
    const el = scroller.value
    if (!el || layoutObserver) return
    layoutObserver = new ResizeObserver(() => {
      if (prefs.followStream && (hydrating.value || atScrollBottom.value)) {
        scrollBottom()
      }
    })
    layoutObserver.observe(el)
  }

  function stopLayoutScroll(): void {
    layoutObserver?.disconnect()
    layoutObserver = null
  }

  function jumpLatest(): void {
    if (jumpState.value === 'following') {
      prefs.followStream = false
      return
    }
    if (running.value) prefs.followStream = true
    lastTurnFinished.value = false
    scrollBottom()
  }

  onMounted(() => {
    if (hydrating.value) startLayoutScroll()
    nextTick(() => {
      onScroll()
      if (prefs.followStream) scrollBottom()
    })
  })

  onUnmounted(() => {
    stopLayoutScroll()
    stopEnterMotion()
  })

  return {
    displayTimeline,
    timelineOffset,
    timelineReady,
    sessionEnterMotion,
    jumpState,
    jumpText,
    jumpTip,
    onScroll,
    jumpLatest,
  }
}
