<script setup lang="ts">
import { computed, ref } from 'vue'

import type { LocalCacheStats, UsageReport, UsageSection } from '../api/client'
import { formatBytes } from '../utils/formatBytes'

const props = defineProps<{
  report: UsageReport
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

const collapsed = ref<Record<string, boolean>>({})

function toggle(id: string) {
  collapsed.value[id] = !collapsed.value[id]
}

function isOpen(id: string) {
  return collapsed.value[id] !== true
}

const slices = computed(() => {
  const total = props.report.total_bytes
  if (total <= 0) return [] as { id: string; label: string; bytes: number; percent: number; color: string; dash: string; offset: string }[]

  let offset = 0
  return props.report.sections
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

interface Row {
  id: string
  depth: number
  label: string
  bytes: number
  percent: string
  color: string
  detail?: string
  hasChildren: boolean
  open: boolean
}

function formatLocal(local: LocalCacheStats): string {
  if (local.file_count === 0) return '—'
  if (local.shared_bytes > 0) {
    return `${formatBytes(local.physical_bytes)} / ${formatBytes(local.logical_bytes)} (共用 ${formatBytes(local.shared_bytes)})`
  }
  return formatBytes(local.logical_bytes)
}

function flattenSection(section: UsageSection, depth: number, color: string, rows: Row[]) {
  const total = props.report.total_bytes
  const percent = total > 0 ? ((section.bytes / total) * 100).toFixed(1) : '0.0'
  const hasChildren = Boolean(section.children?.length)
  const open = isOpen(section.id)

  rows.push({
    id: section.id,
    depth,
    label: section.label,
    bytes: section.bytes,
    percent,
    color,
    hasChildren,
    open,
  })

  if (!hasChildren || !open) {
    if (section.local || section.web) {
      if (section.local) {
        rows.push({
          id: `${section.id}.local`,
          depth: depth + 1,
          label: 'Local',
          bytes: section.local.physical_bytes,
          percent: total > 0 ? ((section.local.physical_bytes / total) * 100).toFixed(1) : '0.0',
          color,
          detail: formatLocal(section.local),
          hasChildren: false,
          open: true,
        })
      }
      if (section.web) {
        rows.push({
          id: `${section.id}.web`,
          depth: depth + 1,
          label: 'Web',
          bytes: section.web.bytes,
          percent: total > 0 ? ((section.web.bytes / total) * 100).toFixed(1) : '0.0',
          color,
          detail: section.web.file_count === 0 ? '—' : formatBytes(section.web.bytes),
          hasChildren: false,
          open: true,
        })
      }
    }
    return
  }

  for (const child of section.children ?? []) {
    flattenSection(child, depth + 1, color, rows)
  }
}

const rows = computed(() => {
  const out: Row[] = []
  props.report.sections.forEach((section, index) => {
    const color = COLORS[index % COLORS.length] ?? 'var(--accent)'
    flattenSection(section, 0, color, out)
  })
  return out
})

function onRowClick(row: Row) {
  if (row.hasChildren) toggle(row.id)
}
</script>

<template>
  <div class="storage-breakdown">
    <div v-if="slices.length === 0" class="storage-breakdown__empty">暂无占用数据</div>
    <div v-else class="storage-breakdown__chart-wrap">
      <svg viewBox="0 0 42 42" class="storage-breakdown__chart" aria-hidden="true">
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
      <div class="storage-breakdown__center">
        <strong>{{ formatBytes(report.total_bytes) }}</strong>
        <span>合计</span>
      </div>
    </div>

    <table v-if="rows.length" class="storage-breakdown__table">
      <thead>
        <tr>
          <th>分类</th>
          <th>占用</th>
          <th>占比</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="row in rows"
          :key="row.id"
          :class="{
            'storage-breakdown__row--branch': row.hasChildren,
            'storage-breakdown__row--leaf': !row.hasChildren,
          }"
          @click="row.hasChildren ? onRowClick(row) : undefined"
        >
          <td :style="{ paddingLeft: `${8 + row.depth * 16}px` }">
            <span class="storage-breakdown__swatch" :style="{ background: row.color }" />
            <span v-if="row.hasChildren" class="storage-breakdown__toggle">{{ row.open ? '▾' : '▸' }}</span>
            {{ row.label }}
          </td>
          <td>{{ row.detail ?? formatBytes(row.bytes) }}</td>
          <td>{{ row.percent }}%</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.storage-breakdown {
  display: grid;
  gap: 20px;
}

.storage-breakdown__empty {
  color: var(--text-muted);
}

.storage-breakdown__chart-wrap {
  position: relative;
  width: min(220px, 100%);
  margin-inline: auto;
}

.storage-breakdown__chart {
  display: block;
  width: 100%;
  height: auto;
}

.storage-breakdown__center {
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

.storage-breakdown__center strong {
  font-size: 16px;
  color: var(--text);
}

.storage-breakdown__table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.storage-breakdown__table th,
.storage-breakdown__table td {
  padding: 8px 6px;
  border-bottom: 1px solid var(--border);
  text-align: left;
}

.storage-breakdown__table th:last-child,
.storage-breakdown__table td:last-child {
  text-align: right;
}

.storage-breakdown__row--branch {
  cursor: pointer;
}

.storage-breakdown__row--branch:hover td {
  background: color-mix(in srgb, var(--border) 35%, transparent);
}

.storage-breakdown__swatch {
  display: inline-block;
  width: 10px;
  height: 10px;
  margin-right: 8px;
  border-radius: 999px;
  vertical-align: middle;
}

.storage-breakdown__toggle {
  display: inline-block;
  width: 12px;
  margin-right: 4px;
  color: var(--text-muted);
  font-size: 11px;
}
</style>
