/** 按 key 分组，保持插入顺序。 */
export function groupBy<T>(items: readonly T[], keyOf: (item: T) => string): [string, T[]][] {
  const map = new Map<string, T[]>()
  for (const item of items) {
    const key = keyOf(item)
    const bucket = map.get(key)
    if (bucket) bucket.push(item)
    else map.set(key, [item])
  }
  return [...map.entries()]
}
