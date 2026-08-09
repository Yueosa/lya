/**
 * 一份共享的配置，而且跟着后端走。
 *
 * # 为什么要共享
 *
 * 配置原先是四处各自 `client.config()` 拉一遍：设置页、工具页、人设页、会话设置页。四份
 * 互不知情，于是其中一处保存之后，别处那份还是旧的——最难看的一例是会话设置页里的全局
 * 人设：它挂载时读一次，此后在人设页改完全局人设，那一行显示的还是旧正文，而界面上没有
 * 任何迹象说明它已经过期了。
 *
 * # 为什么能自动新
 *
 * 后端**早就在广播**了：写完 `runtime.toml` 或 `prompt.toml` 之后 `hub.broadcast_global`
 * 会发一条 `config_changed`（见 `lya-api` 的 `http/config.rs`）。前端也早就收到了，但只用
 * 它去刷新模型列表和运行时默认值，配置本身没人刷。这里把那条广播接上，缺的就只是一个
 * 「谁来存这份配置」——也就是本文件。
 *
 * 所以这里不新增任何请求：四次变一次，加上改动之后自动重取。
 */

import { computed, ref } from 'vue'

import type { ConfigView } from '../api/client'
import { client } from './client'
import { errorText } from '../api/client'

/** 当前配置；还没读到时是 `null`。 */
const config = ref<ConfigView | null>(null)
const loading = ref(false)
const error = ref('')

/** 正在飞的那次请求。同一瞬间几屏一起挂载时只发一次。 */
let inflight: Promise<void> | null = null

/**
 * 确保配置已经读到。
 *
 * 已经有了就直接返回，不重复请求——这是「几屏共用一份」的关键。要强制重取用
 * [`reloadConfig`]。
 */
export async function ensureConfig(): Promise<void> {
  if (config.value) return
  await reloadConfig()
}

/** 重新读一遍。配置变更广播和用户手动刷新都走这里。 */
export function reloadConfig(): Promise<void> {
  inflight ??= (async () => {
    loading.value = true
    error.value = ''
    try {
      config.value = await client.config()
    } catch (err) {
      error.value = errorText(err)
    } finally {
      loading.value = false
      inflight = null
    }
  })()
  return inflight
}

/**
 * 挂上「配置变了就重取」。
 *
 * 由 `App.vue` 在处理全局事件时调一次。不在本模块里自己订阅：那条 SSE 连接归 `App` 管，
 * 两处各订阅一次就会有两条连接，而它们收到的是同一批事件。
 */
export function onConfigChanged(): void {
  // 没人在看配置时也重取：那几屏之间来回切换很频繁，等切过去再拉就会闪一下旧值
  void reloadConfig()
}

/** 只读的共享状态。 */
export const configState = {
  config: computed(() => config.value),
  runtime: computed(() => config.value?.runtime ?? null),
  /** 全局提示词各段。 */
  prompt: computed(() => config.value?.prompt ?? null),
  loading: computed(() => loading.value),
  error: computed(() => error.value),
}
