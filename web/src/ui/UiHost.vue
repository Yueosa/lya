<!--
  三个浮层的宿主，整个应用挂一次。

  弹窗、右键菜单、轻提示都是全局单例，命令式调用（见各自的 use*.ts）。它们
  共用一个宿主是因为都要挂在 body 层：分散挂载会被祖先的 `overflow: hidden`
  或 `transform` 裁掉，那类问题极难查。
-->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import './ui.css'
import {
  closeContextMenu,
  menuState,
  reposition,
  selectItem,
  toRows,
} from './useContextMenu'
import { accept, cancel, dialogState, setValue } from './useDialog'
import { dismissToast, toastState } from './useToast'

const menuEl = ref<HTMLElement | null>(null)
const inputEl = ref<HTMLInputElement | null>(null)
const menuRows = computed(() => toRows(menuState.entries))

// 菜单渲染出来才量得到宽高，量到之后再校正位置，免得贴边时跑出视口
watch(
  () => menuState.open,
  async (open) => {
    if (!open) return
    await nextTick()
    const el = menuEl.value
    if (!el) return
    reposition(
      { width: el.offsetWidth, height: el.offsetHeight },
      { width: window.innerWidth, height: window.innerHeight },
    )
  },
)

// 输入框弹出来就聚焦并全选，用户可以直接改而不用先点一下
watch(
  () => dialogState.open && dialogState.kind === 'prompt',
  async (ready) => {
    if (!ready) return
    await nextTick()
    inputEl.value?.select()
  },
)

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') cancel()
  // 输入框里回车即确认，和原生 prompt 的手感一致
  if (event.key === 'Enter' && dialogState.kind === 'prompt') void accept()
}
</script>

<template>
  <!-- 弹窗 -->
  <div
    v-if="dialogState.open"
    class="overlay"
    @click.self="cancel"
    @keydown="onKeydown"
  >
    <div class="dialog" role="dialog" aria-modal="true" tabindex="-1">
      <h2 class="dialog__title">{{ dialogState.title }}</h2>
      <p v-if="dialogState.message" class="dialog__message">{{ dialogState.message }}</p>

      <input
        v-if="dialogState.kind === 'prompt'"
        ref="inputEl"
        class="input"
        :value="dialogState.value"
        :placeholder="dialogState.placeholder"
        :disabled="dialogState.busy"
        @input="setValue(($event.target as HTMLInputElement).value)"
      />

      <p v-if="dialogState.error" class="dialog__error">{{ dialogState.error }}</p>

      <div class="dialog__actions">
        <button class="btn" :disabled="dialogState.busy" @click="cancel">
          {{ dialogState.cancelText }}
        </button>
        <button
          class="btn"
          :class="dialogState.danger ? 'btn--danger' : 'btn--primary'"
          :disabled="dialogState.busy"
          @click="accept"
        >
          {{ dialogState.busy ? '处理中…' : dialogState.confirmText }}
        </button>
      </div>
    </div>
  </div>

  <!-- 右键菜单。背板吃掉一次点击用于关闭，也拦住误触下层 -->
  <template v-if="menuState.open">
    <div class="ctx-backdrop" @click="closeContextMenu" @contextmenu.prevent="closeContextMenu" />
    <div ref="menuEl" class="ctx-menu" :style="{ left: `${menuState.left}px`, top: `${menuState.top}px` }">
      <template v-for="row in menuRows" :key="row.key">
        <div v-if="row.kind === 'separator'" class="ctx-menu__separator" />
        <button
          v-else
          class="ctx-menu__item"
          :class="{ 'ctx-menu__item--danger': row.item.danger }"
          :disabled="row.item.disabled"
          @click="selectItem(row.item)"
        >
          <span v-if="row.item.icon">{{ row.item.icon }}</span>
          <span>{{ row.item.label }}</span>
        </button>
      </template>
    </div>
  </template>

  <!-- 轻提示 -->
  <div class="toasts">
    <div
      v-for="item in toastState.items"
      :key="item.id"
      class="toast"
      :class="`toast--${item.kind}`"
      @click="dismissToast(item.id)"
    >
      {{ item.message }}
    </div>
  </div>
</template>
