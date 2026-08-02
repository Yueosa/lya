<script setup lang="ts">
import { computed } from 'vue'

import type { CategoryUsage } from '../api/client'
import { formatBytes } from '../utils/formatBytes'

const props = defineProps<{
  categories: CategoryUsage[]
  totalBytes: number
}>()

const COLORS = [
  'var(--accent)',
  'var(--success)',
  'var(--warning)',
  'var(--danger)',
  'var(--info)',
  'var(--accent-soft)',
  'var(--text-muted)',
]

const slices = computed(() => {
  const total = props.totalBytes || props.categories.reduce((sum, item) => sum + item.bytes, 0)
  if (total <= 0) return [] as { id: string; label: string; bytes: number; percent: number; color: string; dash: string; offset: string }[]

  let offset = 0
  return props.categories
    .filter((item) => item.bytes > 0)
    .map((item, index) => {
      const percent = (item.bytes / total) * 100
      const dash = `${percent} ${100 - percent}`
      const slice = {
        id: item.id,
        label: item.label,
        bytes: item.bytes,
        percent,
        color: COLORS[index % COLORS.length] ?? 'var(--accent)',
        dash,
        offset: `${offset}`,
      }
      offset -= percent
      return slice
    })
})

const rows = computed(() =>
  props.categories.map((item, index) => ({
    ...item,
    color: COLORS[index % COLORS.length] ?? 'var(--accent)',
    percent:
      props.totalBytes > 0 ? ((item.bytes / props.totalBytes) * 100).toFixed(1) : '0.0',
  })),
)
</script>

<template>
  <div class="storage-pie">
    <div v-if="slices.length === 0" class="storage-pie__empty">暂无占用数据</div>
    <div v-else class="storage-pie__chart-wrap">
      <svg viewBox="0 0 42 42" class="storage-pie__chart" aria-hidden="true">
        <circle cx="21" cy="21" r="15.915" fill="transparent" stroke="var(--border)" stroke-width="4" />
        <circle
          v-for="slice in slices"
          :key="slice.id"
          cx="21"
          cy="21"
          r="15.915"
          fill="transparent"
          :stroke="slice.color"
          stroke-width="4"
          :stroke-dasharray="slice.dash"
          :stroke-dashoffset="slice.offset"
          transform="rotate(-90 21 21)"
        />
      </svg>
      <div class="storage-pie__center">
        <strong>{{ formatBytes(totalBytes) }}</strong>
        <span>合计</span>
      </div>
    </div>

    <table v-if="rows.length" class="storage-pie__table">
      <thead>
        <tr>
          <th>分类</th>
          <th>占用</th>
          <th>占比</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.id">
          <td>
            <span class="storage-pie__swatch" :style="{ background: row.color }" />
            {{ row.label }}
          </td>
          <td>{{ formatBytes(row.bytes) }}</td>
          <td>{{ row.percent }}%</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.storage-pie {
  display: grid;
  gap: 20px;
}

.storage-pie__empty {
  color: var(--text-muted);
}

.storage-pie__chart-wrap {
  position: relative;
  width: min(220px, 100%);
  margin-inline: auto;
}

.storage-pie__chart {
  display: block;
  width: 100%;
  height: auto;
}

.storage-pie__center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  font-size: 12px;
  color: var(--text-muted);
}

.storage-pie__center strong {
  font-size: 16px;
  color: var(--text);
}

.storage-pie__table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.storage-pie__table th,
.storage-pie__table td {
  padding: 8px 6px;
  border-bottom: 1px solid var(--border);
  text-align: left;
}

.storage-pie__table th:last-child,
.storage-pie__table td:last-child {
  text-align: right;
}

.storage-pie__swatch {
  display: inline-block;
  width: 10px;
  height: 10px;
  margin-right: 8px;
  border-radius: 999px;
  vertical-align: middle;
}
</style>
