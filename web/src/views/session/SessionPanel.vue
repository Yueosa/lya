<!--
  会话面板：概览 / 工具 / 会话 / 显示。
  drawer = 聊天右侧；page = MC 等全屏 view=settings。
-->

<script setup lang="ts">
import { ref } from 'vue'

import ViewHead from '../../ui/ViewHead.vue'
import SessionDisplayTab from './SessionDisplayTab.vue'
import SessionMetaTab from './SessionMetaTab.vue'
import SessionOverviewTab from './SessionOverviewTab.vue'
import SessionToolsTab from './SessionToolsTab.vue'

type Tab = 'overview' | 'tools' | 'meta' | 'display'

withDefaults(
  defineProps<{
    layout?: 'drawer' | 'page'
  }>(),
  { layout: 'drawer' },
)

const tab = ref<Tab>('overview')

const TABS: { id: Tab; label: string }[] = [
  { id: 'overview', label: '概览' },
  { id: 'tools', label: '工具' },
  { id: 'meta', label: '会话' },
  { id: 'display', label: '显示' },
]
</script>

<template>
  <div v-if="layout === 'page'" class="split-view session-panel">
    <ViewHead title="会话" />
    <div class="split-view__body">
      <aside class="split-view__list">
        <div class="split-view__list-scroll" style="padding-top: 8px">
          <button
            v-for="item in TABS"
            :key="item.id"
            class="split-view__list-item"
            :class="{ 'split-view__list-item--on': tab === item.id }"
            @click="tab = item.id"
          >
            <span class="split-view__list-title">{{ item.label }}</span>
          </button>
        </div>
      </aside>
      <main class="split-view__main">
        <div class="session-panel__page-pane">
          <Transition name="lya-split" mode="out-in">
            <SessionOverviewTab v-if="tab === 'overview'" key="overview" />
            <SessionToolsTab v-else-if="tab === 'tools'" key="tools" />
            <SessionMetaTab v-else-if="tab === 'meta'" key="meta" />
            <SessionDisplayTab v-else key="display" />
          </Transition>
        </div>
      </main>
    </div>
  </div>

  <div v-else class="session-panel session-panel--drawer">
    <nav class="session-panel__tabs" role="tablist">
      <button
        v-for="item in TABS"
        :key="item.id"
        class="session-panel__tab"
        :class="{ 'session-panel__tab--on': tab === item.id }"
        role="tab"
        :aria-selected="tab === item.id"
        @click="tab = item.id"
      >
        {{ item.label }}
      </button>
    </nav>
    <div class="session-panel__pane">
      <Transition name="lya-split" mode="out-in">
        <SessionOverviewTab v-if="tab === 'overview'" key="overview" />
        <SessionToolsTab v-else-if="tab === 'tools'" key="tools" />
        <SessionMetaTab v-else-if="tab === 'meta'" key="meta" />
        <SessionDisplayTab v-else key="display" />
      </Transition>
    </div>
  </div>
</template>

<style scoped>
.session-panel--drawer {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.session-panel__tabs {
  display: flex;
  flex-shrink: 0;
  gap: 2px;
  padding: 8px 10px;
  border-bottom: var(--border-width) solid var(--border);
  overflow-x: auto;
}

.session-panel__tab {
  flex-shrink: 0;
  padding: 7px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-sm);
  font-weight: 600;
  cursor: pointer;
  transition: background var(--transition), color var(--transition);
}

.session-panel__tab:hover {
  background: var(--surface-hover);
  color: var(--text);
}

.session-panel__tab--on {
  background: var(--surface-active);
  color: var(--accent);
}

.session-panel__pane {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 14px 16px;
}

.session-panel__pane :deep(.session-tab) {
  min-height: min-content;
}

.session-panel__page-pane {
  height: 100%;
  overflow: auto;
  padding: 16px 20px;
}
</style>
