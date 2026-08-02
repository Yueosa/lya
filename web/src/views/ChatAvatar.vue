<!--
  聊天气泡旁的头像。lya 用 public/icon.jpg，用户走 QQ 头像。
-->

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ role: 'user' | 'assistant' }>()

const ASSISTANT_AVATAR = '/icon.png'
const USER_AVATAR = 'https://q1.qlogo.cn/g?b=qq&nk=1303028790&s=640'

const src = computed(() => (props.role === 'user' ? USER_AVATAR : ASSISTANT_AVATAR))
const label = computed(() => (props.role === 'user' ? '用户' : 'lya'))
</script>

<template>
  <img
    class="chat__avatar"
    :src="src"
    :alt="label"
    loading="lazy"
    decoding="async"
  />
</template>

<style scoped>
.chat__avatar {
  width: 40px;
  height: 40px;
  margin-top: 2px;
  border-radius: 10px;
  object-fit: cover;
  flex-shrink: 0;
  border: var(--border-width) solid var(--border);
  background: var(--surface);
}
</style>
