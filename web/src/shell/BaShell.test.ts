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

describe('蔚蓝档案外壳', () => {
  it('去内容页再回来，背景的 video 元素还是同一个', async () => {
    const wrapper = mountShell('home')

    // 进大厅
    await wrapper.findAll('button').find((b) => b.text().includes('lya'))?.trigger('click')
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

  it('内容页也不卸载背景，只是藏起来', async () => {
    const wrapper = mountShell('tools')
    expect(
      wrapper.findAll('[data-layer="cg"] video').length,
      '内容页也该保留大厅背景，只是不可见',
    ).toBeGreaterThan(0)
  })
})
