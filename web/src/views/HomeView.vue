<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

import type { Memory } from '../api/client'
import { archivedSessions, client, models, sessions } from '../app/useChat'
import { setSidebarCollapsed, sidebarCollapsed } from '../app/useShell'
import Icon from '../ui/Icon.vue'

type FloatKind = 'session' | 'memory' | 'model'

interface FloatLabel {
  id: string
  text: string
  kind: FloatKind
  size: number
}

interface FloatBox {
  x: number
  y: number
  w: number
  h: number
}

interface ActiveFloat extends FloatLabel {
  key: number
  x: number
  y: number
  driftSec: number
  driftDelaySec: number
  lifeMs: number
  phase: 'in' | 'live' | 'out'
}

const TITLE_ZONE: FloatBox = { x: 50, y: 50, w: 44, h: 38 }
const BOX_GAP = 2.4
const MAX_VISIBLE = 16
const SPAWN_INTERVAL_MS = 700

const memories = ref<Memory[]>([])
const active = ref<ActiveFloat[]>([])
let keySeq = 0
let spawnTimer: number | null = null
let timers: number[] = []

onMounted(async () => {
  setSidebarCollapsed(true)
  try {
    memories.value = await client.memories()
  } catch {
    memories.value = []
  }
  primeFloats()
  spawnTimer = window.setInterval(tickSpawn, SPAWN_INTERVAL_MS)
})

onUnmounted(() => {
  if (spawnTimer !== null) window.clearInterval(spawnTimer)
  for (const id of timers) window.clearTimeout(id)
  timers = []
})

function hash(text: string): number {
  let value = 0
  for (let i = 0; i < text.length; i++) value = (value * 31 + text.charCodeAt(i)) | 0
  return Math.abs(value)
}

function estimateBox(text: string, fontSize: number): Pick<FloatBox, 'w' | 'h'> {
  const displayChars = Math.min(text.length, 28)
  const w = Math.min(displayChars * fontSize * 0.038, 30) + 3
  const h = fontSize * 0.065 + 2
  return { w, h }
}

function boxesOverlap(a: FloatBox, b: FloatBox): boolean {
  return (
    Math.abs(a.x - b.x) < (a.w + b.w) / 2 + BOX_GAP &&
    Math.abs(a.y - b.y) < (a.h + b.h) / 2 + BOX_GAP
  )
}

function inTitleZone(box: FloatBox): boolean {
  return boxesOverlap(box, TITLE_ZONE)
}

function occupiedBoxes(): FloatBox[] {
  const boxes: FloatBox[] = [{ ...TITLE_ZONE }]
  for (const item of active.value) {
    if (item.phase === 'out') continue
    boxes.push({ x: item.x, y: item.y, ...estimateBox(item.text, item.size) })
  }
  return boxes
}

function randomPosition(label: FloatLabel): { x: number; y: number } | null {
  const boxSize = estimateBox(label.text, label.size)
  const placed = occupiedBoxes()

  for (let attempt = 0; attempt < 120; attempt++) {
    const candidate: FloatBox = {
      x: 5 + Math.random() * 90,
      y: 5 + Math.random() * 88,
      ...boxSize,
    }
    if (inTitleZone(candidate)) continue
    if (placed.every((other) => !boxesOverlap(candidate, other))) {
      return { x: candidate.x, y: candidate.y }
    }
  }
  return null
}

const labelPool = computed((): FloatLabel[] => {
  const labels: FloatLabel[] = []

  for (const session of sessions.value) {
    const title = session.title?.trim() || '未命名'
    labels.push({
      id: `s-${session.id}`,
      text: title,
      kind: 'session',
      size: 18 + (hash(`s-${session.id}`) % 10),
    })
  }
  for (const session of archivedSessions.value) {
    const title = session.title?.trim() || '未命名'
    labels.push({
      id: `a-${session.id}`,
      text: title,
      kind: 'session',
      size: 18 + (hash(`a-${session.id}`) % 10),
    })
  }
  for (const memory of memories.value) {
    labels.push({
      id: `m-${memory.id}`,
      text: memory.title,
      kind: 'memory',
      size: 18 + (hash(`m-${memory.id}`) % 10),
    })
  }
  for (const model of models.value) {
    labels.push({
      id: `md-${model.id}`,
      text: model.name,
      kind: 'model',
      size: 18 + (hash(`md-${model.id}`) % 10),
    })
  }

  return labels
})

function pickLabel(): FloatLabel | null {
  const pool = labelPool.value
  if (pool.length === 0) return null
  return pool[Math.floor(Math.random() * pool.length)] ?? null
}

function schedule(fn: () => void, ms: number): void {
  const id = window.setTimeout(fn, ms)
  timers.push(id)
}

function removeFloat(key: number): void {
  active.value = active.value.filter((item) => item.key !== key)
}

function beginExit(item: ActiveFloat): void {
  const row = active.value.find((entry) => entry.key === item.key)
  if (!row || row.phase === 'out') return
  row.phase = 'out'
  schedule(() => removeFloat(item.key), 900)
}

