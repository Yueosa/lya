/**
 * Minecraft 主题用的 Zpix 点阵字：按需加载，默认 release 构建可从 dist 里拿掉以减小二进制。
 */
let loading: Promise<void> | null = null

/** MC 主题激活时调用；字体文件不存在时静默回退到 CSS 里的等宽栈。 */
export function ensureMcFont(): Promise<void> {
  if (document.getElementById('lya-mc-font')) return Promise.resolve()
  loading ??= loadMcFont()
  return loading
}

async function loadMcFont(): Promise<void> {
  try {
    const { default: url } = await import('../assets/fonts/zpix.ttf?url')
    const res = await fetch(url, { method: 'HEAD' })
    if (!res.ok) return

    const style = document.createElement('style')
    style.id = 'lya-mc-font'
    style.textContent =
      `@font-face{font-family:'Zpix';src:url('${url}') format('truetype');font-display:swap}`
    document.head.appendChild(style)
  } catch {
    // dist 里裁掉了字体、或离线 HEAD 失败：用 mc.css 里的 fallback 即可
  }
}
