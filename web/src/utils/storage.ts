/**
 * `localStorage` 的统一入口。
 *
 * 这些访问原先散在七个模块里，每处自己写一遍防护动作，而其中三处写漏了：`useShell`
 * 和 `DefaultShell` 在模块顶层裸调 `localStorage.getItem`，`BranchTree` 的写入没有
 * 兜住异常。前两处的后果不是「偏好丢了」，是**整个模块导入就抛**——组件测试里第一次
 * 撞见就是这样，而隐私模式下的浏览器同样会。
 *
 * 所以这件事只该有一份实现：读不到当没存过，写不进去就算了。内存里的值仍然有效，
 * 用户唯一会察觉的是「下次打开没记住」，那远好过白屏。
 */

/** 读一个字符串键；取不到一律当没存过。 */
export function readLocal(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null
  } catch {
    return null
  }
}

/** 写一个字符串键；`value` 为 `null` 表示删掉。 */
export function writeLocal(key: string, value: string | null): void {
  try {
    if (value === null) globalThis.localStorage?.removeItem(key)
    else globalThis.localStorage?.setItem(key, value)
  } catch {
    // 隐私模式或配额满了，内存里的值仍然有效
  }
}

/**
 * 读一个 JSON 键，与 `defaults` **逐字段合并**。
 *
 * 合并而不是整体替换：以后加了新字段，老用户存的那份不会缺键。
 */
export function readJson<T extends object>(key: string, defaults: T): T {
  const raw = readLocal(key)
  if (raw === null) return { ...defaults }
  try {
    return { ...defaults, ...(JSON.parse(raw) as Partial<T>) }
  } catch {
    // 存进去的不是合法 JSON（手改过、或旧版本格式），当没存过
    return { ...defaults }
  }
}

/** 写一个 JSON 键。 */
export function writeJson(key: string, value: object): void {
  writeLocal(key, JSON.stringify(value))
}
