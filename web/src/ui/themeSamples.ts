/**
 * 主题预览用的假数据。
 *
 * 单独一份是为了让 `ThemePreview.vue` 只管排版：数据长什么样、覆盖到哪些分支，
 * 在这里一眼看得完。
 */

import type { UsageReport } from '../api/client'

/**
 * 一段把 Markdown 各种成分都用上的正文。
 *
 * 预览渲染它用的是**真的** `MarkdownBody`，所以代码块的行号、语言条、复制按钮、
 * 表格、行内代码全是真实产物——不是照着抄一份 `.md-code` 结构。上一版预览就是抄的，
 * 抄完就再没跟上过。
 */
export const SAMPLE_MARKDOWN = `正文里有**加粗**、\`行内代码\`和[链接](https://example.com)。

\`\`\`rust
fn main() {
    // 配色与字体走主题 token
    let greeting = "你好";
    println!("{greeting}，{}", 42);
}
\`\`\`

| 列 | 说明 |
|----|------|
| 表格 | 也要有主题 |

> 引用块看起来该和正文分得开。
`

/** 假的占用报告，用来预览真实的 `StorageBreakdown`。 */
export const SAMPLE_USAGE: UsageReport = {
  root: '~/.lya',
  usage: {
    logical_bytes: 268_435_456,
    physical_bytes: 201_326_592,
    reclaimable_bytes: 134_217_728,
    // 有硬链接才看得到「浅色那一截」，那正是这个组件最要紧的表达
    shared_bytes: 67_108_864,
    file_count: 128,
    linked_file_count: 24,
  },
  sections: [
    {
      id: 'cache',
      label: '媒体缓存',
      usage: {
        logical_bytes: 134_217_728,
        physical_bytes: 67_108_864,
        reclaimable_bytes: 0,
        shared_bytes: 67_108_864,
        file_count: 64,
        linked_file_count: 24,
      },
      children: [
        {
          id: 'cache-image',
          label: '图片',
          usage: {
            logical_bytes: 134_217_728,
            physical_bytes: 67_108_864,
            reclaimable_bytes: 0,
            shared_bytes: 67_108_864,
            file_count: 64,
            linked_file_count: 24,
          },
        },
      ],
    },
    {
      id: 'theme',
      label: '主题资源',
      usage: {
        logical_bytes: 67_108_864,
        physical_bytes: 67_108_864,
        reclaimable_bytes: 67_108_864,
        shared_bytes: 0,
        file_count: 32,
        linked_file_count: 0,
      },
      children: [
        {
          id: 'theme.ba',
          label: 'ba',
          usage: {
            logical_bytes: 67_108_864,
            physical_bytes: 67_108_864,
            reclaimable_bytes: 67_108_864,
            shared_bytes: 0,
            file_count: 32,
            linked_file_count: 0,
          },
          children: [
            {
              id: 'theme.ba.cg',
              label: '记忆大厅',
              usage: {
                logical_bytes: 67_108_864,
                physical_bytes: 67_108_864,
                reclaimable_bytes: 67_108_864,
                shared_bytes: 0,
                file_count: 32,
                linked_file_count: 0,
              },
            },
          ],
        },
      ],
    },
    {
      id: 'db',
      label: '数据库',
      usage: {
        logical_bytes: 67_108_864,
        physical_bytes: 67_108_864,
        reclaimable_bytes: 67_108_864,
        shared_bytes: 0,
        file_count: 32,
        linked_file_count: 0,
      },
    },
  ],
}
