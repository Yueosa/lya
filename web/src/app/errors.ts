/**
 * 出错了怎么告诉用户。
 *
 * 不放在 `chat/` 下：请求会失败这件事和聊天无关，记忆、工具、模型、配置、存储那几屏
 * 同样在失败，而它们原先各自拼了一套文案——有的只 toast、有的只在页面上留一行、有的
 * 两个都来、还有的静默吞掉。口径散开的代价是用户学不到规律：同样是「读不出来」，有时
 * 弹窗有时不弹，于是没弹的那次会被当成「加载慢」。
 *
 * 这里定的口径是：**改动失败一律弹提示**（用户刚按了按钮，他在等一个回音），**读取失败
 * 留在页面上**（那一屏本来就是空的，位置正好用来说为什么，弹窗反而会飘走）。取字用
 * `errorText`，弹提示用 [`report`]。
 */

import { errorText } from '../api/client'
import { toast } from '../ui/useToast'

export { errorText }

/**
 * 弹一条「什么失败了、为什么」。
 *
 * `what` 是动作名而不是句子：这里会补上「失败：」。传「保存」得到「保存失败：404 …」。
 */
export function report(error: unknown, what: string): void {
  toast(`${what}失败：${errorText(error)}`, 'error')
}
