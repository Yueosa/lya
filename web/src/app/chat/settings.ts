import { computed } from 'vue'

import type { Mode } from '../../api/wire'
import { report } from './errors'
import { refreshSnapshot } from './snapshot'
import { meta } from './subscription'
import { client } from './client'
import { currentId, models, state, tools } from './state'

export { models, tools }

export async function loadModels(): Promise<void> {
  try {
    models.value = await client.models()
  } catch {
    // 拿不到就只是选不了模型
  }
}

export async function loadTools(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    tools.value = await client.tools(id)
  } catch (error) {
    report(error, '读取工具清单')
  }
}

export async function toggleTool(name: string, enabled: boolean): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.toggleTool(id, name, enabled)
    await loadTools()
  } catch (error) {
    report(error, '切换工具')
  }
}

export async function setMode(mode: Mode): Promise<void> {
  const id = currentId.value
  if (!id || meta.value?.work_mode === mode) return
  try {
    const updated = await client.patchSession(id, { work_mode: mode })
    state.value = { ...state.value, meta: updated }
    await Promise.all([loadTools(), refreshSnapshot()])
  } catch (error) {
    report(error, '切换模式')
  }
}

export async function setModel(modelId: string | null): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    state.value = {
      ...state.value,
      meta: await client.patchSession(id, { model_id: modelId }),
    }
  } catch (error) {
    report(error, '切换模型')
  }
}

export const readOnly = computed(() => state.value.meta?.status === 'archived')
