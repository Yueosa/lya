/**
 * 长期记忆的共享列表与增删改。
 *
 * 三处在用，但要的东西不一样：记忆页要全套增删改查，首页和 Minecraft 主菜单只拿标题当装饰。
 * 后两处原先各自 `client.memories()` 拉一遍再 `catch { = [] }` 静默吞掉，于是同一份列表在
 * 一屏之内可能被请求三次。
 *
 * 收成一份之后还多一个好处：记忆页里增删改会**顺手更新这份共享列表**，所以退回首页时那儿
 * 显示的已经是新的，不必再拉一次也不会显示已经删掉的条目。
 *
 * 错误按 `app/errors.ts` 定的口径分：**改动**失败弹提示（用户刚按了按钮，在等回音），
 * **读取**失败留在 [`memories.error`] 里由界面就地显示（那一屏本来就是空的，位置正好用来
 * 说为什么）。
 */

import { computed, ref } from 'vue'

import { errorText, type Memory, type MemoryHit, type MemoryPatch } from '../api/client'
import { client } from './client'
import { report } from './errors'

const items = ref<Memory[]>([])
const loading = ref(false)
const error = ref('')
let inflight: Promise<void> | null = null

/** 确保列表已经读到；读过就不再请求。 */
export async function ensureMemories(): Promise<void> {
  if (items.value.length) return
  await reloadMemories()
}

/** 重新读一遍。和 [`ensureMemories`] 分开的理由同 `useConfig`：混成一个就刷不动了。 */
export function reloadMemories(): Promise<void> {
  inflight ??= (async () => {
    loading.value = true
    error.value = ''
    try {
      items.value = await client.memories()
    } catch (err) {
      error.value = errorText(err)
    } finally {
      loading.value = false
      inflight = null
    }
  })()
  return inflight
}

/** 取一条完整的。搜索结果只有摘要片段，点进去要看正文时用。 */
export async function fetchMemory(id: number): Promise<Memory | null> {
  try {
    return await client.memory(id)
  } catch (err) {
    report(err, '读取记忆')
    return null
  }
}

/** 搜索。失败返回 `null`——和「搜到 0 条」是两回事，界面要分得开。 */
export async function searchMemories(keyword: string): Promise<MemoryHit[] | null> {
  try {
    return await client.searchMemories(keyword)
  } catch (err) {
    report(err, '搜索')
    return null
  }
}

/** 新建一条只有标题的，成功后落在列表最前面。 */
export async function createMemory(title: string): Promise<Memory | null> {
  try {
    const created = await client.createMemory({ title, summary: '', body: '', tags: [] })
    items.value = [created, ...items.value]
    return created
  } catch (err) {
    report(err, '新建')
    return null
  }
}

/** 改一条，成功后就地替换。 */
export async function updateMemory(id: number, patch: MemoryPatch): Promise<Memory | null> {
  try {
    const updated = await client.updateMemory(id, patch)
    items.value = items.value.map((item) => (item.id === updated.id ? updated : item))
    return updated
  } catch (err) {
    report(err, '保存')
    return null
  }
}

/**
 * 删一条。
 *
 * 不吞异常：调用方是 `confirmAsync`，它靠抛出来把失败留在确认框里。在这儿吞掉的话框会先
 * 关掉再弹个提示，用户不确定到底删没删。
 */
export async function deleteMemory(id: number): Promise<void> {
  await client.deleteMemory(id)
  items.value = items.value.filter((item) => item.id !== id)
}

/** 只读的共享列表。 */
export const memories = {
  items: computed(() => items.value),
  loading: computed(() => loading.value),
  error: computed(() => error.value),
}
