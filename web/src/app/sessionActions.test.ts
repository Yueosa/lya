/**
 * 一个会话动作只许有一处实现。
 *
 * 守的是 `app/sessionActions.ts` 开头那条约定。它值得一条测试是因为**已经违反过一次**：
 * 重命名/归档/删除在右键菜单和会话列表页各写了一遍，八处措辞走偏，同一个操作走按钮和走
 * 右键菜单得到两种反馈。而这种问题没有任何常规测试会红——两边各自都工作。
 *
 * 这里问的是源码：除了 `sessionActions.ts` 自己，界面里不该再有谁直接调那三个变更函数。
 * 谁调了，就意味着他在旁边自己拼了一套「问什么、说什么」。
 */

import { describe, expect, it } from 'vitest'

import { sourcesIn } from '../testing/sources'

/** 会改动会话的那三个口。绕过 sessionActions 直接用它们，就是在另起一套文案。 */
const MUTATIONS = ['rename', 'setArchived', 'removeSession']

describe('会话动作只有一处实现', () => {
  it.each(MUTATIONS)('界面里没有别的地方直接调 %s', (name) => {
    // 不写死文件名单：下一个会话列表（命令面板、快捷键）一样会想自己拼一遍
    const called = new RegExp(`(?<![.\\w])${name}\\s*\\(`)
    const offenders = sourcesIn('views', 'shell')
      .filter(({ src }) => called.test(src))
      .map(({ path }) => path)

    expect(
      offenders,
      `这些文件绕过 app/sessionActions 直接改会话，确认文案和提示会在这里走偏`,
    ).toEqual([])
  })

  it('sessionActions 自己确实用了它们', () => {
    // 少了这句，上面几条把正则写错也是绿的——那就成了永远不会红的测试
    const [actions] = sourcesIn('app').filter(({ path }) =>
      path.endsWith('app/sessionActions.ts'),
    )
    expect(actions, '找不到 app/sessionActions.ts').toBeDefined()
    for (const name of MUTATIONS) {
      expect(actions!.src, `sessionActions 里没见到 ${name}`).toMatch(
        new RegExp(`(?<![.\\w])${name}\\s*\\(`),
      )
    }
  })
})
