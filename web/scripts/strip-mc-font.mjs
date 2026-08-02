#!/usr/bin/env node
/**
 * 从 web/dist 移除 Zpix 字体（约 7MB），供默认 release 内嵌用。
 * 需要 MC 像素字时可跳过此步，或单独把 zpix.ttf 放到 dist/assets/。
 */
import { readdirSync, unlinkSync } from 'node:fs'
import { join } from 'node:path'

const assets = join(import.meta.dirname, '../dist/assets')
let removed = 0

for (const name of readdirSync(assets)) {
  if (!name.startsWith('zpix-') || !name.endsWith('.ttf')) continue
  unlinkSync(join(assets, name))
  removed += 1
  console.log(`strip-mc-font: removed dist/assets/${name}`)
}

if (removed === 0) {
  console.log('strip-mc-font: no zpix font in dist (already slim or not built)')
}
