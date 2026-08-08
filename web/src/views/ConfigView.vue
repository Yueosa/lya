<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { errorText } from '../api/client'
import { client } from '../app/client'
import { catalog, ensureCatalog } from '../app/useCatalog'
import { configState, ensureConfig, reloadConfig } from '../app/useConfig'
import { models, refreshRuntimeDefaults } from '../app/useChat'
import ListStatus from '../ui/ListStatus.vue'
import Picker from '../ui/Picker.vue'
import type { PickerOption } from '../ui/Picker.vue'
import RawToml from '../ui/RawToml.vue'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'
import {
  RUNTIME_FORM_DEFAULTS,
  readRuntimeForm,
  runtimeFormPayload,
  type RuntimeForm,
} from '../model/runtimeForm'
import { type GlobalToolsMode } from '../utils/toolLimits'

type Tab = 'runtime' | 'raw'
type RawFile = 'core' | 'runtime' | 'models' | 'persona'

const TABS: { id: Tab; label: string }[] = [
  { id: 'runtime', label: '默认配置' },
  { id: 'raw', label: '原始文件' },
]

const RAW_FILES: RawFile[] = ['core', 'runtime', 'models', 'persona']

const tab = ref<Tab>('runtime')
const loadError = ref('')
const saving = ref(false)
const rawName = ref<RawFile>('runtime')
const rawText = ref('')

// 默认值来自 model，别在这儿再抄一份——抄的那份和读取时的 fallback 迟早会对不上
const form = ref<RuntimeForm>({ ...RUNTIME_FORM_DEFAULTS })

// 全局工具目录，和工具页共用一份
const catalogTools = catalog.tools
const globalMode = ref<GlobalToolsMode>('all')
const globalEnabled = ref<Set<string>>(new Set())

onMounted(load)

async function load(): Promise<void> {
  loadError.value = ''
  // 配置和工具目录都是共享的，别处读过就不会再发一次请求
  await Promise.all([ensureConfig(), ensureCatalog()])

  const msg = configState.error.value || catalog.error.value
  if (msg) {
    loadError.value = msg.includes('[tables]')
      ? `${msg}\n\n请编辑 ~/.lya/runtime.toml，删除 [tables] 整段后重试。`
      : msg
    toast(`读取配置失败：${msg}`, 'error')
    return
  }
  const data = configState.config.value
  if (data) readForm(data.runtime)
}


/** 把读到的 runtime.toml 铺进表单。映射本体在 model/runtimeForm.ts。 */
function readForm(runtime: Record<string, unknown>): void {
  const state = readRuntimeForm(runtime)
  form.value = state.form
  globalMode.value = state.toolsMode
  globalEnabled.value = state.toolsEnabled
}

function setGlobalMode(mode: GlobalToolsMode): void {
  globalMode.value = mode
  if (mode === 'custom' && globalEnabled.value.size === 0) {
    globalEnabled.value = new Set(catalogTools.value.map((tool) => tool.name))
  }
}

function toggleGlobalTool(name: string, checked: boolean): void {
  if (globalMode.value === 'all') {
    globalMode.value = 'custom'
    globalEnabled.value = new Set(
      catalogTools.value.filter((tool) => tool.name !== name).map((tool) => tool.name),
    )
    if (checked) globalEnabled.value.add(name)
    return
  }
  if (globalMode.value === 'none') {
    globalMode.value = 'custom'
    globalEnabled.value = new Set(checked ? [name] : [])
    return
  }
  const next = new Set(globalEnabled.value)
  if (checked) next.add(name)
  else next.delete(name)
  globalEnabled.value = next
}

async function saveRuntime(): Promise<void> {
  saving.value = true
  try {
    const applied = (await client.writeRuntime(
      runtimeFormPayload({
        form: form.value,
        toolsMode: globalMode.value,
        toolsEnabled: globalEnabled.value,
      }),
    )) as Record<string, unknown>
    readForm(applied)
    // 共享的那份也得跟上，否则工具页还显示保存前的全局启用名单。
    // 后端保存后会广播 config_changed，那条也会触发重取——这里不等它，是为了让
    // 「点了保存，别处立刻是新的」不依赖 SSE 是否连着
    void reloadConfig()
    await refreshRuntimeDefaults()
    toast('已保存并生效', 'success')
  } catch (error) {
    toast(`保存失败：${errorText(error)}`, 'error')
  } finally {
    saving.value = false
  }
}

async function loadRaw(file: RawFile): Promise<void> {
  rawName.value = file
  try {
    rawText.value = await client.rawConfig(file)
  } catch (error) {
    rawText.value = `读取失败：${errorText(error)}`
  }
}

const workModeOptions: PickerOption[] = [
  { value: 'ask', label: '问答' },
  { value: 'edit', label: '编辑' },
  { value: 'agent', label: '代理' },
]

const apiModeOptions: PickerOption[] = [
  { value: 'completions', label: 'Completions' },
  { value: 'responses', label: 'Responses' },
]

