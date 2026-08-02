<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'

import type { ConfigView as Config } from '../api/client'
import { client, refreshRuntimeDefaults } from '../app/useChat'
import Picker from '../ui/Picker.vue'
import type { PickerOption } from '../ui/Picker.vue'
import RawToml from '../ui/RawToml.vue'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'

type Tab = 'persona' | 'runtime' | 'raw'
type RawFile = 'core' | 'runtime' | 'models' | 'persona'

const TABS: { id: Tab; label: string }[] = [
  { id: 'persona', label: '人设' },
  { id: 'runtime', label: '运行时' },
  { id: 'raw', label: '原始文件' },
]

const RAW_FILES: RawFile[] = ['core', 'runtime', 'models', 'persona']

const tab = ref<Tab>('persona')
const config = ref<Config | null>(null)
const loadError = ref('')
const persona = ref('')
const saving = ref(false)
const rawName = ref<RawFile>('runtime')
const rawText = ref('')

const form = ref({
  maxToolRounds: 32,
  defaultWorkMode: 'agent',
  maxIndexEntries: 100,
  maxIndexChars: 4000,
  indexSummaryChars: 120,
  shellConfirm: 'unknown',
})

onMounted(load)

async function load(): Promise<void> {
  loadError.value = ''
  try {
    const data = await client.config()
    config.value = data
    persona.value = data.persona ?? ''
    readForm(data.runtime)
  } catch (error) {
    const msg = errMsg(error)
    loadError.value = msg.includes('[tables]')
      ? `${msg}\n\n请编辑 ~/.lya/runtime.toml，删除 [tables] 整段后重试。`
      : msg
    toast(`读取配置失败：${msg}`, 'error')
  }
}