function spawnOne(): boolean {
  if (active.value.filter((item) => item.phase !== 'out').length >= MAX_VISIBLE) return false

  const label = pickLabel()
  if (!label) return false

  const pos = randomPosition(label)
  if (!pos) return false

  const seed = hash(`${label.id}:${Date.now()}:${Math.random()}`)
  const item: ActiveFloat = {
    ...label,
    key: ++keySeq,
    x: pos.x,
    y: pos.y,
    driftSec: 11 + (seed % 8),
    driftDelaySec: -((seed % 90) / 10),
    lifeMs: 3200 + (seed % 2800),
    phase: 'in',
  }

  active.value = [...active.value, item]

  schedule(() => {
    const row = active.value.find((entry) => entry.key === item.key)
    if (row) row.phase = 'live'
  }, 520)

  schedule(() => beginExit(item), item.lifeMs)

  return true
}

function tickSpawn(): void {
  if (labelPool.value.length === 0) return
  spawnOne()
}

function primeFloats(): void {
  const target = Math.min(MAX_VISIBLE, Math.max(4, Math.ceil(labelPool.value.length * 0.35)))
  for (let i = 0; i < target; i++) {
    schedule(() => spawnOne(), i * 180)
  }
}

function floatStyle(item: ActiveFloat): Record<string, string> {
  return {
    left: `${item.x}%`,
    top: `${item.y}%`,
    fontSize: `${item.size}px`,
    '--drift-sec': `${item.driftSec}s`,
    '--drift-delay': `${item.driftDelaySec}s`,
  }
}

</script>

<template>
  <div class="home">
    <button
      v-if="sidebarCollapsed"
      class="home__expand btn btn--ghost"
      v-tip="'展开侧栏'"
      @click="setSidebarCollapsed(false)"
    >
      <Icon name="menu" size="sm" />
    </button>

    <div class="home__bg" aria-hidden="true">
      <span
        v-for="item in active"
        :key="item.key"
        class="home__float"
        :class="[
          `home__float--${item.kind}`,
          item.phase === 'in' && 'home__float--in',
          item.phase === 'live' && 'home__float--live',
          item.phase === 'out' && 'home__float--out',
        ]"
        :style="floatStyle(item)"
      >
        {{ item.text }}
      </span>
    </div>

    <h1 class="home__title">lya</h1>
    <p v-if="labelPool.length === 0" class="home__hint">开始对话、写记忆、配模型后，这里会慢慢热闹起来。</p>
  </div>
</template>

<style scoped>
.home {
  position: relative;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  background: var(--bg);
}

.home__bg {
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
}

.home__float {
  position: absolute;
  max-width: min(320px, 34vw);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
  line-height: 1.25;
  transform: translate(-50%, -50%);
  opacity: 0;
}

.home__float--in {
  animation: home-float-in 0.55s ease-out forwards;
}

.home__float--live {
  opacity: 0.82;
  animation: home-float-drift var(--drift-sec, 14s) ease-in-out var(--drift-delay, 0s) infinite;
}

.home__float--out {
  animation: home-float-out 0.85s ease-in forwards;
}

@keyframes home-float-in {
  from {
    opacity: 0;
    transform: translate(-50%, -50%) translateY(10px);
  }
  to {
    opacity: 0.82;
    transform: translate(-50%, -50%) translateY(0);
  }
}

@keyframes home-float-drift {
  0%,
  100% {
    transform: translate(-50%, -50%) translate(0, 0);
  }
  25% {
    transform: translate(-50%, -50%) translate(6px, -10px);
  }
  50% {
    transform: translate(-50%, -50%) translate(-5px, 8px);
  }
  75% {
    transform: translate(-50%, -50%) translate(8px, 4px);
  }
}

@keyframes home-float-out {
  from {
    opacity: 0.82;
    transform: translate(-50%, -50%) translateY(0);
  }
  to {
    opacity: 0;
    transform: translate(-50%, -50%) translateY(-12px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .home__float--in,
  .home__float--live,
  .home__float--out {
    animation: none;
  }

  .home__float--live {
    opacity: 0.72;
  }
}

.home__float--session {
  color: color-mix(in srgb, var(--info) 55%, var(--text));
}

.home__float--memory {
  color: color-mix(in srgb, var(--accent) 65%, var(--text));
}

.home__float--model {
  color: color-mix(in srgb, var(--text-muted) 70%, var(--text));
  font-family: var(--font-mono);
}

.home__expand {
  position: absolute;
  top: 10px;
  left: 12px;
  z-index: 2;
  color: var(--accent);
  padding: 4px 8px;
}

.home__title {
  position: relative;
  z-index: 1;
  margin: 0;
  font-size: clamp(72px, 16vw, 148px);
  font-weight: 800;
  letter-spacing: 0.06em;
  color: var(--accent);
  text-shadow: var(--text-shadow);
  user-select: none;
}

.home__hint {
  position: relative;
  z-index: 1;
  margin: 16px 0 0;
  max-width: 320px;
  text-align: center;
  color: var(--text-faint);
  font-size: var(--text-sm);
  line-height: var(--leading);
}
</style>
