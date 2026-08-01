<!--
  全局配置。

  三层里 **core 只读**：端口、日志级别这些改了要重启才生效，在界面上给个能改却
  不生效的输入框比不给更糟。runtime 和人设可写，模型清单只读加一个连通性探测。

  写回走后端的 `toml_edit`，注释和排版都保得住——手写过的配置文件不该因为在
  界面上点了一下就被格式化成另一副样子。
-->

<script setup lang="ts">
import { onMounted, ref } from 'vue'

import type { ConfigView as Config, ProbeResult } from '../api/client'
import { client, loadModels } from '../app/useChat'
import { toast } from '../ui/useToast'

const config = ref<Config | null>(null)
const persona = ref('')
const runtimeText = ref('')
const savingPersona = ref(false)
const savingRuntime = ref(false)
const probing = ref<string | null>(null)
const probeResult = ref<Map<string, ProbeResult>>(new Map())
const rawFile = ref<{ name: string; text: string } | null>(null)

onMounted(load)

async function load(): Promise<void> {
  try {
    const data = await client.config()
    config.value = data
    persona.value = data.persona ?? ''
    runtimeText.value = JSON.stringify(data.runtime, null, 2)
  } catch (error) {
    toast(`读取配置失败：${error instanceof Error ? error.message : error}`, 'error')
  }
}

async function savePersona(): Promise<void> {
  savingPersona.value = true
  try {
    await client.writePersona(persona.value)
    toast('人设已保存', 'success')
  } catch (error) {
    toast(`保存失败：${error instanceof Error ? error.message : error}`, 'error')
  } finally {
    savingPersona.value = false
  }
}

async function saveRuntime(): Promise<void> {
  let tables: Record<string, unknown>
  try {
    tables = JSON.parse(runtimeText.value)
  } catch {
    toast('这段 JSON 有语法错误，先改好再保存', 'error')
    return
  }
  savingRuntime.value = true
  try {
    // 后端写完会回读一次验证，返回的是生效后的值而不是我们发过去那份
    const applied = await client.writeRuntime(tables)
    runtimeText.value = JSON.stringify(applied, null, 2)
    toast('已保存并生效', 'success')
  } catch (error) {
    toast(`保存失败：${error instanceof Error ? error.message : error}`, 'error')
  } finally {
    savingRuntime.value = false
  }
}

/**
 * 探一下这个模型通不通。
 *
 * 后端拿服务器上存的真密钥去打一次 `GET /models`，界面这边始终只看得到脱敏后的
 * 那串——密钥不该为了测一下就发到浏览器里再发回去。
 */
async function probe(modelId: string): Promise<void> {
  probing.value = modelId
  try {
    const result = await client.probeModel(modelId)
    probeResult.value = new Map(probeResult.value).set(modelId, result)
  } catch (error) {
    probeResult.value = new Map(probeResult.value).set(modelId, {
      ok: false,
      models: [],
      error: error instanceof Error ? error.message : String(error),
    })
  } finally {
    probing.value = null
    void loadModels()
  }
}

async function showRaw(file: 'core' | 'runtime' | 'models' | 'persona'): Promise<void> {
  try {
    rawFile.value = { name: file, text: await client.rawConfig(file) }
  } catch (error) {
    toast(`读取失败：${error instanceof Error ? error.message : error}`, 'error')
  }
}
</script>

