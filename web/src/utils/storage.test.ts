/**
 * `localStorage` 只许从 `utils/storage` 走。
 *
 * 立这条规矩是因为它已经被违反过：七个模块各写一遍防护动作，其中 `useShell` 与
 * `DefaultShell` 在模块顶层裸调 `localStorage.getItem`。后果不是「偏好丢了」，是
 * **整个模块导入就抛**——组件测试第一次挂在这儿，而隐私模式下的浏览器同样会。
 *
 * 规矩靠人记住是记不住的，所以让机器拦。
 */

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { readJson, readLocal, writeJson, writeLocal } from './storage'

const SRC = join(import.meta.dirname, '..')
const ALLOWED = join(SRC, 'utils', 'storage.ts')

function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) sourceFiles(path, out)
    else if (/\.(vue|ts)$/.test(entry) && !entry.endsWith('.test.ts')) out.push(path)
  }
  return out
}

describe('localStorage 访问', () => {
  it('只有 utils/storage.ts 直接碰它', () => {
    const offenders = sourceFiles(SRC)
      .filter((path) => path !== ALLOWED)
      .filter((path) => {
        const text = readFileSync(path, 'utf8')
        // 注释里提一句 localStorage 是可以的，真正调用才算
        return /localStorage\s*[.?[]/.test(text)
      })
      .map((path) => path.slice(SRC.length + 1))

    expect(offenders, '这些文件该改用 readLocal / writeLocal / readJson / writeJson').toEqual(
      [],
    )
  })

  it('读不到就当没存过，不抛', () => {
    // happy-dom 里没有 localStorage，这几个调用本身就是在验「缺了也不炸」
    expect(readLocal('lya.test.missing')).toBeNull()
    expect(() => writeLocal('lya.test.missing', 'x')).not.toThrow()
    expect(() => writeLocal('lya.test.missing', null)).not.toThrow()
  })

  it('JSON 读取与默认值逐字段合并', () => {
    const defaults = { a: 1, b: 2 }
    // 存不进去，所以读回来的一定是默认值——重点是它给的是副本而不是同一个对象
    const got = readJson('lya.test.json', defaults)
    expect(got).toEqual(defaults)
    got.a = 99
    expect(defaults.a).toBe(1)
    expect(() => writeJson('lya.test.json', { a: 3 })).not.toThrow()
  })
})
