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

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { sourcesIn } from '../testing/sources'

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

/**
 * 有会话列表的地方，就得有归档。
 *
 * 归档会话在蔚蓝档案那套皮下**完全看不到**——联系人栏只遍历了 `sessions`，
 * 归档过的对话就此从界面上消失，只能换个主题才找得回来。会话列表视图那边则是另一种
 * 漏法：两批揉成一列，归档只在副标题末尾多三个字，等于没标。
 *
 * 两处的共同点是「列了会话却没列归档」，所以这里直接问源码：谁遍历了 `sessions`，
 * 谁就得同时用上归档抽屉。
 */
describe('列了会话的地方都列了归档', () => {
  it('遍历 sessions 的文件都挂了归档抽屉', () => {
    // 不写死文件名单：新外壳一样会遍历会话，也一样会漏掉归档。
    // 抽屉本体是 ArchiveDock（内部调 useArchiveDock）；旧写法直接调 composable 也算数。
    const offenders = sourcesIn('shell', 'views')
      .filter(({ src }) => /v-for="[^"]*\bin (?:active)?[sS]essions\b/.test(src))
      .filter(({ src }) => !/ArchiveDock|useArchiveDock/.test(src))
      .map(({ path }) => path)

    expect(offenders, '这些地方列了会话却没列归档，归档会在这里凭空消失').toEqual([])
  })
})

/**
 * 装视图的那一格要自己定位。
 *
 * `App.vue` 的加载遮罩是 `position: absolute; inset: 0`，就挂在视图旁边。外壳如果没给
 * 它一个定位祖先，遮罩会一路往上找到外壳根节点，于是「换个会话」变成用近乎不透明的底色
 * 盖住**整屏**再放开——蔚蓝档案那套连联系人栏一起被盖，看着就是整页重载了一遍。
 *
 * 只能读源码来问：happy-dom 不跑真实 CSS，`getComputedStyle` 拿不到 scoped 样式的结果。
 */
describe('外壳给视图留了定位上下文', () => {
  const SHELLS = ['DefaultShell', 'McShell', 'BaShell']

  it.each(SHELLS)('%s', (name) => {
    // import.meta.url 在这套 vitest 环境里是被改写过的虚拟路径，用工作目录找
    const src = readFileSync(resolve(process.cwd(), `src/shell/${name}.vue`), 'utf8')

    const holder = /class="([^"]*)"[^>]*>\s*<slot\s*\/>/.exec(src)
    expect(holder, `${name} 里找不到包着 <slot /> 的元素`).not.toBeNull()

    const classes = (holder?.[1] ?? '').split(/\s+/).filter(Boolean)
    const positioned = classes.some((cls) => {
      const rule = new RegExp(`\\.${cls}\\s*\\{[^}]*\\}`).exec(src)
      return rule ? /position:\s*relative/.test(rule[0]) : false
    })

    expect(
      positioned,
      `${name} 包着 <slot /> 的 .${classes.join('/.')} 没有 position: relative，加载遮罩会盖满整个外壳`,
    ).toBe(true)
  })
})
