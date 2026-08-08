<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import type { Memory, MemoryHit } from '../api/client'
import {
  createMemory,
  deleteMemory,
  ensureMemories,
  fetchMemory,
  memories,
  reloadMemories,
  searchMemories,
  updateMemory,
} from '../app/useMemories'
import ListStatus from '../ui/ListStatus.vue'
import { confirmAsync, prompt } from '../ui/useDialog'
import { toast } from '../ui/useToast'
import ViewHead from '../ui/ViewHead.vue'

// 列表是共享的：首页和 Minecraft 主菜单也拿它当装饰，这里增删改会顺手更新那份
const items = memories.items
const loadError = memories.error

const hits = ref<MemoryHit[] | null>(null)
const keyword = ref('')
const searching = ref(false)
const selected = ref<Memory | null>(null)
const editing = ref(false)
const tagsDraft = ref('')

const loading = computed(() => memories.loading.value || searching.value)

onMounted(ensureMemories)

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
  await reloadMemories()
  // 选中的那条可能已经被别处删了，对不上就放开
  if (selected.value) {
    selected.value = items.value.find((m) => m.id === selected.value!.id) ?? null
  }
}

async function search(): Promise<void> {
  const q = keyword.value.trim()
  if (!q) {
    hits.value = null
    return refresh()
  }
  searching.value = true
  const found = await searchMemories(q)
  searching.value = false
  // 搜失败返回 null，和「搜到 0 条」是两回事：前者保持原样，别把结果区变成「没找到」
  if (found) hits.value = found
}

async function create(): Promise<void> {
  const title = await prompt({ title: '新建记忆', placeholder: '标题' })
  if (title === null || !title.trim()) return
  const created = await createMemory(title.trim())
  if (!created) return
  select(created)
  editing.value = true
}

function select(memory: Memory): void {
  selected.value = { ...memory }
  tagsDraft.value = memory.tags.join(', ')
  editing.value = false
}

async function selectHit(hit: MemoryHit): Promise<void> {
  searching.value = true
  const full = await fetchMemory(hit.id)
  searching.value = false
  if (full) select(full)
}

async function save(): Promise<void> {
  const draft = selected.value
  if (!draft) return
  const updated = await updateMemory(draft.id, {
    title: draft.title,
    summary: draft.summary,
    body: draft.body,
    tags: parseTags(tagsDraft.value),
  })
  if (!updated) return
  selected.value = { ...updated }
  tagsDraft.value = updated.tags.join(', ')
  editing.value = false
  toast('已保存', 'success')
}

async function remove(): Promise<void> {
  const memory = selected.value
  if (!memory) return
  await confirmAsync({
    title: `删除「${memory.title}」`,
    message: '不可恢复',
    confirmText: '删除',
    danger: true,
    // 不吞异常：失败要留在确认框里，框先关掉再弹提示的话用户不确定到底删没删
    run: async () => {
      await deleteMemory(memory.id)
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
          <!-- 读取失败就地说明，不弹提示：这一栏本来就是空的，位置正好用来说为什么，
               弹窗几秒就飘走了，回头看到空列表也不知道是没有还是没读到 -->
          <ListStatus
            :error="loadError"
            :loading="loading"
            :empty="!loading && !loadError && (hits ? hits.length === 0 : items.length === 0)"
            empty-text="暂无记忆"
          />
          <template v-if="!loadError && !loading && hits">
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
          <template v-else-if="!loadError && !loading">
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
