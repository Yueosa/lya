/** 把字节数格式化成人类可读的字符串。 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '—'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/** 把 MB 输入转成字节（保存配置用）。 */
export function megabytesToBytes(mb: number): number {
  return Math.round(mb * 1024 * 1024)
}

/** 把字节转成 MB（表单展示用，保留一位小数）。 */
export function bytesToMegabytes(bytes: number): number {
  return Math.round((bytes / (1024 * 1024)) * 10) / 10
}