function errMsg(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function readForm(runtime: Record<string, unknown>): void {
  const agent = (runtime['agent'] ?? {}) as Record<string, unknown>
  const memory = (runtime['memory'] ?? {}) as Record<string, unknown>
  const shell = (runtime['shell'] ?? {}) as Record<string, unknown>
  form.value = {
    maxToolRounds: Number(agent['max_tool_rounds'] ?? 32),
    defaultWorkMode: String(agent['default_work_mode'] ?? 'agent'),
    maxIndexEntries: Number(memory['max_index_entries'] ?? 100),
    maxIndexChars: Number(memory['max_index_chars'] ?? 4000),
    indexSummaryChars: Number(memory['index_summary_chars'] ?? 120),
    shellConfirm: String(shell['confirm'] ?? 'unknown'),
  }
}

async function saveRuntime(): Promise<void> {
  saving.value = true
  try {
    const applied = (await client.writeRuntime({
      agent: {
        max_tool_rounds: form.value.maxToolRounds,
        default_work_mode: form.value.defaultWorkMode,
      },
      memory: {
        max_index_entries: form.value.maxIndexEntries,
        max_index_chars: form.value.maxIndexChars,
        index_summary_chars: form.value.indexSummaryChars,
      },
      shell: { confirm: form.value.shellConfirm },
    })) as Record<string, unknown>
    readForm(applied)
    await refreshRuntimeDefaults()
    toast('已保存并生效', 'success')
  } catch (error) {
    toast(`保存失败：${errMsg(error)}`, 'error')
  } finally {
    saving.value = false
  }
}

async function savePersona(): Promise<void> {
  saving.value = true
  try {
    await client.writePersona(persona.value)
    toast('人设已保存', 'success')
  } catch (error) {
    toast(`保存失败：${errMsg(error)}`, 'error')
  } finally {
    saving.value = false
  }
}

async function loadRaw(file: RawFile): Promise<void> {
  rawName.value = file
  try {
    rawText.value = await client.rawConfig(file)
  } catch (error) {
    rawText.value = `读取失败：${errMsg(error)}`
  }
}

function pickTab(id: Tab): void {
  tab.value = id
  if (id === 'raw') void loadRaw(rawName.value)
}

const workModeOptions: PickerOption[] = [
  { value: 'ask', label: '问答 — 只能看' },
  { value: 'edit', label: '编辑 — 能读写文件' },
  { value: 'agent', label: '代理 — 含执行命令' },
]

const shellConfirmOptions: PickerOption[] = [
  { value: 'always', label: '每条都问' },
  { value: 'unknown', label: '已知只读的直接放行，其余都问' },
  { value: 'risky', label: '只有命中风险规则才问' },
]

watch(tab, (id) => {
  if (id === 'raw') void loadRaw(rawName.value)
})
</script>

<template>
  <div class="split-view">
    <ViewHead title="设置" />

    <div class="split-view__body">
      <aside class="split-view__list">
        <div class="split-view__list-scroll" style="padding-top: 8px">
          <button
            v-for="item in TABS"
            :key="item.id"
            class="split-view__list-item"
            :class="{ 'split-view__list-item--on': tab === item.id }"
            @click="pickTab(item.id)"
          >
            <span class="split-view__list-title">{{ item.label }}</span>
          </button>
        </div>
      </aside>

      <main class="split-view__main">
        <p v-if="loadError" class="page__error">{{ loadError }}</p>
        <p v-else-if="!config" class="split-view__hint">正在读取…</p>

        <Transition v-else name="lya-split" mode="out-in">
          <section v-if="tab === 'persona'" key="persona" class="page__pane">
          <p class="page__hint">全局人设，写入 <code>persona.toml</code>，所有新会话默认继承。</p>
          <textarea v-model="persona" class="input cfg-text" rows="12" placeholder="全局人设" />
          <div class="row row--end">
            <button class="btn btn--primary" :disabled="saving" @click="savePersona">
              {{ saving ? '保存中…' : '保存' }}
            </button>
          </div>
        </section>

          <section v-else-if="tab === 'runtime'" key="runtime" class="page__pane">
          <div class="panel form-panel">
            <h3 class="form-panel__title">对话</h3>
            <label class="field">
              <span class="field__label">新会话默认模式</span>
              <Picker v-model="form.defaultWorkMode" :options="workModeOptions" />
            </label>
            <label class="field">
              <span class="field__label">单轮最多调几次工具</span>
              <input v-model.number="form.maxToolRounds" class="input" type="number" min="1" max="200" />
              <p class="field__note">到上限就停下，防止模型自己转圈停不下来</p>
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">记忆索引</h3>
            <p class="page__hint">索引常驻在提示词里，越大越占 token。</p>
            <label class="field">
              <span class="field__label">最多列几条</span>
              <input v-model.number="form.maxIndexEntries" class="input" type="number" min="1" />
            </label>
            <label class="field">
              <span class="field__label">索引总字数上限</span>
              <input v-model.number="form.maxIndexChars" class="input" type="number" min="200" />
            </label>
            <label class="field">
              <span class="field__label">每条摘要字数</span>
              <input v-model.number="form.indexSummaryChars" class="input" type="number" min="20" />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">命令执行</h3>
            <label class="field">
              <span class="field__label">什么时候要你确认</span>
              <Picker v-model="form.shellConfirm" :options="shellConfirmOptions" />
            </label>
          </div>

          <div class="row row--end">
            <button class="btn btn--primary" :disabled="saving" @click="saveRuntime">
              {{ saving ? '保存中…' : '保存' }}
            </button>
          </div>
        </section>

          <section v-else key="raw" class="page__pane">
          <p class="page__hint">
            core 只读——改端口等需重启进程才生效。models 里除固定字段外的键（如
            <code>max_tokens</code>、<code>temperature</code>）会原样透传进 API 请求体。
          </p>
          <div class="seg-row">
            <button
              v-for="file in RAW_FILES"
              :key="file"
              class="btn btn--sm"
              :class="{ 'btn--primary': rawName === file }"
              @click="loadRaw(file)"
            >
              {{ file }}.toml
            </button>
          </div>
          <RawToml :text="rawText" />
          </section>
        </Transition>
      </main>
    </div>
  </div>
</template>

<style scoped>
.cfg-text {
  width: 100%;
  height: auto;
  padding: 10px 12px;
  line-height: var(--leading);
  resize: vertical;
  font-family: var(--font-mono);
}

@media (max-width: 640px) {
  .field {
    grid-template-columns: 1fr;
  }

  .field__note {
    grid-column: 1;
  }
}
</style>
