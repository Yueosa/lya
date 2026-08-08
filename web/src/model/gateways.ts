/**
 * 模型列表页的展示映射：按网关分组、短标签、上下文长度、能力摘要。
 *
 * 从 ModelsView 里拆出来——这些函数不认 Vue，也不打 HTTP，锁在组件里只能整页测。
 */

import type { ModelInfo } from '../api/client'
import type { ApiMode } from '../api/wire'
import { groupBy } from '../utils/groupBy'

/** 同一 base_url 下的一批模型，列表左侧一行对应一组。 */
export interface GatewayGroup {
  baseUrl: string
  models: ModelInfo[]
}

/** 按网关地址归组，顺序跟 models 数组里第一次出现的地址一致。 */
export function groupModelsByGateway(models: readonly ModelInfo[]): GatewayGroup[] {
  return groupBy(models, (m) => m.base_url).map(([baseUrl, items]) => ({
    baseUrl,
    models: items,
  }))
}

/** 列表上那行短名：host，有非根 path 时带上 path。 */
export function gatewayLabel(url: string): string {
  try {
    const parsed = new URL(url)
    const path = parsed.pathname.replace(/\/$/, '')
    return path && path !== '/' ? `${parsed.host}${path}` : parsed.host
  } catch {
    return url.length > 36 ? `${url.slice(0, 33)}…` : url
  }
}

/** 上下文长度：整兆 / 整 K 时写成 `128K` 这种，否则原样数字。 */
export function formatContext(value: number | null | undefined): string {
  if (value == null) return '—'
  if (value >= 1_048_576 && value % 1_048_576 === 0) return `${value / 1_048_576}M`
  if (value >= 1024 && value % 1024 === 0) return `${value / 1024}K`
  return String(value)
}

export function modeCaps(model: ModelInfo, mode: ApiMode): string {
  return model.modes[mode]?.capabilities.join(', ') ?? '—'
}

/** 某种协议没配时给一句人话，配了就列能力。 */
export function modeStackHint(model: ModelInfo, mode: ApiMode): string {
  if (model.modes[mode]) return modeCaps(model, mode)
  return mode === 'responses' ? '未配置 modes.responses' : '未配置 modes.completions'
}
