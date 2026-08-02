const WEEKDAYS = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']

const GAP_MINUTES = 10

/** 气泡旁短时间：HH:MM */
export function fmtBubbleTime(ts: string | number | Date | null | undefined): string {
  if (!ts) return ''
  try {
    const d = new Date(ts)
    if (Number.isNaN(d.getTime())) return ''
    return formatClock(d)
  } catch {
    return ''
  }
}

/** 悬停显示完整本地时间 */
export function fmtBubbleTooltip(ts: string | number | Date | null | undefined): string {
  if (!ts) return ''
  try {
    const d = new Date(ts)
    if (Number.isNaN(d.getTime())) return ''
    return d.toLocaleString('zh-CN', { hour12: false })
  } catch {
    return ''
  }
}

/**
 * 相邻消息之间应插入的分隔条文案。
 * - 跨日，或同日间隔 > 10 分钟：相对「现在」的日期 + 时间
 */
export function bubbleSeparator(
  prev: string | number | Date | null | undefined,
  curr: string | number | Date | null | undefined,
  now: Date = new Date(),
): string {
  if (!prev || !curr) return ''
  let p: Date
  let c: Date
  try {
    p = new Date(prev)
    c = new Date(curr)
  } catch {
    return ''
  }
  if (Number.isNaN(p.getTime()) || Number.isNaN(c.getTime())) return ''

  const sameDay =
    p.getFullYear() === c.getFullYear() &&
    p.getMonth() === c.getMonth() &&
    p.getDate() === c.getDate()

  if (!sameDay) return formatSeparatorLabel(c, now)

  const gapMin = (c.getTime() - p.getTime()) / 60_000
  if (gapMin > GAP_MINUTES) return formatSeparatorLabel(c, now)
  return ''
}

/** 会话开始 + 间隔较久：居中时间分隔条（精确到分钟）。 */
export function messageTimeSeparator(
  prev: string | number | Date | null | undefined,
  curr: string | number | Date | null | undefined,
  now: Date = new Date(),
): string {
  if (!curr) return ''
  if (!prev) return formatSeparatorLabel(new Date(curr), now)
  return bubbleSeparator(prev, curr, now)
}

function formatClock(date: Date): string {
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

/** 消息日期相对「现在」差几天（按本地日历日，不是 24h 滚动）。 */
function calendarDayDiff(date: Date, now: Date): number {
  const ms = startOfDay(now).getTime() - startOfDay(date).getTime()
  return Math.round(ms / 86_400_000)
}

/**
 * 居中时间块。相对当前查看时间：
 * - 今天 → 今天 14:23
 * - 昨天 → 昨天 14:23
 * - 更早 → 7月12日 周日 14:23
 */
function formatSeparatorLabel(date: Date, now: Date): string {
  if (Number.isNaN(date.getTime())) return ''
  const clock = formatClock(date)
  const diff = calendarDayDiff(date, now)
  if (diff === 0) return `今天 ${clock}`
  if (diff === 1) return `昨天 ${clock}`
  return `${date.getMonth() + 1}月${date.getDate()}日 ${WEEKDAYS[date.getDay()]} ${clock}`
}
