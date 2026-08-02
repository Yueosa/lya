<script setup lang="ts">
import hljs from 'highlight.js/lib/core'
import ini from 'highlight.js/lib/languages/ini'
import { nextTick, onMounted, ref, watch } from 'vue'

hljs.registerLanguage('ini', ini)

const props = defineProps<{ text: string }>()

const root = ref<HTMLElement | null>(null)

function paint(): void {
  const el = root.value
  if (!el) return
  if (!props.text.trim()) {
    el.textContent = ''
    el.className = 'language-ini hljs'
    return
  }
  const { value } = hljs.highlight(props.text, { language: 'ini' })
  el.innerHTML = value
  el.className = 'language-ini hljs'
}

watch(() => props.text, async () => {
  await nextTick()
  paint()
})

onMounted(() => paint())
</script>

<template>
  <pre class="pre-block"><code ref="root" class="language-ini hljs" /></pre>
</template>
