<!--
  存储占用：一条堆叠横条 + 图例，下面每个分类一张可展开的卡片。

  为什么不是甜甜圈加表格：横条在窄栏里不会挤成一团，图例天然排两列，而且
  「谁占了大头」一眼就看得出来——这也是 GitHub 用语言条而不用饼图的原因。

  为什么要区分深浅：本地媒体缓存优先硬链接到用户原来的文件，那些条目看着有几十
  兆，删掉释放 0 字节。浅色那截就是这部分，不标出来的话「可回收 169 MB」是句谎话。
-->

<script setup lang="ts">
import { computed, ref } from 'vue'

import type { DiskUsage, UsageReport, UsageSection } from '../api/client'
import { formatBytes } from '../utils/formatBytes'

const props = defineProps<{ report: UsageReport }>()

const COLORS = [
  'var(--accent)',
  'var(--info)',
  'var(--success)',
  'var(--warning)',
  'var(--danger)',
  'var(--text-muted)',
]

/** 默认全折叠：一进来先看总览，要细节再点开。 */
const opened = ref<Set<string>>(new Set())

function isOpen(id: string): boolean {
  return opened.value.has(id)
}

function toggle(id: string): void {
  const next = new Set(opened.value)
  if (!next.delete(id)) next.add(id)
  opened.value = next
}

const total = computed(() => props.report.usage.physical_bytes)

function share(usage: DiskUsage): number {
  return total.value > 0 ? (usage.physical_bytes / total.value) * 100 : 0
}

function percentText(usage: DiskUsage): string {
  const value = share(usage)
  if (value === 0) return '0%'
  return value < 0.1 ? '<0.1%' : `${value.toFixed(1)}%`
}

function colorOf(index: number): string {
  return COLORS[index % COLORS.length] ?? 'var(--accent)'
}

interface Segment {
  id: string
  label: string
  color: string
  percent: string
  /** 独立占盘的那截。 */
  ownPercent: number
  /** 与外部共用 inode 的那截，画成浅色。 */
  sharedPercent: number
}

const segments = computed<Segment[]>(() =>
  props.report.sections
    .map((section, index) => ({ section, color: colorOf(index) }))
    .filter(({ section }) => section.usage.physical_bytes > 0)
    .map(({ section, color }) => ({
      id: section.id,
      label: section.label,
      color,
      percent: percentText(section.usage),
      ownPercent: total.value > 0 ? (section.usage.reclaimable_bytes / total.value) * 100 : 0,
      sharedPercent: total.value > 0 ? (section.usage.shared_bytes / total.value) * 100 : 0,
    })),
)

const cards = computed(() =>
  props.report.sections.map((section, index) => ({ section, color: colorOf(index) })),
)

/** 有硬链接共用时把两个数都说清楚，否则一个数就够。 */
function usageText(usage: DiskUsage): string {
  if (usage.shared_bytes > 0) {
    return `${formatBytes(usage.physical_bytes)}（可回收 ${formatBytes(usage.reclaimable_bytes)}）`
  }
  return formatBytes(usage.physical_bytes)
}

function fileCountText(usage: DiskUsage): string {
  if (usage.file_count === 0) return '空'
  if (usage.linked_file_count > 0) {
    return `${usage.file_count} 个文件，其中 ${usage.linked_file_count} 个是硬链接`
  }
  return `${usage.file_count} 个文件`
}

/** 扁平化成缩进行，只展开用户点开过的层。 */
interface Row {
  id: string
  depth: number
  label: string
  usage: DiskUsage
  hasChildren: boolean
  open: boolean
}

function flatten(section: UsageSection, depth: number, rows: Row[]): void {
  const hasChildren = Boolean(section.children?.length)
  const open = hasChildren && isOpen(section.id)
  rows.push({
    id: section.id,
    depth,
    label: section.label,
    usage: section.usage,
    hasChildren,
    open,
  })
  if (!open) return
  for (const child of section.children ?? []) flatten(child, depth + 1, rows)
}

function childRows(section: UsageSection): Row[] {
  const rows: Row[] = []
  for (const child of section.children ?? []) flatten(child, 0, rows)
  return rows
}
</script>

