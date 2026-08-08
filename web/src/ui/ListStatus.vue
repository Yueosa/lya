<!--
  列表栏顶部那三种话：读失败 / 还在读 / 空。

  三态文案与 class 原先在记忆、工具、模型、配置各写一遍。失败走 page__error（就地
  说明，不弹窗——那一栏本来就是空的），另两态走 split-view__hint。
-->

<script setup lang="ts">
withDefaults(
  defineProps<{
    error?: string | null
    loading?: boolean
    empty?: boolean
    loadingText?: string
    emptyText?: string
  }>(),
  {
    error: null,
    loading: false,
    empty: false,
    loadingText: '加载中…',
    emptyText: '暂无内容',
  },
)
</script>

<template>
  <p v-if="error" class="page__error">{{ error }}</p>
  <p v-else-if="loading" class="split-view__hint">{{ loadingText }}</p>
  <p v-else-if="empty" class="split-view__hint">{{ emptyText }}</p>
</template>
