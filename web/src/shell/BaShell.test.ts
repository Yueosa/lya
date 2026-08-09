/**
 * 背景层不能随导航销毁重建。
 *
 * 犯过一次：加载页 / 大厅 / 内容页三段用 `v-if` 互斥，去内容页再回大厅时整个
 * `<video>` 被卸载重建——记忆大厅一个 34–89 MB，等于从头下载一遍，表现就是画面先黑
 * 一下再慢慢出来，完全谈不上平滑。
 *
 * 修法是把两个背景层提到 `v-if` 之外、改用 `v-show`，看不见时只暂停解码。这条测试
 * 盯的就是那个提出来的动作有没有被人改回去：**同一个 video 元素要活过一次往返**。
 */

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import { vTip } from '../ui/vTip'
import ThemeStage from '../ui/ThemeStage.vue'
import BaShell from './BaShell.vue'
import type { View } from './types'

/** 给 stage 塞两条假素材，省得依赖真实接口。 */
vi.mock('../ui/useThemeStage', async () => {
  const { ref, computed } = await import('vue')
  return {
    useThemeStage: () => {
      const items = ref([
        { name: 'a.mp4', media: 'video' as const, url: '/a.mp4', title: 'A' },
        { name: 'b.mp4', media: 'video' as const, url: '/b.mp4', title: 'B' },
      ])
      const index = ref(0)
      return {
        items,
        index,
        current: computed(() => items.value[index.value] ?? null),
        many: computed(() => items.value.length > 1),
        dir: ref('/tmp/fake'),
        loading: ref(false),
        go: (delta: number) => {
          index.value = (index.value + delta + items.value.length) % items.value.length
        },
        measure: () => {},
        reload: async () => {},
      }
    },
  }
})

function mountShell(view: View) {
  return mount(BaShell, {
    props: { view },
    global: { directives: { tip: vTip }, stubs: { teleport: true } },
  })
}

/** 模拟大厅 CG 预载完成，否则首页会拦着不让点字标进厅。 */
async function markCgReady(wrapper: ReturnType<typeof mountShell>) {
  const cgStage = wrapper.find('[data-layer="cg"]').findComponent(ThemeStage)
  await cgStage.vm.$emit('loadProgress', { pct: 1, show: false })
  await wrapper.vm.$nextTick()
}

async function enterLobby(wrapper: ReturnType<typeof mountShell>) {
  await markCgReady(wrapper)
  await wrapper.find('.ba__boot-brand').trigger('click')
  await wrapper.vm.$nextTick()
}

describe('蔚蓝档案外壳', () => {
  it('CG 未就绪时不让进大厅，并显示首页进度条', async () => {
    const wrapper = mountShell('home')

    expect(wrapper.find('.ba__boot-bar').exists()).toBe(true)
    expect(wrapper.find('.ba__boot-status').text()).toContain('记忆大厅加载中')

    await wrapper.find('.ba__boot-brand').trigger('click')
    await wrapper.vm.$nextTick()

    expect(wrapper.find('.ba__boot').exists()).toBe(true)
    expect(wrapper.find('.ba__me').exists()).toBe(false)
  })

  it('CG 就绪后可以进大厅', async () => {
    const wrapper = mountShell('home')
    await enterLobby(wrapper)
    expect(wrapper.find('.ba__me').exists()).toBe(true)
    expect(wrapper.find('.ba__boot-bar').exists()).toBe(false)
  })

  it('去内容页再回来，背景的 video 元素还是同一个', async () => {
    const wrapper = mountShell('home')

    await enterLobby(wrapper)
    // 必须只看大厅那一层：加载页那层一直在，会把断言喂饱而掩盖问题
    const before = wrapper.findAll('[data-layer="cg"] video')
    expect(before.length, '大厅该有背景视频').toBeGreaterThan(0)
    const firstEl = before[0]!.element

    // 切到内容页再切回来
    await wrapper.setProps({ view: 'tools' })
    await wrapper.setProps({ view: 'home' })

    const after = wrapper.findAll('[data-layer="cg"] video')
    expect(after.length, '回来之后背景还得在').toBe(before.length)
    expect(
      after[0]!.element,
      '同一个 video 元素要活过往返——重建的话几十 MB 会从头下载',
    ).toBe(firstEl)
  })

  it('从内容页回首页时，CG 已暖好则直接进大厅', async () => {
    const wrapper = mountShell('home')
    await enterLobby(wrapper)
    await wrapper.setProps({ view: 'tools' })
    await wrapper.find('[aria-label="回首页"]').trigger('click')
    await wrapper.setProps({ view: 'home' })
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.ba__me').exists()).toBe(true)
    expect(wrapper.find('.ba__boot').exists()).toBe(false)
  })

  it('内容页也不卸载背景，只是藏起来', async () => {
    const wrapper = mountShell('tools')
    expect(
      wrapper.findAll('[data-layer="cg"] video').length,
      '内容页也该保留大厅背景，只是不可见',
    ).toBeGreaterThan(0)
  })

  it('内容页顶栏有回首页和回大厅，外观页只有回首页', () => {
    const tools = mountShell('tools')
    const toolsBar = tools.find('.ba__bar-nav')
    expect(toolsBar.find('[aria-label="回首页"]').exists()).toBe(true)
    expect(toolsBar.find('[aria-label="回大厅"]').exists()).toBe(true)

    const theme = mountShell('theme')
    const themeBar = theme.find('.ba__bar-nav')
    expect(themeBar.find('[aria-label="回首页"]').exists()).toBe(true)
    expect(themeBar.find('[aria-label="回大厅"]').exists()).toBe(false)
  })

  it('聊天页联系人栏有回首页和回大厅', () => {
    const chat = mountShell('chat')
    const head = chat.find('.ba__roster-head')
    expect(head.find('[aria-label="回首页"]').exists()).toBe(true)
    expect(head.find('[aria-label="回大厅"]').exists()).toBe(true)
  })
})
