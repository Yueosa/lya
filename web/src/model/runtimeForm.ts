/**
 * `runtime.toml` 与设置页那张表单之间的双向映射。
 *
 * # 为什么单独成一个模块
 *
 * 这段映射有**两个方向**，而它们必须逐字段对上：读的时候 `agent.max_tool_rounds` → 表单的
 * `maxToolRounds`，写的时候再反过来。加一个配置项只改一边，表现是「设置页能填，保存后没生效」
 * 或者「保存后这一项被清成默认值」——两种都不会报错，也不会有任何测试红。
 *
 * 它原先长在 `ConfigView.vue` 里，于是这个最需要回归网的地方必须挂起整个组件才碰得到。搬出来
 * 之后可以直接测最要紧的那条性质：**读进来再写回去，值不变**。哪个字段漏了一边，那条就红。
 *
 * # 默认值只写一处
 *
 * 原先同一批数字写了两遍——表单 ref 的初值一份，读取时的 `?? 32` 一份。改一个默认值要改两处,
 * 漏一处的表现是「第一次打开显示 32，读完配置变成 16」。现在都从 [`RUNTIME_FORM_DEFAULTS`] 来。
 */

import { bytesToMegabytes, megabytesToBytes } from '../utils/formatBytes'
import {
  buildToolsEnabledPayload,
  readGlobalToolsMode,
  type GlobalToolsMode,
} from '../utils/toolLimits'

/** 设置页那张表单。字段名是界面侧的写法，和 TOML 里的下划线名一一对应。 */
export interface RuntimeForm {
  maxToolRounds: number
  maxParallelTools: number
  maxConsecutiveToolFailures: number
  defaultWorkMode: string
  defaultApiMode: 'completions' | 'responses'
  /** 空串表示没配，对应「跟随清单第一条」；写回时是 `null`，把这个键删掉。 */
  defaultModel: string
  maxIndexEntries: number
  maxIndexChars: number
  indexSummaryChars: number
  shellConfirm: string
  /** 媒体上限以 MB 计——TOML 里是字节，换算在这个模块里做。 */
  maxImageMb: number
  retainLocal: boolean
  retainWeb: boolean
  maxVideoMb: number
  retainVideoLocal: boolean
  retainVideoWeb: boolean
  maxAudioMb: number
  retainAudioLocal: boolean
  retainAudioWeb: boolean
}

/** 表单加上全局工具名单——那一项不在 `form` 里是因为它是模式加集合，不是单值。 */
export interface RuntimeFormState {
  form: RuntimeForm
  toolsMode: GlobalToolsMode
  toolsEnabled: Set<string>
}

/**
 * 配置项缺省时用的值。
 *
 * 和后端 `runtime.toml` 的默认值对齐。这里对不上的后果是安静的：界面显示一个数，实际生效的是
 * 另一个，而两边都不报错。
 */
export const RUNTIME_FORM_DEFAULTS: RuntimeForm = {
  maxToolRounds: 32,
  maxParallelTools: 3,
  maxConsecutiveToolFailures: 16,
  defaultWorkMode: 'agent',
  defaultApiMode: 'completions',
  defaultModel: '',
  maxIndexEntries: 100,
  maxIndexChars: 4000,
  indexSummaryChars: 120,
  shellConfirm: 'unknown',
  maxImageMb: 32,
  retainLocal: true,
  retainWeb: true,
  maxVideoMb: 512,
  retainVideoLocal: true,
  retainVideoWeb: true,
  maxAudioMb: 128,
  retainAudioLocal: true,
  retainAudioWeb: true,
}

function section(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return (parent[key] ?? {}) as Record<string, unknown>
}

function num(source: Record<string, unknown>, key: string, fallback: number): number {
  const raw = source[key]
  if (raw === undefined || raw === null) return fallback
  const value = Number(raw)
  // 配置里写了个非数字（手改 TOML 时打错）时退回默认，别让 NaN 流进表单——
  // 那会让输入框空着，保存时又把 NaN 写回去
  return Number.isFinite(value) ? value : fallback
}

/** 媒体上限：TOML 存字节，表单用 MB。 */
function limitMb(source: Record<string, unknown>, fallbackMb: number): number {
  const raw = source['max_bytes']
  if (raw === undefined || raw === null) return fallbackMb
  const bytes = Number(raw)
  return Number.isFinite(bytes) ? bytesToMegabytes(bytes) : fallbackMb
}

