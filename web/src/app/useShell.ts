import { ref } from 'vue'

import { readLocal, writeLocal } from '../utils/storage'

const KEY = 'lya.sidebar.collapsed'

/** 侧栏是否收起；默认收起，用户展开后记住选择。 */
export const sidebarCollapsed = ref(readLocal(KEY) !== '0')

export function setSidebarCollapsed(next: boolean): void {
  sidebarCollapsed.value = next
  writeLocal(KEY, next ? '1' : '0')
}

export function toggleSidebar(): void {
  setSidebarCollapsed(!sidebarCollapsed.value)
}
