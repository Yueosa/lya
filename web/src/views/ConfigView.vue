<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { ConfigView as Config, ToolInfo } from '../api/client'
import { client, models, refreshRuntimeDefaults } from '../app/useChat'
import Picker from '../ui/Picker.vue'
import type { PickerOption } from '../ui/Picker.vue'
import RawToml from '../ui/RawToml.vue'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'
import {
  buildToolsEnabledPayload,
  readGlobalToolsMode,
  type GlobalToolsMode,
} from '../utils/toolLimits'
import { bytesToMegabytes, megabytesToBytes } from '../utils/formatBytes'

type Tab = 'runtime' | 'raw'
type RawFile = 'core' | 'runtime' | 'models' | 'persona'

const TABS: { id: Tab; label: string }[] = [
  { id: 'runtime', label: '默认配置' },
  { id: 'raw', label: '原始文件' },
]

const RAW_FILES: RawFile[] = ['core', 'runtime', 'models', 'persona']

const tab = ref<Tab>('runtime')
const config = ref<Config | null>(null)
const loadError = ref('')
const saving = ref(false)
const rawName = ref<RawFile>('runtime')
const rawText = ref('')

const form = ref({
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
  cacheLocal: true,
  cacheWeb: true,
  maxVideoMb: 512,
  cacheVideoLocal: true,
  cacheVideoWeb: true,
  maxAudioMb: 128,
  cacheAudioLocal: true,
  cacheAudioWeb: true,
})

const catalogTools = ref<ToolInfo[]>([])
const globalMode = ref<GlobalToolsMode>('all')
const globalEnabled = ref<Set<string>>(new Set())

onMounted(load)

async function load(): Promise<void> {
  loadError.value = ''
  try {
    const [data, toolList] = await Promise.all([client.config(), client.tools()])
    config.value = data
    catalogTools.value = toolList
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
  const media = (runtime['media'] ?? {}) as Record<string, unknown>
  const image = (media['image'] ?? {}) as Record<string, unknown>
  const video = (media['video'] ?? {}) as Record<string, unknown>
  const audio = (media['audio'] ?? {}) as Record<string, unknown>
  const { mode, enabled } = readGlobalToolsMode(runtime)
  globalMode.value = mode
  globalEnabled.value = new Set(enabled)
  form.value = {
    maxToolRounds: Number(agent['max_tool_rounds'] ?? 32),
    maxParallelTools: Number(agent['max_parallel_tools'] ?? 3),
    maxConsecutiveToolFailures: Number(agent['max_consecutive_tool_failures'] ?? 16),
    defaultWorkMode: String(agent['default_work_mode'] ?? 'agent'),
    defaultApiMode: agent['default_api_mode'] === 'responses' ? 'responses' : 'completions',
    // 没配就是空串，对应「跟随清单第一条」；保存时会写回 null 把这个键删掉
    defaultModel: String(agent['default_model'] ?? ''),
    maxIndexEntries: Number(memory['max_index_entries'] ?? 100),
    maxIndexChars: Number(memory['max_index_chars'] ?? 4000),
    indexSummaryChars: Number(memory['index_summary_chars'] ?? 120),
    shellConfirm: String(shell['confirm'] ?? 'unknown'),
    maxImageMb: bytesToMegabytes(Number(image['max_bytes'] ?? 32 * 1024 * 1024)),
    cacheLocal: image['cache_local'] !== false,
    cacheWeb: image['cache_web'] !== false,
    maxVideoMb: bytesToMegabytes(Number(video['max_bytes'] ?? 512 * 1024 * 1024)),
    cacheVideoLocal: video['cache_local'] !== false,
    cacheVideoWeb: video['cache_web'] !== false,
    maxAudioMb: bytesToMegabytes(Number(audio['max_bytes'] ?? 128 * 1024 * 1024)),
    cacheAudioLocal: audio['cache_local'] !== false,
    cacheAudioWeb: audio['cache_web'] !== false,
  }
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
    const applied = (await client.writeRuntime({
      agent: {
        max_tool_rounds: form.value.maxToolRounds,
        max_parallel_tools: form.value.maxParallelTools,
        max_consecutive_tool_failures: form.value.maxConsecutiveToolFailures,
        default_work_mode: form.value.defaultWorkMode,
        default_api_mode: form.value.defaultApiMode,
        // null 会让后端删掉这个键；空串是非法 id，会被启动校验拦下来
        default_model: form.value.defaultModel || null,
      },
      tools: buildToolsEnabledPayload(globalMode.value, globalEnabled.value),
      memory: {
        max_index_entries: form.value.maxIndexEntries,
        max_index_chars: form.value.maxIndexChars,
        index_summary_chars: form.value.indexSummaryChars,
      },
      shell: { confirm: form.value.shellConfirm },
      media: {
        image: {
          max_bytes: megabytesToBytes(form.value.maxImageMb),
          cache_local: form.value.cacheLocal,
          cache_web: form.value.cacheWeb,
        },
        video: {
          max_bytes: megabytesToBytes(form.value.maxVideoMb),
          cache_local: form.value.cacheVideoLocal,
          cache_web: form.value.cacheVideoWeb,
        },
        audio: {
          max_bytes: megabytesToBytes(form.value.maxAudioMb),
          cache_local: form.value.cacheAudioLocal,
          cache_web: form.value.cacheAudioWeb,
        },
      },
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

async function loadRaw(file: RawFile): Promise<void> {
  rawName.value = file
  try {
    rawText.value = await client.rawConfig(file)
  } catch (error) {
    rawText.value = `读取失败：${errMsg(error)}`
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
        <p v-if="loadError" class="page__error">{{ loadError }}</p>
        <p v-else-if="!config" class="split-view__hint">正在读取…</p>

        <Transition v-else name="lya-split" mode="out-in">
          <section v-if="tab === 'runtime'" key="runtime" class="page__pane">
          <p class="page__hint">
            写入 <code>runtime.toml</code>
          </p>

          <div class="panel form-panel">
            <h3 class="form-panel__title">对话</h3>
            <label class="field">
              <span class="field__label">默认模型</span>
              <Picker v-model="form.defaultModel" :options="modelOptions" />
              <span class="field__note">会话没单独指定模型时用它；在「模型」页维护清单</span>
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
            <p class="page__hint">聊天图片缓存</p>
            <label class="field">
              <span class="field__label">单张图片上限（MB）</span>
              <input v-model.number="form.maxImageMb" class="input" type="number" min="1" max="256" step="0.5" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存本地图片</span>
              <input v-model="form.cacheLocal" type="checkbox" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存远程图片</span>
              <input v-model="form.cacheWeb" type="checkbox" />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">媒体 · 视频</h3>
            <p class="page__hint">聊天视频缓存</p>
            <label class="field">
              <span class="field__label">单个视频上限（MB）</span>
              <input v-model.number="form.maxVideoMb" class="input" type="number" min="1" max="4096" step="1" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存本地视频</span>
              <input v-model="form.cacheVideoLocal" type="checkbox" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存远程视频</span>
              <input v-model="form.cacheVideoWeb" type="checkbox" />
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">媒体 · 音频</h3>
            <p class="page__hint">聊天音频缓存</p>
            <label class="field">
              <span class="field__label">单个音频上限（MB）</span>
              <input v-model.number="form.maxAudioMb" class="input" type="number" min="1" max="1024" step="1" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存本地音频</span>
              <input v-model="form.cacheAudioLocal" type="checkbox" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存远程音频</span>
              <input v-model="form.cacheAudioWeb" type="checkbox" />
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
