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
  delay: number
  duration: number
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

function place(id: string, index: number): { x: number; y: number } {
  const seed = hash(`${id}:${index}`)
  let x = 5 + (seed % 900) / 10
  let y = 5 + ((seed >> 10) % 900) / 10
  if (x > 30 && x < 70 && y > 25 && y < 75) {
    x = x < 50 ? 22 + (seed % 8) : 70 + (seed % 8)
  }
  return { x, y }
}

const floats = computed((): FloatItem[] => {
  const labels: { id: string; text: string; kind: FloatKind }[] = []

  for (const session of sessions.value) {
    const title = session.title?.trim() || '未命名'
    labels.push({ id: `s-${session.id}`, text: title, kind: 'session' })
  }
  for (const session of archivedSessions.value) {
    const title = session.title?.trim() || '未命名'
    labels.push({ id: `a-${session.id}`, text: title, kind: 'session' })
  }
  for (const memory of memories.value) {
    labels.push({ id: `m-${memory.id}`, text: memory.title, kind: 'memory' })
  }
  for (const model of models.value) {
    labels.push({ id: `md-${model.id}`, text: model.name, kind: 'model' })
  }

  return labels.slice(0, 120).map((item, index) => {
    const seed = hash(item.id)
    const { x, y } = place(item.id, index)
    return {
      ...item,
      x,
      y,
      size: 18 + (seed % 10),
      delay: (seed % 8000) / 1000,
      duration: 14 + (seed % 9000) / 1000,
    }
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
    animationDuration: `${item.duration}s`,
    animationDelay: `${item.delay}s`,
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
  animation: home-drift ease-in-out infinite;
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

@keyframes home-drift {
  0%,
  100% {
    transform: translate(-50%, -50%) translate(0, 0);
  }

  50% {
    transform: translate(-50%, -50%) translate(10px, -16px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .home__float {
    animation: none;
  }
}
</style>
