/**
 * 列表变了以后，保住「还在列表里」的选中项；没有就落到偏好项或第一项。
 *
 * ModelsView / ToolsView / SessionsView 原先各写一遍同样的 watch，漏掉偏好项
 * （SessionsView 想优先回到正在开着的会话）或空列表清选中，迟早会有一边走偏。
 */

/** 选中的是 id 一类的键。 */
export function keepSelectedKey(
  keys: readonly string[],
  selected: string | null,
  prefer: string | null = null,
): string | null {
  if (!keys.length) return null
  if (selected && keys.includes(selected)) return selected
  if (prefer && keys.includes(prefer)) return prefer
  return keys[0]!
}

/** 选中的是整项。`key` 用来判断「还在不在」。 */
export function keepSelectedItem<T>(
  items: readonly T[],
  selected: T | null,
  key: (item: T) => string,
  prefer: string | null = null,
): T | null {
  if (!items.length) return null
  if (selected && items.some((item) => key(item) === key(selected))) return selected
  if (prefer != null) {
    const hit = items.find((item) => key(item) === prefer)
    if (hit) return hit
  }
  return items[0]!
}
