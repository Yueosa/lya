/**
 * 给「读源码的测试」用的小工具。
 *
 * 这个代码库里有一类测试是**问源码而不是问行为**：谁列了会话就得列归档、外壳得给视图留
 * 定位上下文、主题不许把头像改回方的。它们守的是约定，而约定正是下一个人（或下一次重构）
 * 会不小心违反的东西——而违反的表现往往是「某套皮下某个东西凭空消失」，跑起来不报错，
 * 常规单测也照样绿。
 *
 * 只从测试里 import。放在 `src/` 下是为了跟着 tsconfig 走类型检查；没有产品代码引用它，
 * 所以不会进构建产物。
 */

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { resolve } from 'node:path'

/** 列出一个目录下全部源码文件，递归，跳过测试自身。 */
export function listSources(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const path = resolve(dir, entry)
    if (statSync(path).isDirectory()) listSources(path, out)
    else if (/\.(vue|ts)$/.test(entry) && !entry.endsWith('.test.ts')) out.push(path)
  }
  return out
}

/**
 * `src` 下若干目录里的全部源码，按 `src/...` 的相对路径给出内容。
 *
 * 用工作目录定位而不是 `import.meta.url`：后者在这套 vitest 环境里是被改写过的虚拟路径。
 */
export function sourcesIn(...dirs: string[]): { path: string; src: string }[] {
  return dirs.flatMap((dir) =>
    listSources(resolve(process.cwd(), 'src', dir)).map((path) => ({
      path: path.slice(path.indexOf('/src/') + 1),
      src: readFileSync(path, 'utf8'),
    })),
  )
}
