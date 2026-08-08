#!/usr/bin/env node
/**
 * 从 web/dist 移除 KaTeX 的 woff 与 ttf 字体（约 800KB），只留 woff2。
 *
 * KaTeX 每种字形都发 woff2/woff/ttf 三份，`@font-face` 按顺序挑第一个认得的。
 * 这个界面跑在内嵌 webview 里，woff2 一定支持，另外两份永远轮不到——它们只是
 * 跟着二进制一起发出去而已。
 *
 * 和 strip-mc-font 不同的是，这一步不损失任何东西：那个会让 MC 主题掉回系统
 * 等宽字，这个在任何认得 woff2 的浏览器上都毫无区别。
 */
import { readdirSync, statSync, unlinkSync } from 'node:fs'
import { join } from 'node:path'

const assets = join(import.meta.dirname, '../dist/assets')
let removed = 0
let bytes = 0

for (const name of readdirSync(assets)) {
  if (!name.startsWith('KaTeX_')) continue
  if (!name.endsWith('.woff') && !name.endsWith('.ttf')) continue
  const path = join(assets, name)
  bytes += statSync(path).size
  unlinkSync(path)
  removed += 1
}

console.log(
  removed === 0
    ? 'strip-katex-fonts: no katex font in dist (no math used, or already slim)'
    : `strip-katex-fonts: removed ${removed} files, ${Math.round(bytes / 1024)} KiB`,
)
