<!--
  显示偏好。

  分两组是因为作用域真的不同，不是为了排版好看：上面那组换个会话依然生效，
  下面那组只管这一个会话。不标出来的话，用户在某个会话里关掉「隐藏思考」，
  换个会话发现又开着，只会以为设置没保存。
-->

<script setup lang="ts">
import { MACHINE_PREF_KEYS, prefs, SESSION_PREF_KEYS } from '../../app/usePrefs'

const LABELS: Record<keyof typeof prefs, string> = {
  followStream: '跟随流式输出',
  codeBlockWrap: '代码块自动换行',
  hideReasoning: '隐藏思考',
  hideTools: '隐藏工具调用',
  hideResolvedHitl: '隐藏已答复的打断',
  hideNotices: '隐藏模式变更',
  autoCollapseAside: '思考块输出结束后自动收起',
}
</script>

<template>
  <div class="session-tab">
    <h4 class="session-tab__group">本机显示</h4>
    <p class="session-tab__note">存在这台浏览器上，所有会话共用。</p>
    <label v-for="key in MACHINE_PREF_KEYS" :key="key" class="session-tab__row">
      <input v-model="prefs[key]" type="checkbox" />
      <span class="session-tab__name">{{ LABELS[key] }}</span>
    </label>

    <h4 class="session-tab__group">本会话</h4>
    <p class="session-tab__note">只影响当前会话，换会话各自记着。</p>
    <label v-for="key in SESSION_PREF_KEYS" :key="key" class="session-tab__row">
      <input v-model="prefs[key]" type="checkbox" />
      <span class="session-tab__name">{{ LABELS[key] }}</span>
    </label>
  </div>
</template>

<style scoped>
.session-tab {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.session-tab__group {
  margin: 12px 0 0;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-muted);
}

.session-tab__group:first-child {
  margin-top: 0;
}

.session-tab__note {
  margin: 0 0 2px;
  font-size: var(--text-xs);
  color: var(--text-faint);
}

.session-tab__row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  cursor: pointer;
}

.session-tab__row:hover {
  background: var(--surface-hover);
}

.session-tab__name {
  flex: 1;
  font-weight: 500;
}
</style>
