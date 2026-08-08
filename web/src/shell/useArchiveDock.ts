/**
 * 归档会话的抽屉。
 *
 * 归档不能和普通会话混在一列里。它们在列表中长得一模一样，只有副标题末尾多两个字，
 * 扫一眼根本分不出来——「这个对话我是不是归档过」于是变成一道要逐行读的题。默认外壳
 * 早就把它拆成了列表底部一个可折叠的抽屉，这里把那套状态抽出来，让别的外壳和会话列表
 * 视图直接用，而不是各自再写一遍（写第二遍的那次，蔚蓝档案外壳干脆整个漏掉了归档，
 * 归档过的对话在那套皮下**完全消失**）。
 */

import { computed, ref, watch } from 'vue'

import { archivedSessions, currentId } from '../app/useChat'
import { readLocal, writeLocal } from '../utils/storage'

const OPEN_KEY = 'lya.sidebar.archived'

export function useArchiveDock() {
  const open = ref(readLocal(OPEN_KEY) === '1')
  const count = computed(() => archivedSessions.value.length)

  /** 当前打开的会话是不是归档里的那一个。 */
  const viewing = computed(() =>
    archivedSessions.value.some((session) => session.id === currentId.value),
  )

  watch(open, (on) => writeLocal(OPEN_KEY, on ? '1' : '0'))

  // 正在看的就是归档里的会话，抽屉却是收着的——那份列表里找不到当前项，
  // 看上去就像它凭空消失了
  watch(
    viewing,
    (on) => {
      if (on) open.value = true
    },
    { immediate: true },
  )

  return { open, count, viewing, items: archivedSessions }
}
