/**
 * 浮层定位。
 *
 * 单独抽出来是因为这是**唯一有分支的部分**——贴边翻转、够不下就压边——而它
 * 完全不需要 DOM 就能测。混在组件里的话，验证「靠右键点击时菜单会往左弹」
 * 就得先造一个视口。
 */

/** 一个矩形区域。 */
export interface Size {
  width: number
  height: number
}

/** 放置结果，单位 px，相对视口。 */
export interface Placement {
  left: number
  top: number
}

/** 离视口边缘留多少。 */
const MARGIN = 8

/**
 * 把浮层放在锚点右下方；放不下就翻到另一侧。
 *
 * 翻转优先于压边：右键点在屏幕右缘时，菜单往左展开比贴着右边挤更自然，也不会
 * 盖住鼠标。只有翻过去还是放不下（浮层比视口一半还宽）才退化成压边。
 */
export function placeNear(anchor: Placement, size: Size, viewport: Size): Placement {
  return {
    left: axis(anchor.left, size.width, viewport.width),
    top: axis(anchor.top, size.height, viewport.height),
  }
}

function axis(start: number, extent: number, limit: number): number {
  // 正常方向放得下
  if (start + extent + MARGIN <= limit) return start
  // 翻到另一侧
  const flipped = start - extent
  if (flipped >= MARGIN) return flipped
  // 两边都放不下，压到边上，并保证不会跑到视口外面去
  return Math.max(MARGIN, limit - extent - MARGIN)
}
