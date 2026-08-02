<!--
  主题预览块：包在带 data-theme 的容器里，可在不切换全局主题的情况下看效果。
  结构与 ChatView / Composer / 侧栏使用同一套全局类名。
-->

<script setup lang="ts">
import Icon from './Icon.vue'

defineProps<{ themeId: string }>()
</script>

<template>
  <div class="theme-preview" :data-theme="themeId">
    <section v-if="themeId === 'mc'" class="panel theme-preview__card">
      <h4 class="theme-preview__title">主菜单</h4>
      <div class="theme-preview__mc-menu">
        <button type="button" class="btn theme-preview__mc-entry theme-preview__mc-entry--wide">
          新的对话
        </button>
        <button type="button" class="btn theme-preview__mc-entry theme-preview__mc-entry--wide">
          对话列表
        </button>
        <button type="button" class="btn theme-preview__mc-entry">记忆</button>
        <button type="button" class="btn theme-preview__mc-entry">工具</button>
        <button type="button" class="btn theme-preview__mc-entry">模型</button>
        <button type="button" class="btn theme-preview__mc-entry">设置</button>
        <button type="button" class="btn theme-preview__mc-entry theme-preview__mc-entry--wide">
          外观
        </button>
      </div>
    </section>

    <section v-else-if="themeId === 'tokyo-night'" class="panel theme-preview__card">
      <h4 class="theme-preview__title">地铁线导航</h4>
      <div class="theme-preview__tokyo-metro">
        <div class="theme-preview__tokyo-track" aria-hidden="true">
          <span class="theme-preview__tokyo-seg theme-preview__tokyo-seg--0" />
          <span class="theme-preview__tokyo-seg theme-preview__tokyo-seg--1" />
          <span class="theme-preview__tokyo-seg theme-preview__tokyo-seg--2" />
        </div>
        <div class="theme-preview__tokyo-stations">
          <div class="theme-preview__tokyo-station theme-preview__tokyo-station--on">
            <span class="theme-preview__tokyo-dot" aria-hidden="true" />
            <span>首页</span>
          </div>
          <div class="theme-preview__tokyo-station">
            <span class="theme-preview__tokyo-dot" aria-hidden="true" />
            <span>对话</span>
          </div>
          <div class="theme-preview__tokyo-station">
            <span class="theme-preview__tokyo-dot" aria-hidden="true" />
            <span>记忆</span>
          </div>
          <div class="theme-preview__tokyo-station">
            <span class="theme-preview__tokyo-dot" aria-hidden="true" />
            <span>工具</span>
          </div>
        </div>
      </div>
    </section>

    <section v-else class="panel theme-preview__card">
      <h4 class="theme-preview__title">侧栏导航</h4>
      <div class="theme-preview__side">
        <div class="theme-preview__side-item theme-preview__side-item--on">记忆</div>
        <div class="theme-preview__side-item">工具</div>
        <div class="theme-preview__side-item">设置</div>
      </div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">按钮</h4>
      <div class="theme-preview__row">
        <button type="button" class="btn">普通</button>
        <button type="button" class="btn btn--primary">主要</button>
        <button type="button" class="btn btn--danger">危险</button>
        <button type="button" class="btn btn--ghost">幽灵</button>
        <button type="button" class="btn btn--on">选中</button>
      </div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">输入</h4>
      <input class="input" placeholder="在这里输入…" />
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">对话</h4>
      <div class="theme-preview__chat">
        <div class="theme-preview__chat-row theme-preview__chat-row--user">
          <div class="bubble bubble--user">用户消息：淡蓝底，不是粉色实心。</div>
        </div>
        <div class="theme-preview__chat-row theme-preview__chat-row--assistant">
          <div class="bubble bubble--assistant">助手回复：accent-soft 底 + 粉色描边。</div>
        </div>
      </div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">输入栏</h4>
      <div class="theme-preview__composer">
        <button type="button" class="btn">DeepSeek</button>
        <textarea class="input theme-preview__composer-input" readonly disabled placeholder="输入消息…" />
        <div class="seg" role="presentation">
          <button type="button" class="seg__btn seg__btn--ask">
            <Icon class="seg__icon" name="modeAsk" size="sm" />
            <span>问答</span>
          </button>
          <button type="button" class="seg__btn seg__btn--edit seg__btn--on">
            <Icon class="seg__icon" name="modeEdit" size="sm" />
            <span>编辑</span>
          </button>
          <button type="button" class="seg__btn seg__btn--agent">
            <Icon class="seg__icon" name="modeAgent" size="sm" />
            <span>代理</span>
          </button>
        </div>
        <button type="button" class="btn btn--primary">发送</button>
      </div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">代码</h4>
      <pre class="hljs theme-preview__code"><code><span class="hljs-keyword">fn</span> <span class="hljs-function">main</span>() {
    <span class="hljs-comment">// 配色与字体走主题 token</span>
    <span class="hljs-built_in">println!</span>(<span class="hljs-string">"你好"</span>, <span class="hljs-number">42</span>);
}</code></pre>
    </section>
  </div>
</template>
