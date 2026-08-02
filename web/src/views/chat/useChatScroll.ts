/** 聊天区滚动：跟随流式、跳转按钮、hydration 尾部渲染。 */

import { computed, nextTick, onMounted, ref, watch, type Ref } from 'vue'

import { hydrating, running, timeline } from '../../app/useChat'
import { prefs } from '../../app/usePrefs'

const INITIAL_TAIL = 48

export function useChatScroll(scroller: Ref<HTMLElement | null>) {
  const renderTail = ref<number | null>(null)

  watch(hydrating, (on) => {
    if (on && timeline.value.length > INITIAL_TAIL) {
      renderTail.value = INITIAL_TAIL
    } else if (!on) {
      renderTail.value = null
    }
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

  watch(
    timeline,
    async () => {
      if (!prefs.followStream) return
      await nextTick()
      scrollBottom()
    },
    { deep: true, immediate: true },
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
    nextTick(() => {
      onScroll()
      if (prefs.followStream) scrollBottom()
    })
  })

  return {
    displayTimeline,
    jumpState,
    jumpText,
    jumpTip,
    onScroll,
    jumpLatest,
  }
}
