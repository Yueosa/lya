/** 内置 tool 的硬编码限制（只读展示，与 Rust 常量对应）。 */
export interface ToolLimitRow {
  label: string
  value: string
}

const LIMITS: Record<string, ToolLimitRow[]> = {
  file_read: [
    { label: '全文行数上限', value: '2,000 行' },
    { label: '全文字节上限', value: '256 KB' },
  ],
  file_write: [{ label: '单次写入上限', value: '1 MB' }],
  file_edit: [{ label: '文件大小上限', value: '8 MB' }],
  dir_list: [
    { label: '默认深度 / 上限', value: '1 / 8' },
    { label: '默认条目 / 上限', value: '300 / 2,000' },
  ],
  image_scan: [
    { label: '默认条目 / 上限', value: '100 / 1,000' },
    { label: '扫描深度上限', value: '8' },
  ],
  web_search: [{ label: '默认结果 / 上限', value: '8 / 20' }],
  web_fetch: [
    { label: '默认字符 / 上限', value: '6,000 / 20,000' },
    { label: '下载体积上限', value: '4 MB' },
    { label: '内网 URL', value: '需 HITL 确认' },
  ],
  bash: [
    { label: '默认超时 / 上限', value: '30s / 600s' },
    { label: '捕获输出', value: '50 KB' },
    { label: '回灌字符', value: '2,000' },
    { label: '确认策略', value: '见全局「命令确认」' },
  ],
  file_manage: [{ label: '路径', value: '家目录内（与 file_read 规则一致）' }],
  system_info: [{ label: '探测命令', value: '内置白名单（uname、df 等）' }],
}

export function toolLimits(name: string): ToolLimitRow[] {
  return LIMITS[name] ?? []
}

export type GlobalToolsMode = 'all' | 'none' | 'custom'

/** 从 runtime.tools 解析全局默认启用模式。 */
export function readGlobalToolsMode(
  runtime: Record<string, unknown>,
): { mode: GlobalToolsMode; enabled: Set<string> } {
  const tools = runtime['tools'] as Record<string, unknown> | undefined
  if (!tools || !Object.prototype.hasOwnProperty.call(tools, 'enabled')) {
    return { mode: 'all', enabled: new Set() }
  }
  const raw = tools['enabled']
  if (!Array.isArray(raw)) {
    return { mode: 'all', enabled: new Set() }
  }
  if (raw.length === 0) {
    return { mode: 'none', enabled: new Set() }
  }
  return { mode: 'custom', enabled: new Set(raw.filter((v): v is string => typeof v === 'string')) }
}

/** 写入 runtime.tools.enabled；`all` 用 null 删键。 */
export function buildToolsEnabledPayload(
  mode: GlobalToolsMode,
  enabled: Set<string>,
): { enabled: string[] | null } {
  if (mode === 'all') return { enabled: null }
  if (mode === 'none') return { enabled: [] }
  return { enabled: [...enabled].sort() }
}
