<!--
  全局人设。

  单独一页而不是「设置」下的一个 tab：这是天天要改的正文，不是配置项。
-->

<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { errorText } from '../api/client'
import { client } from '../app/useChat'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'

const persona = ref('')
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)

onMounted(load)


async function load(): Promise<void> {
  loadError.value = ''
  try {
    persona.value = (await client.config()).persona ?? ''
  } catch (error) {
    loadError.value = errorText(error)
    toast(`读取人设失败：${loadError.value}`, 'error')
  } finally {
    loading.value = false
  }
}

async function save(): Promise<void> {
  saving.value = true
  try {
    await client.writePersona(persona.value)
    toast('人设已保存', 'success')
  } catch (error) {
    toast(`保存失败：${errorText(error)}`, 'error')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="page">
    <ViewHead title="人设" />

    <div class="page__body">
      <p v-if="loadError" class="page__error">{{ loadError }}</p>
      <p v-else-if="loading" class="split-view__hint">正在读取…</p>

      <section v-else class="page__pane">
        <p class="page__hint">
          写入 <code>persona.toml</code>，所有新会话默认继承；单个会话可以在会话设置里覆盖。
        </p>
        <textarea
          v-model="persona"
          class="input persona__text"
          rows="16"
          placeholder="想让 lya 用什么身份、什么语气跟你说话"
        />
        <div class="row row--end">
          <button class="btn btn--primary" :disabled="saving" @click="save">
            {{ saving ? '保存中…' : '保存' }}
          </button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.persona__text {
  width: 100%;
  height: auto;
  padding: 10px 12px;
  line-height: var(--leading);
  resize: vertical;
  font-family: var(--font-mono);
}
</style>
