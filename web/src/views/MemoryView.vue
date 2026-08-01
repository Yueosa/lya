<!--
  长期记忆。

  模型只能读和写，**删除只走这里**——让它自己删记忆太容易一句话抹掉你要它记住
  的东西。这一页是那道人工闸门。

  摘要单独一栏是因为它进常驻提示词：正文按需读取，摘要每轮都在模型眼前。写得
  含糊的话，模型根本不知道该不该去读正文。
-->

<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { client } from '../app/useChat'
import type { Memory } from '../api/client'
import { confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'

const items = ref<Memory[]>([])
const keyword = ref('')
const loading = ref(false)
const editing = ref<Memory | null>(null)
/** 命中的字段，搜索结果才有。 */
const hitField = ref<Map<number, string>>(new Map())

onMounted(refresh)

async function refresh(): Promise<void> {
  loading.value = true
  hitField.value = new Map()
  try {
    items.value = await client.memories()
  } catch {
    toast('读取记忆失败', 'error')
  } finally {
    loading.value = false
  }
}

async function search(): Promise<void> {
  const q = keyword.value.trim()
  if (!q) return refresh()
  loading.value = true
  try {
    const hits = await client.searchMemories(q)
    items.value = hits.map((hit) => hit.memory)
    hitField.value = new Map(hits.map((hit) => [hit.memory.id, hit.field]))
  } catch {
    toast('搜索失败', 'error')
  } finally {
    loading.value = false
  }
}

async function create(): Promise<void> {
  const title = await prompt({ title: '新建记忆', placeholder: '起个标题，全局唯一' })
  if (title === null || !title.trim()) return
  try {
    const created = await client.createMemory({
      title: title.trim(),
      summary: '',
      body: '',
      tags: [],
    })
    items.value = [created, ...items.value]
    editing.value = created
  } catch (error) {
    toast(`新建失败：${error instanceof Error ? error.message : error}`, 'error')
  }
}

async function save(): Promise<void> {
  const draft = editing.value
  if (!draft) return
  try {
    const updated = await client.updateMemory(draft.id, {
      title: draft.title,
      summary: draft.summary,
      body: draft.body,
      tags: draft.tags,
    })
    items.value = items.value.map((item) => (item.id === updated.id ? updated : item))
    editing.value = null
    toast('已保存', 'success')
  } catch (error) {
    toast(`保存失败：${error instanceof Error ? error.message : error}`, 'error')
  }
}

async function remove(memory: Memory): Promise<void> {
  await confirmAsync({
    title: `删除「${memory.title}」？`,
    message: '模型以后就想不起这件事了，不可恢复。',
    confirmText: '删除',
    danger: true,
    run: async () => {
      await client.deleteMemory(memory.id)
      items.value = items.value.filter((item) => item.id !== memory.id)
      if (editing.value?.id === memory.id) editing.value = null
    },
  })
}

/** 编辑时标签用逗号分隔，比让人填 JSON 数组友好。 */
function tagsText(memory: Memory): string {
  return memory.tags.join('、')
}

function setTags(memory: Memory, text: string): void {
  memory.tags = text
    .split(/[、,，]/)
    .map((tag) => tag.trim())
    .filter(Boolean)
}
</script>

<template>
  <div class="mem">
    <header class="mem__head">
      <h2 class="mem__title">记忆</h2>
      <input
        v-model="keyword"
        class="input mem__search"
        placeholder="搜标题、摘要、正文、标签…"
        @keydown.enter="search"
      />
      <button class="btn" @click="search">搜索</button>
      <button class="btn btn--primary" @click="create">新建</button>
    </header>

    <p v-if="loading" class="mem__hint">正在读取…</p>
    <p v-else-if="items.length === 0" class="mem__hint">
      还没有记忆。模型会在对话里自己记下值得记的事，你也可以手动加。
    </p>

    <ul class="mem__list">
      <li v-for="memory in items" :key="memory.id" class="panel mem__item">
        <div class="mem__row">
          <span class="mem__id">#{{ memory.id }}</span>
          <strong class="mem__name">{{ memory.title }}</strong>
          <span v-if="hitField.get(memory.id)" class="mem__hit">
            命中 {{ hitField.get(memory.id) }}
          </span>
          <span class="mem__gap" />
          <button class="btn btn--sm" @click="editing = { ...memory }">编辑</button>
          <button class="btn btn--sm btn--danger" @click="remove(memory)">删除</button>
        </div>
        <p class="mem__summary">{{ memory.summary || '（没有摘要，模型看不出它值不值得读）' }}</p>
        <div v-if="memory.tags.length" class="mem__tags">
          <span v-for="tag in memory.tags" :key="tag" class="mem__tag">{{ tag }}</span>
        </div>
      </li>
    </ul>

    <!-- 编辑 -->
    <div v-if="editing" class="overlay" @click.self="editing = null">
      <div class="dialog mem__editor">
        <h3 class="dialog__title">#{{ editing.id }}</h3>

        <label class="mem__field">
          <span>标题</span>
          <input v-model="editing.title" class="input" />
        </label>

        <label class="mem__field">
          <span>摘要</span>
          <input v-model="editing.summary" class="input" placeholder="一句话，会常驻在提示词里" />
        </label>

        <label class="mem__field">
          <span>标签</span>
          <input
            class="input"
            :value="tagsText(editing)"
            placeholder="用顿号或逗号分隔"
            @input="setTags(editing, ($event.target as HTMLInputElement).value)"
          />
        </label>

        <label class="mem__field">
          <span>正文</span>
          <textarea v-model="editing.body" class="input mem__body" rows="10" />
        </label>

        <div class="dialog__actions">
          <button class="btn" @click="editing = null">取消</button>
          <button class="btn btn--primary" @click="save">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mem {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.mem__head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mem__title {
  margin: 0;
  font-size: var(--text-lg);
}

.mem__search {
  flex: 1;
  max-width: 340px;
}

.mem__hint {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.mem__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.mem__item {
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mem__row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mem__id {
  color: var(--text-faint);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.mem__name {
  font-size: var(--text-md);
}

.mem__hit {
  color: var(--info);
  font-size: var(--text-xs);
}

.mem__gap {
  flex: 1;
}

.mem__summary {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.mem__tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.mem__tag {
  padding: 1px 8px;
  border-radius: var(--radius-pill);
  background: var(--accent-soft);
  color: var(--text-muted);
  font-size: var(--text-xs);
}

.mem__editor {
  width: 620px;
}

.mem__field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.mem__body {
  height: auto;
  padding: 8px 12px;
  resize: vertical;
  font-family: var(--font-mono);
  line-height: var(--leading);
}
</style>