<template>
  <div class="cfg">
    <h2 class="cfg__title">设置</h2>
    <p v-if="!config" class="cfg__hint">正在读取…</p>

    <template v-else>
      <section class="panel cfg__card">
        <h3 class="cfg__section">人设</h3>
        <p class="cfg__hint">
          它排在系统提示词最后，只影响说话的样子，不能改变行为准则与工具权限。
        </p>
        <textarea v-model="persona" class="input cfg__text" rows="4" />
        <div class="cfg__actions">
          <button class="btn btn--primary" :disabled="savingPersona" @click="savePersona">
            {{ savingPersona ? '保存中…' : '保存' }}
          </button>
        </div>
      </section>

      <section class="panel cfg__card">
        <h3 class="cfg__section">模型</h3>
        <p class="cfg__hint">
          密钥存在 <code>~/.lya/models.toml</code>（权限 0600），界面上只看得到末几位。
        </p>
        <div v-for="model in config.models" :key="model.id" class="cfg__model">
          <div class="cfg__row">
            <strong>{{ model.name }}</strong>
            <code class="cfg__dim">{{ model.id }}</code>
            <span class="cfg__gap" />
            <code class="cfg__dim">{{ model.api_key_masked }}</code>
            <button class="btn btn--sm" :disabled="probing === model.id" @click="probe(model.id)">
              {{ probing === model.id ? '测试中…' : '测一下' }}
            </button>
          </div>
          <p class="cfg__hint">
            {{ model.base_url }} · 能力：{{ model.capabilities.join('、') || '未标注' }}
          </p>
          <p v-if="model.api_key_placeholder" class="cfg__warn">
            密钥还是模板里的占位符，这个模型用不了。
          </p>
          <p v-if="probeResult.get(model.id)" class="cfg__hint">
            <template v-if="probeResult.get(model.id)!.ok">
              ✓ 通了，对方声明支持 {{ probeResult.get(model.id)!.models.length }} 个模型
            </template>
            <span v-else class="cfg__warn">✕ {{ probeResult.get(model.id)!.error }}</span>
          </p>
        </div>
      </section>

      <section class="panel cfg__card">
        <h3 class="cfg__section">运行时</h3>
        <p class="cfg__hint">
          各模块的默认值。写回时用 <code>toml_edit</code>，你在文件里写的注释不会丢。
        </p>
        <textarea v-model="runtimeText" class="input cfg__text cfg__code" rows="14" />
        <div class="cfg__actions">
          <button class="btn btn--primary" :disabled="savingRuntime" @click="saveRuntime">
            {{ savingRuntime ? '保存中…' : '保存' }}
          </button>
        </div>
      </section>

      <section class="panel cfg__card">
        <h3 class="cfg__section">核心（只读）</h3>
        <p class="cfg__hint">
          端口、日志级别这些改了要重启才生效，所以界面上不给改——给一个点了不算数的
          输入框比不给更糟。要改就直接编辑 <code>~/.lya/core.toml</code>。
        </p>
        <pre class="cfg__readonly">{{ JSON.stringify(config.core, null, 2) }}</pre>
      </section>

      <section class="panel cfg__card">
        <h3 class="cfg__section">原始文件</h3>
        <div class="cfg__row">
          <button
            v-for="file in (['core', 'runtime', 'models', 'persona'] as const)"
            :key="file"
            class="btn btn--sm"
            @click="showRaw(file)"
          >
            {{ file }}.toml
          </button>
        </div>
      </section>
    </template>

    <div v-if="rawFile" class="overlay" @click.self="rawFile = null">
      <div class="dialog cfg__raw">
        <h3 class="dialog__title">{{ rawFile.name }}.toml</h3>
        <pre class="cfg__readonly">{{ rawFile.text }}</pre>
        <div class="dialog__actions">
          <button class="btn" @click="rawFile = null">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cfg {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-width: 860px;
}

.cfg__title {
  margin: 0;
  font-size: var(--text-lg);
}

.cfg__card {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.cfg__section {
  margin: 0;
  font-size: var(--text-md);
}

.cfg__hint {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.cfg__warn {
  color: var(--danger);
}

.cfg__dim {
  color: var(--text-faint);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.cfg__row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.cfg__gap {
  flex: 1;
}

.cfg__model {
  padding: 8px 0;
  border-top: var(--border-width) solid var(--border);
}

.cfg__model:first-of-type {
  border-top: none;
}

.cfg__text {
  height: auto;
  padding: 8px 12px;
  resize: vertical;
  line-height: var(--leading);
}

.cfg__code {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.cfg__readonly {
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--bg-sunken);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  max-height: 380px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.cfg__actions {
  display: flex;
  justify-content: flex-end;
}

.cfg__raw {
  width: 720px;
}
</style>
