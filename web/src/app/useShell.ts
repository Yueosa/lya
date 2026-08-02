import { ref } from 'vue'

/** 侧栏是否收起；与 lianclaw 一样由聊天头补偿左侧空间。 */
export const sidebarCollapsed = ref(localStorage.getItem('lya.sidebar.collapsed') === '1')

export function setSidebarCollapsed(next: boolean): void {
  sidebarCollapsed.value = next
  localStorage.setItem('lya.sidebar.collapsed', next ? '1' : '0')
}

export function toggleSidebar(): void {
  setSidebarCollapsed(!sidebarCollapsed.value)
}
