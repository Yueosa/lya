<!--
  存储占用。

  单独一页而不是「设置」下的一个 tab：这里一个字都改不了，纯观测面板，
  和「配置」放一起只会让人以为动了什么就会写盘。
-->

<script setup lang="ts">
import { onMounted, ref } from 'vue'

import { errorText, type UsageReport } from '../api/client'
import { client } from '../app/client'
import ListStatus from '../ui/ListStatus.vue'
import StorageBreakdown from '../ui/StorageBreakdown.vue'
import ViewHead from '../ui/ViewHead.vue'

const report = ref<UsageReport | null>(null)
const loading = ref(true)
const loadError = ref('')

onMounted(load)

async function load(): Promise<void> {
  loading.value = true
  loadError.value = ''
  try {
    report.value = await client.storageStats()
  } catch (error) {
    report.value = null
    loadError.value = errorText(error)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page">
    <ViewHead title="存储" />

    <div class="page__body">
      <ListStatus :loading="loading" :error="loadError" loading-text="正在扫描…" />

      <section v-if="!loading && !loadError && report" class="page__pane">
        <p class="page__hint">
          数据目录：<code>{{ report.root }}</code>
        </p>
        <StorageBreakdown :report="report" />
        <div class="row row--end">
          <button class="btn btn--sm" @click="load">重新扫描</button>
        </div>
      </section>
    </div>
  </div>
</template>
