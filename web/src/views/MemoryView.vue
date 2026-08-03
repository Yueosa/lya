<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'

import { client } from '../app/useChat'
import type { Memory, MemoryHit } from '../api/client'
import { confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'

const items = ref<Memory[]>([])
const hits = ref<MemoryHit[] | null>(null)
const keyword = ref('')
const loading = ref(false)
const selected = ref<Memory | null>(null)
const editing = ref(false)
const tagsDraft = ref('')

onMounted(refresh)

watch(keyword, (value) => {
  if (!value.trim()) {
    hits.value = null
    void refresh()
  }
})

const FIELD_LABELS: Record<string, string> = {
  title: '标题',
  summary: '摘要',
  body: '正文',
  tag: '标签',
  tags: '标签',
}

function fieldLabel(field: string): string {
  return FIELD_LABELS[field] ?? field
}

async function refresh(): Promise<void> {
  loading.value = true
  try {
    items.value = await client.memories()
    if (selected.value) {
      selected.value = items.value.find((m) => m.id === selected.value!.id) ?? null
    }
  } catch {
    toast('读取失败', 'error')
  } finally {
    loading.value = false
  }
}

async function search(): Promise<void> {
  const q = keyword.value.trim()
  if (!q) {
    hits.value = null
    return refresh()
  }
  loading.value = true
  try {
    hits.value = await client.searchMemories(q)
  } catch {
    toast('搜索失败', 'error')
  } finally {
    loading.value = false
  }
}

async function create(): Promise<void> {
  const title = await prompt({ title: '新建记忆', placeholder: '标题' })
  if (title === null || !title.trim()) return
  try {
    const created = await client.createMemory({ title: title.trim(), summary: '', body: '', tags: [] })
    items.value = [created, ...items.value]
    select(created)
    editing.value = true
  } catch (error) {
    toast(`新建失败：${error instanceof Error ? error.message : error}`, 'error')
  }
}

function select(memory: Memory): void {
  selected.value = { ...memory }
  tagsDraft.value = memory.tags.join(', ')
  editing.value = false
}

async function selectHit(hit: MemoryHit): Promise<void> {
  loading.value = true
  try {
    const full = await client.memory(hit.id)
    select(full)
  } catch {
    toast('读取记忆失败', 'error')
  } finally {
    loading.value = false
  }
}

async function save(): Promise<void> {
  const draft = selected.value
  if (!draft) return
  try {
    const updated = await client.updateMemory(draft.id, {
      title: draft.title,
      summary: draft.summary,
      body: draft.body,
      tags: parseTags(tagsDraft.value),
    })
    items.value = items.value.map((item) => (item.id === updated.id ? updated : item))
    selected.value = { ...updated }
    tagsDraft.value = updated.tags.join(', ')
    editing.value = false
    toast('已保存', 'success')
  } catch (error) {
    toast(`保存失败：${error instanceof Error ? error.message : error}`, 'error')
  }
}

async function remove(): Promise<void> {
  const memory = selected.value
  if (!memory) return
  await confirmAsync({
    title: `删除「${memory.title}」`,
    message: '不可恢复',
    confirmText: '删除',
    danger: true,
    run: async () => {
      await client.deleteMemory(memory.id)
      items.value = items.value.filter((item) => item.id !== memory.id)
      selected.value = null
    },
  })
}

function parseTags(text: string): string[] {
  return text
    .split(/[,，、]/)
    .map((tag) => tag.trim())
    .filter(Boolean)
}
</script>

<template>
  <div class="split-view">
    <ViewHead title="记忆">
      <template #actions>
        <button class="btn btn--sm" @click="refresh">刷新</button>
      </template>
    </ViewHead>

    <div class="split-view__body">
      <aside class="split-view__list">
        <div class="split-view__list-toolbar">
          <input
            v-model="keyword"
            class="input"
            placeholder="搜索…"
            @keydown.enter="search"
          />
          <button class="btn btn--sm" @click="search">搜索</button>
          <button class="btn btn--sm btn--primary" @click="create">新建</button>
        </div>

        <div class="split-view__list-scroll">
          <p v-if="loading" class="split-view__hint">加载中…</p>
          <template v-if="hits">
            <button
              v-for="hit in hits"
              :key="hit.id"
              class="split-view__list-item"
              :class="{ 'split-view__list-item--on': selected?.id === hit.id }"
              @click="selectHit(hit)"
            >
              <span class="split-view__list-title">{{ hit.title }}</span>
              <span class="split-view__list-meta">命中 · {{ fieldLabel(hit.matched_in) }}</span>
            </button>
          </template>
          <template v-else>
            <button
              v-for="memory in items"
              :key="memory.id"
              class="split-view__list-item"
              :class="{ 'split-view__list-item--on': selected?.id === memory.id }"
              @click="select(memory)"
            >
              <span class="split-view__list-title">{{ memory.title }}</span>
              <span v-if="memory.tags.length" class="split-view__list-meta">{{ memory.tags.slice(0, 3).join(' · ') }}</span>
            </button>
          </template>
          <p v-if="!loading && (hits ? hits.length === 0 : items.length === 0)" class="split-view__hint">暂无记忆</p>
        </div>
      </aside>

      <main class="split-view__main">
        <Transition name="lya-split" mode="out-in">
          <div v-if="!selected" key="_empty" class="split-view__empty">选择一条记忆</div>
          <div v-else :key="selected.id" class="page__pane">
          <header class="mem__detail-head">
            <h3>{{ selected.title }}</h3>
            <div class="mem__detail-actions">
              <button v-if="!editing" class="btn btn--sm" @click="editing = true">编辑</button>
              <template v-else>
                <button class="btn btn--sm btn--primary" @click="save">保存</button>
                <button class="btn btn--sm" @click="editing = false">取消</button>
              </template>
              <button class="btn btn--sm btn--danger" @click="remove">删除</button>
            </div>
          </header>

          <template v-if="editing">
            <div class="content-stack">
              <label class="mem__field">
                <span>标题</span>
                <input v-model="selected.title" class="input" />
              </label>
              <label class="mem__field">
                <span>摘要</span>
                <input v-model="selected.summary" class="input" />
              </label>
              <label class="mem__field">
                <span>标签</span>
                <input v-model="tagsDraft" class="input" placeholder="逗号分隔" />
              </label>
              <label class="mem__field">
                <span>正文</span>
                <textarea v-model="selected.body" class="input mem__body" rows="12" />
              </label>
            </div>
          </template>

          <template v-else>
            <section v-if="selected.summary" class="detail-section">
              <h4 class="detail-section__title">摘要</h4>
              <p class="prose">{{ selected.summary }}</p>
            </section>
            <section class="detail-section">
              <h4 class="detail-section__title">标签</h4>
              <div v-if="selected.tags.length" class="seg-row">
                <span v-for="tag in selected.tags" :key="tag" class="mem__tag">{{ tag }}</span>
              </div>
              <p v-else class="prose muted">（无）</p>
            </section>
            <section class="detail-section">
              <h4 class="detail-section__title">正文</h4>
              <pre class="pre-block">{{ selected.body || '（空）' }}</pre>
            </section>
          </template>
          </div>
        </Transition>
      </main>
    </div>
  </div>
</template>

<style scoped>
.mem__detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 0;
}

.mem__detail-head h3 {
  margin: 0;
  flex: 1;
  font-size: var(--text-md);
}

.mem__detail-actions {
  display: flex;
  gap: 6px;
}

.mem__field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.mem__field span {
  font-weight: 600;
  color: var(--text-muted);
}

.mem__body {
  height: auto;
  padding: 8px 12px;
  resize: vertical;
  font-family: var(--font-mono);
}

.mem__tag {
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  background: var(--accent-soft);
  font-size: var(--text-xs);
}
</style>