/** 留存开关：只有显式写了 `false` 才算关，缺省是开。 */
function retain(source: Record<string, unknown>, key: string): boolean {
  return source[key] !== false
}

/** 从 `runtime.toml` 的结构读成表单。 */
export function readRuntimeForm(runtime: Record<string, unknown>): RuntimeFormState {
  const D = RUNTIME_FORM_DEFAULTS
  const agent = section(runtime, 'agent')
  const memory = section(runtime, 'memory')
  const shell = section(runtime, 'shell')
  const media = section(runtime, 'media')
  const image = section(media, 'image')
  const video = section(media, 'video')
  const audio = section(media, 'audio')
  const { mode, enabled } = readGlobalToolsMode(runtime)

  return {
    toolsMode: mode,
    toolsEnabled: enabled,
    form: {
      maxToolRounds: num(agent, 'max_tool_rounds', D.maxToolRounds),
      maxParallelTools: num(agent, 'max_parallel_tools', D.maxParallelTools),
      maxConsecutiveToolFailures: num(
        agent,
        'max_consecutive_tool_failures',
        D.maxConsecutiveToolFailures,
      ),
      defaultWorkMode: String(agent['default_work_mode'] ?? D.defaultWorkMode),
      defaultApiMode: agent['default_api_mode'] === 'responses' ? 'responses' : 'completions',
      defaultModel: String(agent['default_model'] ?? D.defaultModel),
      maxIndexEntries: num(memory, 'max_index_entries', D.maxIndexEntries),
      maxIndexChars: num(memory, 'max_index_chars', D.maxIndexChars),
      indexSummaryChars: num(memory, 'index_summary_chars', D.indexSummaryChars),
      shellConfirm: String(shell['confirm'] ?? D.shellConfirm),
      maxImageMb: limitMb(image, D.maxImageMb),
      retainLocal: retain(image, 'retain_local'),
      retainWeb: retain(image, 'retain_web'),
      maxVideoMb: limitMb(video, D.maxVideoMb),
      retainVideoLocal: retain(video, 'retain_local'),
      retainVideoWeb: retain(video, 'retain_web'),
      maxAudioMb: limitMb(audio, D.maxAudioMb),
      retainAudioLocal: retain(audio, 'retain_local'),
      retainAudioWeb: retain(audio, 'retain_web'),
    },
  }
}

/**
 * 把表单拼回 `runtime.toml` 的结构。
 *
 * 每一项都写出来，不做「和默认值相同就省略」的优化：那会让「我明确选了 32」和「我没选过」在
 * 文件里长得一样，而后者的含义是「跟着将来的默认值变」。
 */
export function runtimeFormPayload({
  form,
  toolsMode,
  toolsEnabled,
}: RuntimeFormState): Record<string, unknown> {
  return {
    agent: {
      max_tool_rounds: form.maxToolRounds,
      max_parallel_tools: form.maxParallelTools,
      max_consecutive_tool_failures: form.maxConsecutiveToolFailures,
      default_work_mode: form.defaultWorkMode,
      default_api_mode: form.defaultApiMode,
      // null 会让后端删掉这个键；空串是非法 id，会被启动校验拦下来
      default_model: form.defaultModel || null,
    },
    tools: buildToolsEnabledPayload(toolsMode, toolsEnabled),
    memory: {
      max_index_entries: form.maxIndexEntries,
      max_index_chars: form.maxIndexChars,
      index_summary_chars: form.indexSummaryChars,
    },
    shell: { confirm: form.shellConfirm },
    media: {
      image: {
        max_bytes: megabytesToBytes(form.maxImageMb),
        retain_local: form.retainLocal,
        retain_web: form.retainWeb,
      },
      video: {
        max_bytes: megabytesToBytes(form.maxVideoMb),
        retain_local: form.retainVideoLocal,
        retain_web: form.retainVideoWeb,
      },
      audio: {
        max_bytes: megabytesToBytes(form.maxAudioMb),
        retain_local: form.retainAudioLocal,
        retain_web: form.retainAudioWeb,
      },
    },
  }
}
