import { computed } from 'vue'

import type { PatchSession } from '../../api/client'
import type { ApiMode, Mode } from '../../api/wire'
import { report } from '../errors'
import { refreshSnapshot } from './snapshot'
import { meta } from './subscription'
import { client } from './client'
import { currentId, models, state, tools } from './state'
import { modelIdForNewSession } from './modelPick'

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

export async function setApiMode(mode: ApiMode): Promise<void> {
  const id = currentId.value
  if (!id || meta.value?.api_mode === mode) return
  if (models.value.length === 0) await loadModels()

  // 当前模型撑不起目标栈时连模型一起换：后端按「换完之后」的组合校验，
  // 分两次发会卡在旧模型 + 新栈这个中间态上。
  const patch: PatchSession = { api_mode: mode }
  const current = meta.value?.model_id
  if (current && !models.value.find((m) => m.id === current)?.modes[mode]) {
    patch.model_id = modelIdForNewSession(mode)
  }

  try {
    const updated = await client.patchSession(id, patch)
    state.value = { ...state.value, meta: updated }
    await loadTools()
  } catch (error) {
    report(error, '切换 API 栈')
  }
}

export const readOnly = computed(() => state.value.meta?.status === 'archived')
