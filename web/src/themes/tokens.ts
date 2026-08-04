/**
 * 主题 token 契约。
 *
 * # 为什么要有这份清单
 *
 * 上一代（lianclaw）的 CSS 里引用了 `--border` 和 `--surface`，但 `:root` 从没
 * 定义过它们——浏览器静默回退，没人发现。没有明确契约就必然会这样：写着写着
 * 就引用了不存在的变量。所以这份清单是**可执行**的，`tokens.test.ts` 会检查
 * 每套主题都定义了全部 token，以及组件里没有引用清单外的变量。
 *
 * # 为什么 token 不只是颜色
 *
 * 对比现有的两套风格就会发现，它们的差别根本不在调色板上：
 *
 * - MTF：`-3px 3px 0 <粉>` **硬偏移阴影**，8px 圆角，2px 边
 * - Minecraft：无圆角，硬边，点阵字
 *
 * 那个偏移阴影是 MTF 的灵魂，不是配色的一部分。如果 token 层只管颜色，另一套
 * 主题就得整套重写组件 CSS——那等于把界面做两遍，而不是换主题。所以阴影、圆角、
 * 边框宽度、控件尺寸、字体全都是 token。
 *
 * # 组件的规矩
 *
 * **组件只写布局，不写外观。** flex、grid、尺寸关系留在组件里；颜色、圆角、
 * 阴影、边框、字体一律走这里的变量。写死一个十六进制色值，就等于给未来的主题
 * 埋一个必须特判的地方。
 */

/**
 * 组件局部 CSS 变量的前缀。
 *
 * 组件偶尔需要自己的变量——比如一个可拖宽的面板要把宽度传给样式。那类变量不是
 * 主题的一部分，主题也不该定义它们。约定加上这个前缀，检查器才分得清「引用了
 * 不存在的 token」和「这是组件自己的变量」；不然每加一个局部变量测试就红一次，
 * 最后只会有人把测试改松。
 */
export const LOCAL_PREFIX = 'local-'

/** 一个 token 的定义。 */
export interface TokenSpec {
  /** 变量名，不含 `--` 前缀。 */
  name: string
  /** 它表示什么，以及主题该怎么给值。 */
  doc: string
}

function group(doc: string, names: Record<string, string>): TokenSpec[] {
  return Object.entries(names).map(([name, own]) => ({ name, doc: `${doc}：${own}` }))
}

/** 底色与层次。 */
const surfaces = group('底色', {
  bg: '页面最底层',
  'bg-sunken': '比页面更低一层，如侧边栏、代码块背景',
  surface: '浮在页面上的面板、卡片、气泡',
  'surface-hover': '面板悬停',
  'surface-active': '面板选中',
  // 深色主题下遮罩要压得更狠才看得出层次，浅色主题浅一点就够，所以是 token
  overlay: '对话框背后的遮罩',
})

/** 文字。 */
const text = group('文字', {
  text: '正文',
  'text-muted': '次要信息，如时间、说明',
  'text-faint': '几乎不需要被读到的，如占位符',
  'on-accent': '压在强调色上的文字',
})

/**
 * 线条。
 *
 * `border-width` 是 token 而不是写死 1px：MTF 用 2px 粉线，Minecraft 要更粗。
 */
const lines = group('线条', {
  border: '常规描边',
  'border-strong': '需要强调的描边，如聚焦、选中',
  'border-width': '描边宽度',
  'border-accent-width': '侧栏选中条、引用块等 accent 描边宽度',
})

/** 强调色与语义色。 */
const accents = group('语义色', {
  accent: '主强调色',
  'accent-soft': '强调色的淡背景，用于选中态、聚焦光晕',
  danger: '破坏性操作、错误',
  'danger-soft': '错误的淡背景',
  success: '成功、正在运行',
  warning: '需要注意',
  info: '中性提示，如工具调用',
})

/**
 * 圆角。
 *
 * 气泡单列出来，因为形状差异很大：MTF 是均匀 18px 无尾巴，Minecraft 是 0，
 * 而带尖尾巴的风格要靠单独的角半径才做得出来。
 */
const radii = group('圆角', {
  'radius-sm': '小控件，如按钮、输入框',
  'radius-md': '面板、卡片',
  'radius-lg': '大面板、对话框',
  'radius-pill': '胶囊形，如标签、分段控件',
  'bubble-radius': '消息气泡',
  'bubble-tail-radius': '气泡指向说话人那一角；与 bubble-radius 相同即为无尾巴',
})

/**
 * 阴影。
 *
 * 整条 CSS 值都由主题给，所以「柔和模糊」和「硬偏移」可以共存于同一个变量。
 */
const shadows = group('阴影', {
  'shadow-card': '静止的卡片；可以是 none',
  'shadow-float': '浮层，如对话框、下拉菜单、右键菜单',
  'shadow-focus': '输入框等元素聚焦',
  'shadow-button': '主按钮等可点击控件的硬阴影',
  'shadow-tooltip': '悬停提示，比卡片更轻',
})

/** 字体。 */
const fonts = group('字体', {
  'font-ui': '界面字体',
  'font-mono': '等宽字体，用于代码与命令',
  'text-xs': '最小号，如角标',
  'text-sm': '次要信息',
  'text-md': '正文',
  'text-lg': '标题',
  leading: '正文行高',
  // Minecraft 的招牌是白字加一圈硬投影，那既不是颜色也不是字号，但少了它整个
  // 风格就不成立。另外两套主题给 none
  'text-shadow': '文字投影',
})

/**
 * 控件尺寸。
 *
 * 从上一代学来的：统一三档高度之后，按钮、输入框、下拉在一行里能自然对齐，
 * 不必每处手调 padding。
 */
const controls = group('控件', {
  'sidebar-width': '侧栏宽度',
  'split-list-width': 'split-view 左侧列表宽度',
  'ctl-h-sm': '小号控件高度',
  'ctl-h-md': '中号控件高度',
  'ctl-h-lg': '大号控件高度',
  'ctl-pad-x-sm': '小号控件左右内边距',
  'ctl-pad-x-md': '中号控件左右内边距',
  'ctl-pad-x-lg': '大号控件左右内边距',
})

/** 动效。 */
const motion = group('动效', {
  transition: '常规过渡',
  'duration-fast': '快速过渡，如关闭、离开',
  'duration-normal': '常规进入动画',
})

/**
 * 代码高亮。
 *
 * highlight.js 的配色**必须跟着主题走**，否则浅色主题里代码块还是深色的。
 * 上一代是静态 import 一个 `tokyo-night-dark.css`，换不了。这里把它降成
 * 七个 token，由每套主题自己给值。
 */
const code = group('代码高亮', {
  'code-keyword': '关键字',
  'code-string': '字符串',
  'code-number': '数字与常量',
  'code-comment': '注释',
  'code-function': '函数名',
  'code-type': '类型与类名',
  'code-variable': '变量与属性',
})

/** 全部 token。 */
export const TOKENS: TokenSpec[] = [
  ...surfaces,
  ...text,
  ...lines,
  ...accents,
  ...radii,
  ...shadows,
  ...fonts,
  ...controls,
  ...motion,
  ...code,
]

/** 只要名字，检查用。 */
export const TOKEN_NAMES: readonly string[] = TOKENS.map((token) => token.name)
