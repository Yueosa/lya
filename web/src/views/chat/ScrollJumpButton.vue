<!-- 滚动跳转 / 跟随按钮。 -->

<script setup lang="ts">
defineProps<{
  jumpState: 'hidden' | 'following' | 'finished' | 'percent'
  jumpText: string
  jumpTip: string
}>()

defineEmits<{ jump: [] }>()
</script>

<template>
  <button
    v-if="jumpState !== 'hidden'"
    class="chat__jump"
    :class="{
      'chat__jump--follow': jumpState === 'following',
      'chat__jump--done': jumpState === 'finished',
    }"
    v-tip="jumpTip"
    @click="$emit('jump')"
  >
    {{ jumpText }}
  </button>
</template>
