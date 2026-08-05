/**
 * 每套外壳都得能走到每个去处。
 *
 * 这条规矩被破过两次，两次都不报错：
 *
 * 1. MC 外壳没有归档入口，主菜单常年显示「0 归档」——数据在服务端好好的，只是没人
 *    给它一个入口
 * 2. 蔚蓝档案外壳没有回首页的按钮。`NAV_ITEMS` 里故意不含 `home`（默认外壳挂在字标
 *    上、MC 外壳自己就是首页），于是只遍历那张表的新外壳就漏掉了它——一离开首页就
 *    再也回不去
 *
 * 共同点是「导航完整性」这件事没有任何东西在看。`NAV_ITEMS` 的注释写着「所有外壳都
 * 遍历这张表」，但表外的去处不受它保护。所以这里换个问法：**把每个外壳里的按钮全点
 * 一遍，看它到底能去哪。**
 */

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { vTip } from '../ui/vTip'
import { THEMES } from '../themes'
import { shellFor } from './registry'
import { NAV_ITEMS, type View } from './types'

/**
 * 从哪些状态出发去点。
 *
 * 首页和内容页要分别挂一次：MC 外壳的主菜单只在首页存在，而它的「返回」按钮只在
 * 内容页存在。只挂一种状态会得出错误结论——第一版就把 MC 误判成不能回首页。
 */
const ENTRY_VIEWS: View[] = ['home', 'tools']

/** 点遍一个外壳里的所有按钮，收集它 emit 出来的去处。 */
async function reachableViews(themeId: string): Promise<Set<View>> {
  const seen = new Set<View>()

  for (const view of ENTRY_VIEWS) {
    const wrapper = mount(shellFor(themeId), {
      props: { view },
      global: { directives: { tip: vTip } },
    })

    // 点击会改变 DOM（展开归档之类），所以多来几轮，每轮重新取按钮
    for (let round = 0; round < 3; round += 1) {
      for (const button of wrapper.findAll('button')) {
        try {
          await button.trigger('click')
        } catch {
          // 有些按钮会去打接口，点不通不影响我们要问的问题
        }
      }
    }

    for (const [event] of wrapper.emitted('navigate') ?? []) {
      seen.add(event as View)
    }
    wrapper.unmount()
  }

  return seen
}

describe('外壳导航完整性', () => {
  it.each(THEMES.map((theme) => theme.id))('%s 的外壳能回首页', async (id) => {
    const reachable = await reachableViews(id)
    expect(
      reachable.has('home'),
      `${id} 用的外壳没有任何能回首页的按钮。NAV_ITEMS 里没有 home，每套外壳要自己给一个`,
    ).toBe(true)
  })

  it.each(THEMES.map((theme) => theme.id))('%s 的外壳覆盖了 NAV_ITEMS', async (id) => {
    const reachable = await reachableViews(id)
    const missing = NAV_ITEMS.filter((item) => !reachable.has(item.view)).map((item) => item.label)
    expect(missing, `${id} 用的外壳到不了这些去处`).toEqual([])
  })
})
