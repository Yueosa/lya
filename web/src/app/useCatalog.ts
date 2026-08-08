/**
 * 工具与动作的**全局目录**：这个程序一共有哪些能力。
 *
 * 和 `app/chat` 里那份工具清单不是同一回事，这一点值得说清，因为它们来自同一个接口：
 *
 * - `client.tools(sessionId)` —— 带会话 id，每项多一个 `enabled`，答的是「这个会话现在
 *   开着哪些」。归 chat 层（`loadTools`），会话设置那一屏用。
 * - `client.tools()` —— 不带 id，答的是「一共有哪些」。就是这里。工具页和设置页用它，
 *   两者都跟当前会话无关，甚至没有当前会话时也要能看。
 *
 * 混用过一次的后果是「没有打开任何会话时工具页是空的」——因为 chat 那个版本在没有
 * currentId 时直接 return。
 *
 * 目录在进程活着期间基本不变（工具是启动时注册的），所以读一次就够；配置里改全局启用
 * 名单不影响这份目录，那是另一回事。
 */

import { computed, ref } from 'vue'

import { errorText, type ActionInfo, type ToolInfo } from '../api/client'
import { client } from './client'

const tools = ref<ToolInfo[]>([])
const actions = ref<ActionInfo[]>([])
const loading = ref(false)
const error = ref('')
let inflight: Promise<void> | null = null

/** 确保目录已经读到；读过就不再请求。 */
export async function ensureCatalog(): Promise<void> {
  if (tools.value.length || actions.value.length) return
  inflight ??= (async () => {
    loading.value = true
    error.value = ''
    try {
      const [toolList, actionList] = await Promise.all([client.tools(), client.actions()])
      tools.value = toolList
      actions.value = actionList
    } catch (err) {
      error.value = errorText(err)
    } finally {
      loading.value = false
      inflight = null
    }
  })()
  await inflight
}

/** 只读的共享目录，按名字排好序——两屏都要排，没道理各排一次。 */
export const catalog = {
  tools: computed(() =>
    [...tools.value].sort((a, b) => a.raw_name.localeCompare(b.raw_name, 'zh-CN')),
  ),
  actions: computed(() =>
    [...actions.value].sort((a, b) => a.raw_name.localeCompare(b.raw_name, 'zh-CN')),
  ),
  loading: computed(() => loading.value),
  error: computed(() => error.value),
}
