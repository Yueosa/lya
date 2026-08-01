/**
 * token 契约检查。
 *
 * 上一代的 CSS 引用了 `--border` 和 `--surface`，而 `:root` 从没定义过它们——
 * 浏览器静默回退，肉眼看不出来，没人发现。这几条测试就是为了让这种事在 CI 里
 * 红掉，而不是等到某个主题下界面莫名其妙缺一块颜色。
 */

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { THEMES } from './index'
import { LOCAL_PREFIX, TOKENS, TOKEN_NAMES } from './tokens'

const SRC = join(import.meta.dirname, '..')
const THEME_DIR = import.meta.dirname

/** 一套主题 CSS 里定义了哪些 token。 */
function definedIn(file: string): Set<string> {
  const css = readFileSync(join(THEME_DIR, file), 'utf8')
  const names = new Set<string>()
  for (const match of css.matchAll(/^\s*--([\w-]+)\s*:/gm)) {
    names.add(match[1]!)
  }
  return names
}

/** 递归收集所有源码文件。 */
function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) sourceFiles(path, out)
    else if (/\.(css|vue|ts)$/.test(entry) && !entry.endsWith('.test.ts')) out.push(path)
  }
  return out
}

describe('token 契约', () => {
  it.each(THEMES.map((theme) => theme.id))('%s 定义了全部 token', (id) => {
    const defined = definedIn(`${id}.css`)
    const missing = TOKEN_NAMES.filter((name) => !defined.has(name))
    expect(missing, `${id} 缺少这些 token`).toEqual([])
  })

  it.each(THEMES.map((theme) => theme.id))('%s 没有定义契约外的 token', (id) => {
    const defined = [...definedIn(`${id}.css`)]
    const extra = defined.filter((name) => !TOKEN_NAMES.includes(name))
    // 多出来的多半是拼错了名字——它定义了但没人用，而真正要的那个还是空的
    expect(extra, `${id} 定义了清单外的变量，是不是拼错了`).toEqual([])
  })

  it('每套主题都给出了不同的结构值，而不只是换色', () => {
    const shape = THEMES.map((theme) => {
      const css = readFileSync(join(THEME_DIR, `${theme.id}.css`), 'utf8')
      const pick = (name: string) => css.match(new RegExp(`--${name}\\s*:\\s*([^;]+);`))?.[1]?.trim()
      return [pick('shadow-float'), pick('border-width'), pick('bubble-tail-radius')].join('|')
    })
    // 两套风格的差异不在调色板上：一个是柔和模糊阴影 + 细边 + 尖尾巴气泡，
    // 一个是硬偏移阴影 + 粗边 + 无尾巴。如果这些值都一样，说明 token 层
    // 只抽象了颜色，另一套风格迟早要靠改组件 CSS 来实现
    expect(new Set(shape).size).toBe(THEMES.length)
  })

  it('源码里引用的 token 都在契约内', () => {
    const unknown = new Map<string, string>()
    for (const file of sourceFiles(SRC)) {
      const content = readFileSync(file, 'utf8')
      for (const match of content.matchAll(/var\(\s*--([\w-]+)/g)) {
        const name = match[1]!
        // 组件自己的变量按约定带前缀，不该拿去和主题 token 比对
        if (name.startsWith(LOCAL_PREFIX)) continue
        if (!TOKEN_NAMES.includes(name)) unknown.set(name, file)
      }
    }
    expect(Object.fromEntries(unknown), '引用了不存在的 token').toEqual({})
  })

  it('组件不写死颜色，一律走 token', () => {
    const offenders: string[] = []
    for (const file of sourceFiles(SRC)) {
      // 主题文件本来就该写具体值
      if (file.startsWith(join(THEME_DIR)) && file.endsWith('.css')) continue
      const content = readFileSync(file, 'utf8')
      // 只看 CSS 段落里的十六进制色值，避开 TS 里的字符串
      const styles = file.endsWith('.vue')
        ? (content.match(/<style[\s\S]*?<\/style>/g) ?? []).join('\n')
        : file.endsWith('.css')
          ? content
          : ''
      if (/#[0-9a-fA-F]{3,8}\b/.test(styles)) offenders.push(file)
    }
    // 上一版就是这么漏的：气泡渐变里写死了 #1a1b26 和 #9d7cd8，
    // 换成浅色主题时那两处会突兀地留在深色
    expect(offenders, '这些文件里有写死的颜色').toEqual([])
  })

  it('每个 token 都写了用途说明', () => {
    const undocumented = TOKENS.filter((token) => token.doc.split('：')[1]!.trim().length === 0)
    expect(undocumented.map((token) => token.name)).toEqual([])
  })
})
