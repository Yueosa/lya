import { ref } from 'vue'

/** 侧栏是否收起；默认收起，用户展开后记住选择。 */
export const sidebarCollapsed = ref(localStorage.getItem('lya.sidebar.collapsed') !== '0')

export function setSidebarCollapsed(next: boolean): void {
  sidebarCollapsed.value = next
  localStorage.setItem('lya.sidebar.collapsed', next ? '1' : '0')
}

export function toggleSidebar(): void {
  setSidebarCollapsed(!sidebarCollapsed.value)
}
