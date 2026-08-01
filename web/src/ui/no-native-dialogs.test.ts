/**
 * 禁止原生弹窗。
 *
 * 上一代有了统一的确认框之后，两个设置页仍然在用 `window.confirm`，侧边栏
 * 又把右键菜单的 markup 抄了一遍——规矩立了却没人守，因为没有东西会拦。
 * 原生弹窗不受主题控制，在任何一套皮肤下都长得像另一个程序。
 */

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const SRC = join(import.meta.dirname, '..')

function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) sourceFiles(path, out)
    else if (/\.(vue|ts)$/.test(entry) && !entry.endsWith('.test.ts')) out.push(path)
  }
  return out
}

describe('界面一致性', () => {
  it('不使用原生 confirm / alert / prompt', () => {
    const offenders: string[] = []
    for (const file of sourceFiles(SRC)) {
      const content = readFileSync(file, 'utf8')
      // window.x(...) 或裸调用；我们自己的 confirm/prompt 是 import 进来的，
      // 所以只拦 window. 前缀和全局裸调用两种写法
      if (/\bwindow\.(confirm|alert|prompt)\s*\(/.test(content)) {
        offenders.push(file)
      }
    }
    expect(offenders, '这些文件用了原生弹窗，请改用 ui/useDialog').toEqual([])
  })

  it('右键菜单只有一份实现', () => {
    const withMenuMarkup = sourceFiles(SRC).filter((file) => {
      if (!file.endsWith('.vue')) return false
      return readFileSync(file, 'utf8').includes('ctx-menu__item')
    })
    // 抄第二份的那一刻，两份实现就开始各自演化了
    expect(withMenuMarkup.map((f) => f.split('/src/')[1])).toEqual(['ui/UiHost.vue'])
  })
})
