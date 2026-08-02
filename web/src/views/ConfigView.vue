<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'

import type { ConfigView as Config, UsageReport } from '../api/client'
import { client, refreshRuntimeDefaults } from '../app/useChat'
import Picker from '../ui/Picker.vue'
import type { PickerOption } from '../ui/Picker.vue'
import RawToml from '../ui/RawToml.vue'
import StoragePie from '../ui/StoragePie.vue'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'
import { bytesToMegabytes, megabytesToBytes } from '../utils/formatBytes'

type Tab = 'persona' | 'runtime' | 'storage' | 'raw'
type RawFile = 'core' | 'runtime' | 'models' | 'persona'

const TABS: { id: Tab; label: string }[] = [
  { id: 'persona', label: '人设' },
  { id: 'runtime', label: '运行时' },
  { id: 'storage', label: '存储' },
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
const storage = ref<UsageReport | null>(null)
const storageError = ref('')
const storageLoading = ref(false)

const form = ref({
  maxToolRounds: 32,
  defaultWorkMode: 'agent',
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
  const media = (runtime['media'] ?? {}) as Record<string, unknown>
  const image = (media['image'] ?? {}) as Record<string, unknown>
  const video = (media['video'] ?? {}) as Record<string, unknown>
  const audio = (media['audio'] ?? {}) as Record<string, unknown>
  form.value = {
    maxToolRounds: Number(agent['max_tool_rounds'] ?? 32),
    defaultWorkMode: String(agent['default_work_mode'] ?? 'agent'),
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

async function loadStorage(): Promise<void> {
  storageLoading.value = true
  storageError.value = ''
  try {
    storage.value = await client.storageStats()
  } catch (error) {
    storage.value = null
    storageError.value = errMsg(error)
  } finally {
    storageLoading.value = false
  }
}

function pickTab(id: Tab): void {
  tab.value = id
  if (id === 'raw') void loadRaw(rawName.value)
  if (id === 'storage') void loadStorage()
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
  if (id === 'storage') void loadStorage()
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
            <h3 class="form-panel__title">媒体 · 图片</h3>
            <p class="page__hint">聊天图片与会话 <code>img_cache</code>；保存后立即生效。</p>
            <label class="field">
              <span class="field__label">单张图片上限（MB）</span>
              <input v-model.number="form.maxImageMb" class="input" type="number" min="1" max="256" step="0.5" />
              <p class="field__note">同时作用于 local-image 与会话 media 端点</p>
            </label>
            <label class="field field--check">
              <span class="field__label">缓存本地图片</span>
              <input v-model="form.cacheLocal" type="checkbox" />
              <p class="field__note">关闭后仍可读原路径，但不写入 img_cache/local</p>
            </label>
            <label class="field field--check">
              <span class="field__label">缓存远程图片</span>
              <input v-model="form.cacheWeb" type="checkbox" />
              <p class="field__note">关闭后每次访问重新拉取，不写入持久 web 缓存</p>
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">媒体 · 视频</h3>
            <p class="page__hint">聊天 Markdown 视频与会话 <code>vdo_cache</code>。</p>
            <label class="field">
              <span class="field__label">单个视频上限（MB）</span>
              <input v-model.number="form.maxVideoMb" class="input" type="number" min="1" max="4096" step="1" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存本地视频</span>
              <input v-model="form.cacheVideoLocal" type="checkbox" />
              <p class="field__note">关闭后仍可读原路径，但不写入 vdo_cache/local</p>
            </label>
            <label class="field field--check">
              <span class="field__label">缓存远程视频</span>
              <input v-model="form.cacheVideoWeb" type="checkbox" />
              <p class="field__note">关闭后每次播放重新拉取，不写入 vdo_cache/web</p>
            </label>
          </div>

          <div class="panel form-panel">
            <h3 class="form-panel__title">媒体 · 音频</h3>
            <p class="page__hint">聊天 Markdown 音频与会话 <code>ado_cache</code>。</p>
            <label class="field">
              <span class="field__label">单个音频上限（MB）</span>
              <input v-model.number="form.maxAudioMb" class="input" type="number" min="1" max="1024" step="1" />
            </label>
            <label class="field field--check">
              <span class="field__label">缓存本地音频</span>
              <input v-model="form.cacheAudioLocal" type="checkbox" />
              <p class="field__note">关闭后仍可读原路径，但不写入 ado_cache/local</p>
            </label>
            <label class="field field--check">
              <span class="field__label">缓存远程音频</span>
              <input v-model="form.cacheAudioWeb" type="checkbox" />
              <p class="field__note">关闭后每次播放重新拉取，不写入 ado_cache/web</p>
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

          <section v-else-if="tab === 'storage'" key="storage" class="page__pane">
          <p class="page__hint">
            只读统计 <code>~/.lya</code> 占用；第一版不提供清除按钮。
          </p>
          <p v-if="storageLoading" class="split-view__hint">正在扫描…</p>
          <p v-else-if="storageError" class="page__error">{{ storageError }}</p>
          <template v-else-if="storage">
            <p class="page__hint">数据目录：<code>{{ storage.root }}</code></p>
            <StoragePie :categories="storage.categories" :total-bytes="storage.total_bytes" />
            <div class="row row--end">
              <button class="btn btn--sm" @click="loadStorage">刷新</button>
            </div>
          </template>
        </section>

          <section v-else key="raw" class="page__pane">
          <p class="page__hint">
            core 只读——改端口等需重启进程才生效。models 里
            <code>context_window</code> 是 lya 输入预算；<code>max_tokens</code> 等透传键会原样进 API 请求体。
            若缺少 <code>[media.*]</code>，请对照模板合并 <code>runtime.toml</code>。
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

.field--check {
  align-items: center;
}

.field--check input[type='checkbox'] {
  width: auto;
  justify-self: start;
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
