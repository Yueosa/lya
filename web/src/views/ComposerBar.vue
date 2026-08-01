<!--
  输入框两侧的两个选择器：左边工作模式，右边模型。

  单独摆出来而不是塞进设置面板，是因为这两个**每轮都可能想换**——问个小问题切
  快模型、要动文件切 edit 模式。藏两层菜单里的东西没人会用。

  其余的（工具开关、显示偏好）低频，收进设置面板。
-->

<script setup lang="ts">
import { computed } from 'vue'

import type { Mode } from '../api/wire'
import { meta, models, setMode, setModel } from '../app/useChat'
import { openContextMenu } from '../ui/useContextMenu'

const props = defineProps<{ disabled?: boolean }>()

/** 三种模式，按能力从窄到宽。 */
const MODES: { id: Mode; label: string; hint: string }[] = [
  { id: 'ask', label: '问答', hint: '只能看，不能改任何东西' },
  { id: 'edit', label: '编辑', hint: '能读能写文件，不能执行命令' },
  { id: 'agent', label: '代理', hint: '什么都能做，含执行命令' },
]

const currentMode = computed(() => MODES.find((m) => m.id === meta.value?.work_mode) ?? MODES[2]!)

const currentModel = computed(() => {
  const id = meta.value?.model_id
  if (!id) return '默认模型'
  return models.value.find((model) => model.id === id)?.name ?? id
})

function pickMode(event: MouseEvent): void {
  if (props.disabled) return
  openContextMenu(
    event,
    MODES.map((mode) => ({
      label: `${mode.label} — ${mode.hint}`,
      icon: mode.id === currentMode.value.id ? '●' : '○',
      onSelect: () => void setMode(mode.id),
    })),
  )
}

function pickModel(event: MouseEvent): void {
  if (props.disabled) return
  const current = meta.value?.model_id ?? null
  openContextMenu(event, [
    {
      label: '默认模型',
      icon: current === null ? '●' : '○',
      onSelect: () => void setModel(null),
    },
    { separator: true },
    ...models.value.map((model) => ({
      label: model.api_key_placeholder ? `${model.name}（未配密钥）` : model.name,
      icon: model.id === current ? '●' : '○',
      // 密钥还是占位符的选了也跑不起来，直接灰掉比让人撞一次 401 强
      disabled: model.api_key_placeholder,
      onSelect: () => void setModel(model.id),
    })),
  ])
}
</script>

<template>
  <div class="bar">
    <button class="btn btn--sm" :disabled="disabled" @click="pickMode($event)">
      {{ currentMode.label }}
    </button>
    <span class="bar__gap" />
    <button class="btn btn--sm" :disabled="disabled" @click="pickModel($event)">
      {{ currentModel }}
    </button>
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bar__gap {
  flex: 1;
}
</style>
