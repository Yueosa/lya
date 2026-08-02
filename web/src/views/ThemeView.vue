<script setup lang="ts">
import { computed, ref } from 'vue'

import { applyTheme, themeId, THEMES } from '../themes'
import ThemePreview from '../ui/ThemePreview.vue'
import ViewHead from '../ui/ViewHead.vue'

const selectedId = ref(themeId.value)

const selected = computed(() => THEMES.find((item) => item.id === selectedId.value) ?? THEMES[0]!)

function pickTheme(id: string): void {
  selectedId.value = id
}

function useTheme(): void {
  applyTheme(selectedId.value)
}
</script>

<template>
  <div class="split-view">
    <ViewHead title="外观" />

    <div class="split-view__body">
      <aside class="split-view__list">
        <div class="split-view__list-scroll" style="padding-top: 8px">
          <button
            v-for="item in THEMES"
            :key="item.id"
            class="split-view__list-item"
            :class="{ 'split-view__list-item--on': selectedId === item.id }"
            @click="pickTheme(item.id)"
          >
            <span class="split-view__list-title">{{ item.label }}</span>
            <span class="split-view__list-meta">{{ item.scheme === 'dark' ? '深色' : '浅色' }}</span>
          </button>
        </div>
      </aside>

      <main class="split-view__main">
        <Transition name="lya-split" mode="out-in">
          <div :key="selected.id" class="page__pane">
          <header class="split-view__detail-head">
            <h3>{{ selected.label }}</h3>
            <span class="row__grow" />
            <span v-if="themeId === selected.id" class="pill">当前使用中</span>
            <button v-else class="btn btn--primary btn--sm" @click="useTheme">应用此主题</button>
          </header>

          <p class="page__hint">
            左侧切换预览，点「应用此主题」才会改全局配色与外壳。界面结构不变。
          </p>

          <ThemePreview :theme-id="selected.id" />
          </div>
        </Transition>
      </main>
    </div>
  </div>
</template>
