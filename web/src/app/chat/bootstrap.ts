import { computed } from 'vue'

import type { ImageContext } from '../../model/markdown'
import { toast } from '../../ui/useToast'
import { client } from './client'
import { currentId, defaultModel, defaultWorkMode, defaultApiMode, imageBootstrap } from './state'

/** Markdown 渲染用的图片上下文（含当前会话 id）。 */
export const imageContext = computed<ImageContext | null>(() => {
  const base = imageBootstrap.value
  const sid = currentId.value
  if (!base) return null
  return { ...base, ...(sid ? { sessionId: sid } : {}) }
})

/** 启动握手。 */
export async function bootstrap(): Promise<void> {
  try {
    const info = await client.bootstrap()
    setImageBootstrap(info.image_token, info.home)
    if (info.default_model_id) {
      defaultModel.value = {
        id: info.default_model_id,
        name: info.default_model_name ?? info.default_model_id,
      }
    } else {
      defaultModel.value = null
    }
  } catch {
    toast('拿不到图片令牌，本地图片将无法显示', 'error')
  }
}

/**
 * 只在**令牌真的变了**时才换对象。
 *
 * `imageBootstrap` 是 `imageContext` 的来源，而 `MarkdownBody` 的 `html` 依赖
 * `imageContext`。无脑赋一个新对象就会让整段正文重渲染、图片重新请求；如果那张图本来
 * 就 404，失败又会来刷一次令牌——一张坏图足够把界面卷进死循环，实测每 140ms 一轮。
 */
function setImageBootstrap(token: string, home: string | null): void {
  if (!home) return
  const now = imageBootstrap.value
  if (now?.token === token && now.home === home) return
  imageBootstrap.value = { token, home }
}

/** 正在进行的令牌刷新；同一批媒体一起报错时只握手一次。 */
let tokenRefresh: Promise<string | null> | null = null

/**
 * 重新握手取当前令牌。
 *
 * 令牌每次进程启动重新生成（泄露出去的链接活不过一次重启），代价是**服务端一重启，
 * 已经打开的页面上所有媒体地址就全部作废**，表现为清一色的 403。开发期反复重启时
 * 这尤其烦人。重新握手一次就能拿到新令牌，不必让用户自己按 F5。
 *
 * 安全性没变：握手接口受同源守卫保护，恶意页面调不到。
 */
export async function refreshImageToken(): Promise<string | null> {
  tokenRefresh ??= (async () => {
    try {
      const info = await client.bootstrap()
      setImageBootstrap(info.image_token, info.home)
      return info.image_token
    } catch {
      return null
    } finally {
      // 下次再失败时重新握手，别把这一次的结果一直用下去
      queueMicrotask(() => {
        tokenRefresh = null
      })
    }
  })()
  return tokenRefresh
}

/** 从 runtime.toml 刷新默认工作模式与 API 栈。 */
export async function refreshRuntimeDefaults(): Promise<void> {
  try {
    const cfg = await client.config()
    const agent = (cfg.runtime['agent'] ?? {}) as Record<string, unknown>
    const mode = agent['default_work_mode']
    if (mode === 'ask' || mode === 'edit' || mode === 'agent') {
      defaultWorkMode.value = mode
    }
    const apiMode = agent['default_api_mode']
    if (apiMode === 'completions' || apiMode === 'responses') {
      defaultApiMode.value = apiMode
    }
  } catch {
    // 读不到就保持上次值
  }
}
