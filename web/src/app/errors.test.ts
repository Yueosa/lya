/**
 * 错误话术只有一处，而且那一处得说得对。
 *
 * 这两条各有来历。原先应用里有两套取错误文本的写法，**各自坏一头**：
 *
 * - 聊天那套用 `String(error)`，会把 `Error: ` 这个前缀带进用户读的那句话。
 * - 其余各屏自己写的 `errMsg` 用 `error.message`，会把 `ApiError` 的状态码丢掉，
 *   于是 404 和 500 读起来一模一样——而那是两件事，一个是这东西不在了，一个是服务端
 *   炸了，用户能不能自己解决全看这个数。
 *
 * 所以下面两条不是「测一个字符串拼接」，是把这两个坑各钉一根钉子。
 */

import { describe, expect, it } from 'vitest'

import { ApiError, errorText } from '../api/client'
import { sourcesIn } from '../testing/sources'

describe('errorText', () => {
  it('ApiError 带上状态码', () => {
    // 404 和 500 必须读得出区别，只报 message 的话两者一样
    expect(errorText(new ApiError(404, '会话不存在'))).toBe('404 会话不存在')
    expect(errorText(new ApiError(500, '会话不存在'))).toBe('500 会话不存在')
  })

  it('后端只给了状态码、没给正文时也说得出话', () => {
    // 空 body 时别拼出「404 」这种带尾空格的半句
    expect(errorText(new ApiError(502, ''))).toBe('HTTP 502')
  })

  it('普通 Error 只要正文，不带 Error: 前缀', () => {
    // String(error) 会给出「Error: 连接被拒绝」，那个前缀是给程序员看的
    expect(errorText(new Error('连接被拒绝'))).toBe('连接被拒绝')
  })

  it('抛出来的不是 Error 也不至于说不出话', () => {
    // 这几种都真的见过
    expect(errorText('直接抛了个字符串')).toBe('直接抛了个字符串')
    expect(errorText(undefined)).toBe('undefined')
    expect(errorText(new Error())).toBe('Error')
  })
})

describe('取错误文本只有一处实现', () => {
  it('没有别的地方再手写 instanceof Error 那三行', () => {
    // 手写一遍就是重新选一次「要不要状态码、要不要 Error: 前缀」，而上面那两个坑
    // 正是这么来的。不写死文件名单：下一屏一样会想就地写一个 errMsg
    const offenders = sourcesIn('app', 'views', 'shell', 'ui', 'model', 'store', 'utils')
      .filter(({ src }) => /instanceof Error\s*\?/.test(src))
      .map(({ path }) => path)

    expect(offenders, '这些文件自己拼了一套错误文本，用 api/client 的 errorText').toEqual([])
  })

  it('errorText 自己确实是靠 instanceof 分情况的', () => {
    // 少了这句，上面那条正则写错也是绿的
    const [client] = sourcesIn('api').filter(({ path }) => path.endsWith('api/client.ts'))
    expect(client!.src).toMatch(/error instanceof ApiError/)
  })
})