<template>
  <div class="storage">
    <p class="storage__total">
      <strong>{{ formatBytes(report.usage.physical_bytes) }}</strong>
      <span v-if="report.usage.shared_bytes > 0" class="storage__total-note">
        其中 {{ formatBytes(report.usage.shared_bytes) }} 与目录外的原文件共用，删不掉
      </span>
    </p>

    <div v-if="segments.length === 0" class="storage__empty">暂无占用数据</div>

    <template v-else>
      <div class="storage__bar" role="img" aria-label="各分类占比">
        <template v-for="segment in segments" :key="segment.id">
          <span
            v-if="segment.ownPercent > 0"
            class="storage__seg"
            :style="{ width: `${segment.ownPercent}%`, background: segment.color }"
            :title="`${segment.label}（独立占盘）`"
          />
          <span
            v-if="segment.sharedPercent > 0"
            class="storage__seg storage__seg--shared"
            :style="{ width: `${segment.sharedPercent}%`, '--local-seg': segment.color }"
            :title="`${segment.label}（与原文件共用，删掉不释放空间）`"
          />
        </template>
      </div>

      <ul class="storage__legend">
        <li v-for="segment in segments" :key="segment.id" class="storage__legend-item">
          <span class="storage__dot" :style="{ background: segment.color }" />
          <span class="storage__legend-label">{{ segment.label }}</span>
          <span class="storage__legend-value">{{ segment.percent }}</span>
        </li>
      </ul>
    </template>

    <div class="storage__cards">
      <section v-for="{ section, color } in cards" :key="section.id" class="panel storage__card">
        <button
          type="button"
          class="storage__card-head"
          :class="{ 'storage__card-head--plain': !section.children?.length }"
          :aria-expanded="isOpen(section.id)"
          :disabled="!section.children?.length"
          @click="toggle(section.id)"
        >
          <span class="storage__dot" :style="{ background: color }" />
          <span class="storage__card-title">{{ section.label }}</span>
          <span class="storage__card-size">{{ usageText(section.usage) }}</span>
          <span class="storage__card-share">{{ percentText(section.usage) }}</span>
          <span
            v-if="section.children?.length"
            class="storage__caret"
            :class="{ 'storage__caret--open': isOpen(section.id) }"
            aria-hidden="true"
            >▸</span
          >
        </button>

        <p class="storage__card-meta">{{ fileCountText(section.usage) }}</p>

        <ul v-if="isOpen(section.id)" class="storage__rows">
          <li
            v-for="row in childRows(section)"
            :key="row.id"
            class="storage__row"
            :class="{ 'storage__row--branch': row.hasChildren }"
            :style="{ paddingLeft: `${row.depth * 16}px` }"
            @click="row.hasChildren ? toggle(row.id) : undefined"
          >
            <!-- 叶子行也占住这个宽度，否则同一层的标签会左右错开 -->
            <span
              class="storage__caret"
              :class="{ 'storage__caret--open': row.open }"
              aria-hidden="true"
              >{{ row.hasChildren ? '▸' : '' }}</span
            >
            <span class="storage__row-label">{{ row.label }}</span>
            <span v-if="row.usage.linked_file_count > 0" class="storage__badge">共用</span>
            <span class="storage__row-size">{{ usageText(row.usage) }}</span>
          </li>
        </ul>
      </section>
    </div>
  </div>
</template>

<style scoped>
.storage {
  display: grid;
  gap: 14px;
}

.storage__total {
  margin: 0;
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-wrap: wrap;
}

.storage__total strong {
  font-size: var(--text-lg);
  color: var(--text);
}

.storage__total-note {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.storage__empty {
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.storage__bar {
  display: flex;
  height: 10px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--bg-sunken);
}

.storage__seg {
  height: 100%;
  min-width: 2px;
}

/* 浅色 = 与目录外的原文件共用 inode，删了不腾空间 */
.storage__seg--shared {
  background: color-mix(in srgb, var(--local-seg) 30%, transparent) !important;
}

.storage__legend {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 4px 16px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.storage__legend-item {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: var(--text-sm);
  min-width: 0;
}

.storage__legend-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-muted);
}

.storage__legend-value {
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.storage__dot {
  width: 9px;
  height: 9px;
  flex-shrink: 0;
  border-radius: var(--radius-pill);
}

.storage__cards {
  display: grid;
  gap: 10px;
}

.storage__card {
  padding: 12px 14px;
}

.storage__card-head {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 0;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.storage__card-head--plain {
  cursor: default;
}

.storage__card-title {
  flex: 1;
  min-width: 0;
  font-weight: 600;
}

.storage__card-size,
.storage__card-share {
  font-size: var(--text-sm);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.storage__card-share {
  min-width: 52px;
  text-align: right;
}

.storage__caret {
  flex-shrink: 0;
  width: 12px;
  color: var(--text-faint);
  font-size: 11px;
  transition: transform 0.15s ease;
}

.storage__caret--open {
  transform: rotate(90deg);
}

.storage__card-meta {
  margin: 6px 0 0;
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.storage__rows {
  margin: 10px 0 0;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 1px;
}

.storage__row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.storage__row--branch {
  cursor: pointer;
}

.storage__row--branch:hover {
  background: var(--surface-hover);
}

.storage__row-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.storage__row-size {
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

.storage__badge {
  flex-shrink: 0;
  padding: 0 6px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--info) 16%, transparent);
  color: var(--info);
  font-size: var(--text-xs);
}
</style>