const modelOptions = computed<PickerOption[]>(() => [
  { value: '', label: '跟随 models.toml 第一条' },
  ...models.value.map((model) => ({ value: model.id, label: `${model.name}（${model.id}）` })),
])

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
            @click="tab = item.id"
          >
            <span class="split-view__list-title">{{ item.label }}</span>
          </button>
        </div>
      </aside>

      <main class="split-view__main">
        <ListStatus
          :error="loadError"
          :loading="!loadError && !configState.config.value"
          loading-text="正在读取…"
        />

        <Transition v-if="!loadError && configState.config.value" name="lya-split" mode="out-in">
          <section v-if="tab === 'runtime'" key="runtime" class="page__pane">
          <p class="page__hint">
            写入 <code>runtime.toml</code>
          </p>

          <div class="panel form-panel">
            <h3 class="form-panel__title">对话</h3>
            <label class="field">
              <span class="field__label">默认模型</span>
              <Picker v-model="form.defaultModel" :options="modelOptions" />
            </label>
            <label class="field">
              <span class="field__label">新会话默认模式</span>
              <Picker v-model="form.defaultWorkMode" :options="workModeOptions" />
            </label>
            <label class="field">
              <span class="field__label">新会话默认 API 栈</span>
              <Picker v-model="form.defaultApiMode" :options="apiModeOptions" />
            </label>
            <label class="field">
              <span class="field__label">单轮最多调几次工具</span>
              <input v-model.number="form.maxToolRounds" class="input" type="number" min="1" max="200" />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">工具</h3>
            <p class="page__hint">新会话默认启用哪些 tool</p>
            <div class="seg-row">
              <button
                class="btn btn--sm"
                :class="{ 'btn--primary': globalMode === 'all' }"
                @click="setGlobalMode('all')"
              >
                全部启用
              </button>
              <button
                class="btn btn--sm"
                :class="{ 'btn--primary': globalMode === 'custom' }"
                @click="setGlobalMode('custom')"
              >
                自定义
              </button>
              <button
                class="btn btn--sm"
                :class="{ 'btn--primary': globalMode === 'none' }"
                @click="setGlobalMode('none')"
              >
                全部关闭
              </button>
            </div>
            <div v-if="globalMode === 'custom'" class="cfg-tool-checks">
              <label v-for="tool in catalogTools" :key="tool.name" class="cfg-tool-check">
                <input
                  type="checkbox"
                  :checked="globalEnabled.has(tool.name)"
                  @change="toggleGlobalTool(tool.name, ($event.target as HTMLInputElement).checked)"
                />
                <span>{{ tool.raw_name }}</span>
              </label>
            </div>
            <label class="field">
              <span class="field__label">同条消息并行 tool 上限</span>
              <input v-model.number="form.maxParallelTools" class="input" type="number" min="1" max="10" />
            </label>
            <label class="field">
              <span class="field__label">连续失败几次就中止（0 = 不启用）</span>
              <input
                v-model.number="form.maxConsecutiveToolFailures"
                class="input"
                type="number"
                min="0"
                max="100"
              />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">命令执行</h3>
            <label class="field">
              <span class="field__label">bash 命令确认</span>
              <Picker v-model="form.shellConfirm" :options="shellConfirmOptions" />
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
            <h3 class="form-panel__title">媒体 · 图片</h3>
            <label class="field">
              <span class="field__label">单张图片上限（MB）</span>
              <input v-model.number="form.maxImageMb" class="input" type="number" min="1" max="256" step="0.5" />
            </label>
            <label class="field field--check">
              <span class="field__label">本地图片留一份</span>
              <input v-model="form.retainLocal" type="checkbox" />
            </label>
            <label class="field field--check">
              <span class="field__label">远程图片留一份</span>
              <input v-model="form.retainWeb" type="checkbox" />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">媒体 · 视频</h3>
            <label class="field">
              <span class="field__label">单个视频上限（MB）</span>
              <input v-model.number="form.maxVideoMb" class="input" type="number" min="1" max="4096" step="1" />
            </label>
            <label class="field field--check">
              <span class="field__label">本地视频留一份</span>
              <input v-model="form.retainVideoLocal" type="checkbox" />
            </label>
            <label class="field field--check">
              <span class="field__label">远程视频留一份</span>
              <input v-model="form.retainVideoWeb" type="checkbox" />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">媒体 · 音频</h3>
            <label class="field">
              <span class="field__label">单个音频上限（MB）</span>
              <input v-model.number="form.maxAudioMb" class="input" type="number" min="1" max="1024" step="1" />
            </label>
            <label class="field field--check">
              <span class="field__label">本地音频留一份</span>
              <input v-model="form.retainAudioLocal" type="checkbox" />
            </label>
            <label class="field field--check">
              <span class="field__label">远程音频留一份</span>
              <input v-model="form.retainAudioWeb" type="checkbox" />
            </label>
          </div>

          <div class="row row--end">
            <button class="btn btn--primary" :disabled="saving" @click="saveRuntime">
              {{ saving ? '保存中…' : '保存默认配置' }}
            </button>
          </div>
        </section>

          <section v-else key="raw" class="page__pane">
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
.field--check {
  align-items: center;
}

.field--check input[type='checkbox'] {
  width: auto;
  justify-self: start;
}

.cfg-tool-checks {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 240px;
  overflow: auto;
  padding: 8px;
  margin-bottom: 8px;
  border: var(--border-width) solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
}

.cfg-tool-check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-sm);
  cursor: pointer;
}

.cfg-tool-check input {
  width: auto;
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
