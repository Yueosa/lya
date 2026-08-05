<!--
  聊天气泡旁的头像。lya 用 public/icon.png，用户走 QQ 头像。
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
  <!--
    外面包一层，是为了给主题留装饰位：`<img>` 挂不了伪元素，而蔚蓝档案那套要在助手
    头像上方画一个光环。包装层不带任何视觉，默认主题下它就是个透明壳子。
  -->
  <span class="chat__avatar-wrap" :class="`chat__avatar-wrap--${role}`">
    <img class="chat__avatar" :src="src" :alt="label" loading="lazy" decoding="async" />
  </span>
</template>

<style scoped>
.chat__avatar-wrap {
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
}

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
