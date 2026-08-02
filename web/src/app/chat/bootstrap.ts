import { computed } from 'vue'

import type { ImageContext } from '../../model/markdown'
import { toast } from '../../ui/useToast'
import { client } from './client'
import { currentId, defaultModel, defaultWorkMode, imageBootstrap } from './state'

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
    if (info.home) imageBootstrap.value = { token: info.image_token, home: info.home }
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

/** 从 runtime.toml 刷新默认工作模式。 */
export async function refreshRuntimeDefaults(): Promise<void> {
  try {
    const cfg = await client.config()
    const agent = (cfg.runtime['agent'] ?? {}) as Record<string, unknown>
    const mode = agent['default_work_mode']
    if (mode === 'ask' || mode === 'edit' || mode === 'agent') {
      defaultWorkMode.value = mode
    }
  } catch {
    // 读不到就保持上次值
  }
}
