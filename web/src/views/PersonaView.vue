<!--
  默认人设：新会话的起点。

  单独一页而不是「设置」下的一个 tab：这是天天要改的正文，不是配置项。

  它**不作用于已有会话**。人设是会话级的，每个会话在创建时从这儿抄一份，之后各自独立。
  反过来做（所有会话每轮都来读这一份）曾经是这里的实现，而那意味着改一次人设会把每段
  正在进行的对话都换掉性格——上面几十条聊天记录还是旧性格写的，模型下一轮得同时扮演
  两个人。所以这一页的措辞要把「只影响新会话」说在明面上，别让人以为改完全都变。
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
    <ViewHead title="默认人设" />

    <div class="page__body">
      <p v-if="loadError" class="page__error">{{ loadError }}</p>
      <p v-else-if="loading" class="split-view__hint">正在读取…</p>

      <section v-else class="page__pane">
        <p class="page__hint">
          写入 <code>persona.toml</code>，只作为<strong>新会话</strong>的起点。已有会话各自
          留着自己的一份，改这里不会动它们——要改某段对话的人设，去那个会话的设置里改。
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
