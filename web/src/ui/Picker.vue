<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

export interface PickerOption {
  value: string
  label: string
  disabled?: boolean
  hint?: string
}

const props = defineProps<{
  modelValue: string
  options: readonly PickerOption[]
  placeholder?: string
}>()

const emit = defineEmits<{ 'update:modelValue': [value: string] }>()

const open = ref(false)
const root = ref<HTMLElement | null>(null)

const currentLabel = computed(
  () => props.options.find((item) => item.value === props.modelValue)?.label ?? props.placeholder ?? '请选择',
)

function pick(value: string): void {
  open.value = false
  emit('update:modelValue', value)
}

function onDocClick(event: MouseEvent): void {
  if (!open.value || !root.value) return
  if (!root.value.contains(event.target as Node)) open.value = false
}

onMounted(() => document.addEventListener('click', onDocClick))
onUnmounted(() => document.removeEventListener('click', onDocClick))
</script>

<template>
  <div ref="root" class="picker">
    <button type="button" class="btn picker__btn" @click.stop="open = !open">
      <span class="picker__label">{{ currentLabel }}</span>
      <span class="picker__caret">{{ open ? '▴' : '▾' }}</span>
    </button>
    <div v-if="open" class="picker__menu panel">
      <button
        v-for="item in options"
        :key="item.value"
        type="button"
        class="picker__option"
        :class="{ 'picker__option--on': item.value === modelValue }"
        :disabled="item.disabled"
        @click="pick(item.value)"
      >
        <span>{{ item.label }}</span>
        <span v-if="item.hint" class="picker__hint">{{ item.hint }}</span>
      </button>
    </div>
  </div>
</template>
