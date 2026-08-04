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
 * 所以「该不该重新贴底」只认一个信号：内容容器的高度变了。常驻 `ResizeObserver`
 * 盯着它，谁让内容长高都一样处理，媒体加载完也算，且只在用户本来就贴着底时才跟。
 */

import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Ref } from 'vue'

import { currentId, hydrating, running, timeline } from '../../app/useChat'
import { prefs } from '../../app/usePrefs'
import { sessionEnterMotionMs } from '../../ui/useMotion'

const INITIAL_TAIL = 48

/** 离底多少像素以内算「精确贴着底」。用于百分比显示和位置记忆。 */
const BOTTOM_EPS = 2

/**
 * 离底多少像素以内仍然算「在跟随」。
 *
 * 比 [`BOTTOM_EPS`] 松得多，因为这两件事问的不是一回事：贴没贴底是几何事实，
 * 跟不跟随是用户意图。内容长高、字体换成、图片撑开都会让位置飘几十像素，用 2px
 * 判定的话一次重排就把跟随取消了，之后再长高也没人管——那正是「进会话没滚到底」
 * 的成因。真想离开底部的人不会只滚两行。
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

export function useChatScroll(scroller: Ref<HTMLElement | null>, content: Ref<HTMLElement | null>) {
  const renderTail = ref<number | null>(null)
  const timelineReady = ref(false)
  /** 仅进入会话时短暂为 true；SSE 流式更新不再播放入场动画。 */
  const sessionEnterMotion = ref(false)
  let enterMotionTimer: number | null = null

  // 组件建立时就把上次的位置抄下来：之后的每次滚动都会覆盖 map 里的值
  const sessionAtEnter = currentId.value
  const restoreOffset =
    sessionAtEnter !== null ? (savedOffsets.get(sessionAtEnter) ?? 0) : 0

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
  /** 内容长高时要不要跟着贴底。用户往上翻就false，翻回底部就 true。 */
  const stuckToBottom = ref(restoreOffset <= BOTTOM_EPS)
  let programmaticScroll = false
  /** [`settle`] 的代次，见那里的注释。 */
  let settleGeneration = 0
  let layoutObserver: ResizeObserver | null = null

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

  async function revealTimeline(): Promise<void> {
    if (timeline.value.length > INITIAL_TAIL && renderTail.value === null) {
      renderTail.value = INITIAL_TAIL
    }
    await nextTick()
    restoreScroll()

    const finish = (): void => {
      requestAnimationFrame(() => {
        restoreScroll()
        timelineReady.value = true
        startEnterMotion()
      })
    }

    if (renderTail.value !== null) {
      requestAnimationFrame(() => {
        renderTail.value = null
        nextTick(() => {
          restoreScroll()
          finish()
        })
      })
    } else {
      finish()
    }
  }

  watch(hydrating, async (on, wasOn) => {
    if (on) {
      stopEnterMotion()
      timelineReady.value = false
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

    if (programmaticScroll) return
    // 铺首屏期间位置归 restoreScroll() 管，不能有第二个人写：那会儿内容正在长高，
    // 「此刻没贴底」是过程量不是用户意图。这段时间容器还是 visibility: hidden，
    // 也不可能有真的用户滚动
    if (hydrating.value || !timelineReady.value) return
    stuckToBottom.value = nearBottom
    if (running.value && scrollPercent.value < 92) {
      prefs.followStream = false
    }
    if (atScrollBottom.value) lastTurnFinished.value = false
  }

  /**
   * 连着试三帧：一次 nextTick 后布局还可能再变一次（字体、媒体）。
   *
   * 认代次而不是共享一个布尔：进会话时 `revealTimeline` 连发三次 `restoreScroll`，
   * `ResizeObserver` 还会再插几次，它们叠在几帧里跑。共享布尔的话，最先跑到最内层的
   * 那一次会替还在写 `scrollTop` 的其余几次把守卫撤掉，它们造成的 scroll 事件就被
   * 当成用户滚动，跟随状态当场丢掉。谁最后发起谁说了算，旧的直接不干了。
   */
  function settle(place: (el: HTMLElement) => void): void {
    const el = scroller.value
    if (!el) return
    const generation = ++settleGeneration
    programmaticScroll = true
    const step = (next: () => void): void => {
      if (generation !== settleGeneration) return
      place(el)
      next()
    }
    nextTick(() => {
      step(() => {
        requestAnimationFrame(() => {
          step(() => {
            requestAnimationFrame(() => {
              step(() => {
                programmaticScroll = false
                onScroll()
              })
            })
          })
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

  /** 回到进入会话时记下的位置；之前就在底部（含首次打开）就直接贴底。 */
  function restoreScroll(): void {
    if (restoreOffset <= BOTTOM_EPS) {
      scrollBottom()
      return
    }
    settle((el) => {
      el.scrollTop = Math.max(0, el.scrollHeight - el.clientHeight - restoreOffset)
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
    // 盯内容不盯滚动容器：容器高度不随消息变。也不能定时摘掉，
    // 媒体可能好几秒后才报出自己的尺寸
    if (content.value) {
      layoutObserver = new ResizeObserver(() => {
        // 还在铺首屏，位置该听「进来时记下的那个」的。窗口和 onScroll 让位的那段
        // 一致：这期间位置只有 restoreScroll() 一个主人
        if (hydrating.value || !timelineReady.value) {
          restoreScroll()
          return
        }
        if (!stuckToBottom.value) return
        // 正在输出时跟不跟由偏好定；不在输出时，贴底就该一直贴着
        if (running.value && !prefs.followStream) return
        scrollBottom()
      })
      layoutObserver.observe(content.value)
    }
    nextTick(() => {
      onScroll()
      restoreScroll()
    })
  })

  onUnmounted(() => {
    layoutObserver?.disconnect()
    layoutObserver = null
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
