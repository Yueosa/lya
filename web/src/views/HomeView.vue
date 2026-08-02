<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import type { Memory } from '../api/client'
import { archivedSessions, client, models, sessions } from '../app/useChat'
import { setSidebarCollapsed, sidebarCollapsed } from '../app/useShell'
import Icon from '../ui/Icon.vue'

type FloatKind = 'session' | 'memory' | 'model'

interface FloatItem {
  id: string
  text: string
  kind: FloatKind
  x: number
  y: number
  size: number
}

interface FloatBox {
  x: number
  y: number
  w: number
  h: number
}

const TITLE_ZONE: FloatBox = { x: 50, y: 50, w: 44, h: 38 }
const BOX_GAP = 1.8

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

function placeLabels(
  labels: { id: string; text: string; size: number }[],
): Map<string, { x: number; y: number }> {
  const placed: FloatBox[] = [{ ...TITLE_ZONE }]
  const positions = new Map<string, { x: number; y: number }>()

  for (const [index, label] of labels.entries()) {
    const box = estimateBox(label.text, label.size)
    let found = false

    for (let attempt = 0; attempt < 240; attempt++) {
      const seed = hash(`${label.id}:${index}:${attempt}`)
      const candidate: FloatBox = {
        x: 6 + (seed % 880) / 10,
        y: 6 + ((seed >> 10) % 880) / 10,
        ...box,
      }
      if (placed.every((other) => !boxesOverlap(candidate, other))) {
        placed.push(candidate)
        positions.set(label.id, { x: candidate.x, y: candidate.y })
        found = true
        break
      }
    }

    if (found) continue

    for (let row = 0; row < 14 && !found; row++) {
      for (let col = 0; col < 10 && !found; col++) {
        const candidate: FloatBox = {
          x: 8 + col * 8.4,
          y: 7 + row * 6.4,
          ...box,
        }
        if (placed.every((other) => !boxesOverlap(candidate, other))) {
          placed.push(candidate)
          positions.set(label.id, { x: candidate.x, y: candidate.y })
          found = true
        }
      }
    }
  }

  return positions
}

const memories = ref<Memory[]>([])

onMounted(async () => {
  try {
    memories.value = await client.memories()
  } catch {
    memories.value = []
  }
})

function hash(text: string): number {
  let value = 0
  for (let i = 0; i < text.length; i++) value = (value * 31 + text.charCodeAt(i)) | 0
  return Math.abs(value)
}

const floats = computed((): FloatItem[] => {
  const labels: { id: string; text: string; kind: FloatKind; size: number }[] = []

  for (const session of sessions.value) {
    const title = session.title?.trim() || '未命名'
    labels.push({ id: `s-${session.id}`, text: title, kind: 'session', size: 18 + (hash(`s-${session.id}`) % 10) })
  }
  for (const session of archivedSessions.value) {
    const title = session.title?.trim() || '未命名'
    labels.push({ id: `a-${session.id}`, text: title, kind: 'session', size: 18 + (hash(`a-${session.id}`) % 10) })
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

  const capped = labels.slice(0, 120)
  const positions = placeLabels(capped)

  return capped.map((item) => {
    const pos = positions.get(item.id) ?? { x: 50, y: 92 }
    return { ...item, x: pos.x, y: pos.y }
  })
})

/** 条目越多，背景越实、越亮。 */
const richness = computed(() => Math.min(1, floats.value.length / 36))

const bgOpacity = computed(() => 0.5 + richness.value * 0.38)

function floatStyle(item: FloatItem): Record<string, string> {
  return {
    left: `${item.x}%`,
    top: `${item.y}%`,
    fontSize: `${item.size}px`,
    opacity: String(Math.min(0.92, bgOpacity.value * (0.88 + (hash(item.id) % 14) / 100))),
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
        v-for="item in floats"
        :key="item.id"
        class="home__float"
        :class="`home__float--${item.kind}`"
        :style="floatStyle(item)"
      >
        {{ item.text }}
      </span>
    </div>

    <h1 class="home__title">lya</h1>
    <p v-if="floats.length === 0" class="home__hint">开始对话、写记忆、配模型后，这里会慢慢热闹起来。</p>
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
