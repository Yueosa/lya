/**
 * 对比度检查。
 *
 * MTF 那套原本沿用了上一代的「粉底白字」，看着干净，实测只有 1.87:1——
 * 远低于可读标准的 4.5:1，小字几乎糊在背景里。肉眼很难判断这种事，尤其是
 * 浅色主题上的浅色文字：它「看起来还行」，读起来才累。
 *
 * 所以按 WCAG 的相对亮度公式算，让机器判。将来加像素风时，这条会在第一时间
 * 拦下同样的问题。
 */

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { THEMES } from './index'

/** 从主题文件里读一个 token 的值。 */
function tokenValue(themeId: string, name: string): string {
  const css = readFileSync(join(import.meta.dirname, `${themeId}.css`), 'utf8')
  const match = css.match(new RegExp(`--${name}\\s*:\\s*([^;]+);`))
  if (!match) throw new Error(`${themeId} 没有定义 --${name}`)
  return match[1]!.trim()
}

/** 十六进制转 0–1 的三个通道。 */
function channels(hex: string): [number, number, number] {
  const value = hex.replace('#', '')
  const full =
    value.length === 3
      ? value
          .split('')
          .map((c) => c + c)
          .join('')
      : value
  return [0, 2, 4].map((offset) => parseInt(full.slice(offset, offset + 2), 16) / 255) as [
    number,
    number,
    number,
  ]
}

/** WCAG 相对亮度。 */
function luminance(hex: string): number {
  const [r, g, b] = channels(hex).map((channel) =>
    channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  ) as [number, number, number]
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** 两色的对比度，1 到 21。 */
function contrast(a: string, b: string): number {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number]
  return (high + 0.05) / (low + 0.05)
}

/**
 * 要检查的搭配。
 *
 * 正文级别要 4.5:1（WCAG AA）。次要文字放宽到 3:1——它本来就是刻意压暗的
 * 辅助信息（时间、说明），要求同等对比度反而会让层次消失。
 */
const PAIRS: { fg: string; bg: string; min: number; where: string }[] = [
  { fg: 'text', bg: 'bg', min: 4.5, where: '正文压在页面上' },
  { fg: 'text', bg: 'surface', min: 4.5, where: '正文压在面板上' },
  { fg: 'on-accent', bg: 'accent', min: 4.5, where: '主按钮与用户气泡' },
  { fg: 'on-accent', bg: 'danger', min: 4.5, where: '危险按钮' },
  { fg: 'text-muted', bg: 'bg', min: 3, where: '次要文字' },
  { fg: 'text-muted', bg: 'surface', min: 3, where: '面板上的次要文字' },
]

describe('对比度', () => {
  for (const theme of THEMES) {
    for (const pair of PAIRS) {
      it(`${theme.id}：${pair.where}`, () => {
        const ratio = contrast(tokenValue(theme.id, pair.fg), tokenValue(theme.id, pair.bg))
        expect(
          Number(ratio.toFixed(2)),
          `--${pair.fg} 压在 --${pair.bg} 上只有 ${ratio.toFixed(2)}:1，要求至少 ${pair.min}:1`,
        ).toBeGreaterThanOrEqual(pair.min)
      })
    }
  }

  it('公式本身是对的', () => {
    // 黑白是 21:1，同色是 1:1——算错了这两个数会立刻不对
    expect(Number(contrast('#000000', '#ffffff').toFixed(1))).toBe(21)
    expect(contrast('#123456', '#123456')).toBe(1)
  })
})
