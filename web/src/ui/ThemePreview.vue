<!--
  主题预览：包在带 data-theme 的容器里，不切换全局主题也能看效果。

  # 尽量渲染真组件，别抄样例 markup

  上一版这里是一份手抄的 HTML，抄完就再没跟上过——导航少了「人设」「存储」，输入栏
  还留着早已搬去会话设置的模式段选择，代码块是裸 `<pre class="hljs">` 而真实的代码块
  有语言条、复制按钮和行号。抄一份就意味着每加一个组件都要记得回来补，漏了不报错，
  只是预览悄悄失真——而预览失真的代价，是换主题时看不出哪里没配好。

  所以能给 props 的一律渲染真组件：正文和代码块走 `MarkdownBody`，工具块走
  `CollapsibleBlock`，跳转按钮走 `ScrollJumpButton`，占用条走 `StorageBreakdown`。
  导航从 `NAV_ITEMS` 生成。剩下几处依赖全局会话状态的（输入栏、媒体附注）仍是手写，
  各自注明了对应的真实来源。
-->

<script setup lang="ts">
import CollapsibleBlock from '../views/CollapsibleBlock.vue'
import MarkdownBody from '../views/MarkdownBody.vue'
import ScrollJumpButton from '../views/chat/ScrollJumpButton.vue'
import { NAV_ITEMS } from '../shell/types'
import StorageBreakdown from './StorageBreakdown.vue'
import { SAMPLE_MARKDOWN, SAMPLE_USAGE } from './themeSamples'

defineProps<{ themeId: string }>()

/** MC 主菜单是两列，项数为奇数时最后一个占满整行——和 `McShell` 同一条规则。 */
const entries = NAV_ITEMS.map((item, index) => ({
  ...item,
  wide: NAV_ITEMS.length % 2 === 1 && index === NAV_ITEMS.length - 1,
}))
</script>

<template>
  <div class="theme-preview" :data-theme="themeId">
    <section v-if="themeId === 'mc'" class="panel theme-preview__card">
      <h4 class="theme-preview__title">主菜单</h4>
      <div class="theme-preview__mc-menu">
        <button type="button" class="btn theme-preview__mc-entry theme-preview__mc-entry--wide">
          对话列表
        </button>
        <button
          v-for="entry in entries"
          :key="entry.view"
          type="button"
          class="btn theme-preview__mc-entry"
          :class="{ 'theme-preview__mc-entry--wide': entry.wide }"
        >
          {{ entry.label }}
        </button>
      </div>
    </section>

    <section v-else class="panel theme-preview__card">
      <h4 class="theme-preview__title">侧栏导航</h4>
      <div class="theme-preview__side">
        <div
          v-for="(entry, index) in entries"
          :key="entry.view"
          class="theme-preview__side-item"
          :class="{ 'theme-preview__side-item--on': index === 0 }"
        >
          {{ entry.label }}
        </div>
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
        <button type="button" class="btn" disabled>禁用</button>
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
        <div class="theme-preview__chat-row theme-preview__chat-row--assistant">
          <div class="bubble bubble--assistant bubble--interrupted">中断的回复到这里就停了</div>
        </div>
      </div>
    </section>

    <section class="panel theme-preview__card">
      <!--
        输入栏是这里唯一没法用真组件的地方：`Composer` 的样式是 scoped 的，外面套不上，
        而直接渲染它会把 v-model 绑到全局草稿上——在预览里打字会污染真的输入框。
        所以只保证形状和 token 一致，样式在 ui.css 的 theme-preview__composer* 里。
      -->
      <h4 class="theme-preview__title">输入栏</h4>
      <div class="theme-preview__composer">
        <textarea class="theme-preview__composer-input" rows="1" readonly placeholder="输入消息…" />
        <button type="button" class="btn btn--primary">发送</button>
      </div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">折叠块</h4>
      <CollapsibleBlock icon="tool" label="file_read · 读取 3 个文件" />
      <CollapsibleBlock icon="reasoning" label="思考" streaming busy>
        <p>流式块在输出时展开，结束后自己收起来。</p>
      </CollapsibleBlock>
      <CollapsibleBlock icon="tool" label="bash · 退出码 1" failed>
        <p>失败的块标题走危险色。</p>
      </CollapsibleBlock>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">正文与代码</h4>
      <MarkdownBody :text="SAMPLE_MARKDOWN" />
    </section>

    <section class="panel theme-preview__card">
      <!-- 这两块由 useChatMedia.ts 在运行时插到媒体元素后面，类名跟着那里改 -->
      <h4 class="theme-preview__title">媒体附注</h4>
      <div class="lya-chat-media-path">
        <div class="lya-chat-media-path__line">~/图片/示例.png</div>
        <div class="lya-chat-media-path__line">本地副本：~/.lya/sessions/…/web/a1b2c3.png</div>
      </div>
      <div class="lya-chat-media-error">媒体加载失败：https://example.com/video.mp4</div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">跳到最新</h4>
      <div class="theme-preview__row theme-preview__jumps">
        <ScrollJumpButton jump-state="following" jump-text="跟随" jump-tip="取消跟随" />
        <ScrollJumpButton jump-state="finished" jump-text="完毕" jump-tip="跳到最新" />
        <ScrollJumpButton jump-state="percent" jump-text="62%" jump-tip="跳到最新" />
      </div>
    </section>

    <section class="panel theme-preview__card">
      <h4 class="theme-preview__title">存储占用</h4>
      <StorageBreakdown :report="SAMPLE_USAGE" />
    </section>
  </div>
</template>
