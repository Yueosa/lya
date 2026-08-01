<!--
  会话设置：工具开关 + 显示偏好。

  放在一起是因为它们本质上是同一件事——**这个会话我想怎么用**。区别只在于一个
  影响模型能做什么（存后端、跟着会话走），一个影响我看到什么（存本地、跟着这台
  机器走）。分成两个入口反而要人记住哪个在哪。

  模式与模型不在这里：那两个每轮都可能想换，摆在输入框两侧。
-->

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import type { ActionInfo } from '../api/client'
import { client, loadTools, meta, readOnly, toggleTool, tools } from '../app/useChat'
import { prefs } from '../app/usePrefs'

const actions = ref<ActionInfo[]>([])

onMounted(async () => {
  await loadTools()
  try {
    actions.value = await client.actions()
  } catch {
    // 拿不到只是少一块白盒展示，不影响用
  }
})

/** 当前模式下够不着的工具单独归一堆，省得用户开了半天发现没生效。 */
const reachable = computed(() => tools.value.filter((tool) => !outOfReach(tool.min_mode)))
const blocked = computed(() => tools.value.filter((tool) => outOfReach(tool.min_mode)))

function outOfReach(minMode: string): boolean {
  const order = ['ask', 'edit', 'agent']
  const current = order.indexOf(meta.value?.work_mode ?? 'agent')
  return order.indexOf(minMode) > current
}

const DISPLAY: { key: keyof typeof prefs; label: string; hint: string }[] = [
  { key: 'hideReasoning', label: '隐藏思考', hint: '有些模型想得很长，看正文时是噪音' },
  { key: 'hideTools', label: '隐藏工具调用', hint: '只想看结论时清爽些' },
  { key: 'hideResolvedHitl', label: '隐藏已答复的打断', hint: '折起历史里的表单与确认' },
  { key: 'followStream', label: '跟随流式输出', hint: '有新内容时自动滚到底' },
]
</script>

<template>
  <div class="settings">
    <section>
      <h3 class="settings__title">它能用哪些工具</h3>
      <p class="settings__hint">
        存在会话里，换台设备也一样。归档的会话仍然可以调整——只读只是不能发消息。
      </p>
      <label v-for="tool in reachable" :key="tool.name" class="settings__row">
        <input
          type="checkbox"
          :checked="tool.enabled !== false"
          @change="toggleTool(tool.name, ($event.target as HTMLInputElement).checked)"
        />
        <span class="settings__name">{{ tool.raw_name }}</span>
        <code class="settings__perm">{{ tool.permission }}</code>
        <span class="settings__desc">{{ tool.description }}</span>
      </label>

      <template v-if="blocked.length">
        <p class="settings__hint settings__hint--gap">
          下面这些当前模式够不着，切到更宽的模式才会生效：
        </p>
        <div v-for="tool in blocked" :key="tool.name" class="settings__row settings__row--off">
          <span class="settings__name">{{ tool.raw_name }}</span>
          <code class="settings__perm">{{ tool.permission }}</code>
          <span class="settings__desc">需要 {{ tool.min_mode }} 模式</span>
        </div>
      </template>
    </section>

    <section>
      <h3 class="settings__title">它还能对自己做什么</h3>
      <p class="settings__hint">
        动作是它操作自身状态的手段——记东西、问你一句、请求换模式。这些不能关：
        关掉「问你一句」它就只会自己猜了。
      </p>
      <div v-for="action in actions" :key="action.name" class="settings__row settings__row--off">
        <span class="settings__name">{{ action.raw_name }}</span>
        <code class="settings__perm">{{ action.flow === 'await_human' ? '会等你' : '直接继续' }}</code>
        <span class="settings__desc">{{ action.description }}</span>
      </div>
    </section>

    <section>
      <h3 class="settings__title">我想看到什么</h3>
      <p class="settings__hint">只影响显示，存在这台机器上。藏起来的内容不会丢，随时能打开。</p>
      <label v-for="item in DISPLAY" :key="item.key" class="settings__row">
        <input v-model="prefs[item.key]" type="checkbox" />
        <span class="settings__name">{{ item.label }}</span>
        <span class="settings__desc">{{ item.hint }}</span>
      </label>
    </section>

    <p v-if="readOnly" class="settings__hint">这个会话已归档，不能再发消息，但上面这些照常可用。</p>
  </div>
</template>

<style scoped>
.settings {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.settings__title {
  margin: 0 0 4px;
  font-size: var(--text-md);
}

.settings__hint {
  margin: 0 0 10px;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.settings__hint--gap {
  margin-top: 14px;
}

.settings__row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  cursor: pointer;
}

.settings__row:hover {
  background: var(--surface-hover);
}

.settings__row--off {
  opacity: 0.5;
  cursor: default;
  /* 复选框位置留空，让名字和上面那组对齐 */
  padding-left: 32px;
}

.settings__name {
  min-width: 7em;
}

.settings__perm {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.settings__desc {
  flex: 1;
  min-width: 0;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
