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
  /** 本地媒体的源文件路径。 */
  source_path: string | null
  /** 远程媒体的原始 URL。 */
  origin_url: string | null
  /** 我们自己留的那一份在哪；没留则为 null。 */
  retained_path: string | null
  /** `hardlink`（与源文件共用空间）或 `copy`。 */
  retained_kind: 'hardlink' | 'copy' | null
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

/** 媒体从哪儿来：本地文件路径，或远程 URL。 */
export function mediaOriginText(meta: MediaMeta): string | null {
  return meta.source_path ?? meta.origin_url
}

/**
 * 我们留的那一份在哪，以及它占不占额外空间。
 *
 * 远程媒体只报 URL 的话，「这个视频已经在本地存了 86 MB」这件事界面上完全看不出来；
 * 而本地媒体的副本多数是硬链接，说成「又存了一份」也是骗人。两件事都得说清。
 */
export function mediaRetainText(meta: MediaMeta): string | null {
  if (!meta.retained_path) return null
  // 本地媒体的硬链接副本没有新增占用，路径本身也和源文件是同一份数据，不值得占一行
  if (meta.kind === 'local' && meta.retained_kind === 'hardlink') return null
  return meta.retained_path
}
