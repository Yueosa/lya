/**
 * 聊天媒体的元数据。
 *
 * 图片走灯箱、视频音频走浏览器自带播放器，两条路差别很大；但「这个文件到底在
 * 哪」的答案格式三者一致，所以把取元数据和路径文案抽在这里，免得两处各写一份
 * 结构体、字段改名时漏掉一边。
 */

/** 媒体接口 `?meta=1` 的返回。字段名跟后端 `MediaMeta` 对齐，用蛇形。 */
export interface MediaMeta {
  kind: 'local' | 'web'
  filename: string
  copy_path: string | null
  copy_url: string | null
  display_url: string
}

/** 是不是本会话媒体接口的地址——只有这种地址才有元数据可查。 */
export function isSessionMediaSrc(src: string): boolean {
  return src.includes('/api/sessions/') && src.includes('/media/')
}

/** 拉元数据。调用方负责兜住异常：拿不到元数据不该影响媒体本身的播放。 */
export async function fetchMediaMeta(displayUrl: string): Promise<MediaMeta> {
  const url = `${displayUrl}${displayUrl.includes('?') ? '&' : '?'}meta=1`
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${response.status}`)
  return response.json() as Promise<MediaMeta>
}

/** 路径条与「复制路径」显示什么：本地路径优先，没有就退回原始 URL。 */
export function mediaPathText(meta: MediaMeta): string | null {
  return meta.copy_path ?? meta.copy_url
}
