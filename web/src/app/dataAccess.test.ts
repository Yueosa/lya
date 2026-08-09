/**
 * 数据访问只走 `app/` 这一层，界面不直接认识传输层。
 *
 * 这条边界曾经完全形同虚设：`useChat` 门面把 `client` 原封不动再导出一次，于是十个视图
 * 从那儿取到它自己发请求。代价不是「不好看」，是三件具体的事：
 *
 * 1. 同一份配置被四处各拉一遍，其中一处保存之后别处那份还是旧的（会话设置页里的全局人设
 *    就这么显示过期正文，而界面上没有任何迹象）。
 * 2. loading 与错误处理各写一遍，于是同样是「读不出来」有的弹提示有的不弹。
 * 3. 那几屏没法测——要 mock fetch 才能碰到。
 *
 * 所以这里问源码：视图和外壳里不许出现 `client.xxx()`。允许的例外写在下面，每一个都得
 * 附上理由——它们是「只有一处调用、包一层只是多一层」，不是「懒得抽」。
 */

import { describe, expect, it } from 'vitest'

import { sourcesIn } from '../testing/sources'

/**
 * 允许直接用 client 的地方。
 *
 * 加进来之前先问一句：这个请求真的只有一处会用吗？两处就该收进 `app/`——第二处出现的那天
 * 才是文案和缓存开始分叉的那天。
 */
const ALLOWED = new Set([
  // 只有设置页写 runtime.toml、读原文
  'src/views/ConfigView.vue',
  // 只有提示词那一页写 prompt.toml
  'src/views/PromptView.vue',
  // 存储是纯只读的观测面板，一次请求一整份报告
  'src/views/StorageView.vue',
  // 探测某个模型通不通，只有模型页会做
  'src/views/ModelsView.vue',
])

describe('界面不直接发请求', () => {
  it('views/ 和 shell/ 里没有别的地方在用 client', () => {
    const offenders = sourcesIn('views', 'shell')
      .filter(({ src }) => /\bclient\.\w+\s*\(/.test(src))
      .map(({ path }) => path)
      .filter((path) => !ALLOWED.has(path))

    expect(
      offenders,
      '这些地方绕过 app/ 自己发请求，loading 和错误文案会在这里分叉',
    ).toEqual([])
  })

  it('门面不再把 client 转手给界面', () => {
    // 摆在门面上就等于没有边界：十处越界当初全是从这一行来的
    const [facade] = sourcesIn('app').filter(({ path }) => path.endsWith('app/useChat.ts'))
    expect(facade, '找不到 app/useChat.ts').toBeDefined()
    expect(facade!.src).not.toMatch(/^\s*client,\s*$/m)
  })

  it('允许名单上的文件确实还在用 client', () => {
    // 少了这条，名单会慢慢变成一张过期的免罪符：某处早就不用了，条目还留着，
    // 于是下一个人在那个文件里重新开始直接发请求，测试也不会红
    const using = new Set(
      sourcesIn('views', 'shell')
        .filter(({ src }) => /\bclient\.\w+\s*\(/.test(src))
        .map(({ path }) => path),
    )
    const stale = [...ALLOWED].filter((path) => !using.has(path))
    expect(stale, '这些条目可以从允许名单里删了').toEqual([])
  })
})
