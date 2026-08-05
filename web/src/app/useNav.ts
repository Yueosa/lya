/**
 * 当前视图，以及刷新之后回到原处。
 *
 * `view` 原先是 `App.vue` 里一个裸 `ref('home')`，`currentId` 也不持久化，于是按一下
 * F5 就回首页、当前会话也丢了——本地应用没有理由这样，浏览器刷新在这里不是「重新
 * 开始」，只是重新加载代码。
 *
 * 用 `localStorage` 而不是 URL：路由要引进来一整套依赖，而这个应用没有分享链接的
 * 场景（媒体地址里的令牌本来就每次重启换）。代价是多开标签页会共享位置，本机单用户
 * 无所谓。
 */

import { ref, watch } from 'vue'

import type { View } from '../shell/types'
import { currentId } from './chat/state'
import { readLocal, writeLocal } from '../utils/storage'

const VIEW_KEY = 'lya.nav.view'
const SESSION_KEY = 'lya.nav.session'

/** 认得的视图名。存进去的字符串不一定还算数——版本换了可能就没这个视图了。 */
const VIEWS: readonly View[] = [
  'home',
  'chat',
  'sessions',
  'settings',
  'memory',
  'tools',
  'models',
  'theme',
  'persona',
  'config',
  'storage',
]

function savedView(): View {
  const raw = readLocal(VIEW_KEY)
  return VIEWS.includes(raw as View) ? (raw as View) : 'home'
}

/**
 * 上次停在哪个视图。
 *
 * 注意这只是**落点意向**：`chat` 和 `settings` 还要有会话才立得住，能不能真回去由
 * [`restoreSession`] 说了算。
 */
export const view = ref<View>(savedView())

export function setView(next: View): void {
  view.value = next
  writeLocal(VIEW_KEY, next)
}

/** 上次打开的会话 id。 */
export function savedSession(): string | null {
  return readLocal(SESSION_KEY)
}

// 盯着 currentId 记，而不是让 openSession / closeSession 各自记一次：那样每加一条
// 改变当前会话的路径（删除、归档、切分支）都得记着补一句，漏了不报错，只是刷新之后
// 落错地方
watch(currentId, (id) => {
  writeLocal(SESSION_KEY, id)
})
