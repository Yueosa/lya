/**
 * @vitest-environment jsdom
 *
 * 这一份必须跑在 jsdom 上，不能用别处那个更快的 happy-dom：DOMPurify 在
 * happy-dom 下会**剥掉每个片段的最外层元素**（`<p>a</p>` 变成 `a`、`<ul>` 只剩
 * `<li>`）。后果不只是断言写不对——`<script>x</script>` 也会因为同一个原因只剩
 * 文字，于是「挡住了脚本注入」这条会**因为错误的理由通过**，而消毒到底有没有
 * 生效根本测不出来。这里是防 XSS 的边界，不能靠一个测不准的环境。
 */

import { describe, expect, it } from 'vitest'

import { localImageUrl, renderMarkdown } from './markdown'

const IMAGES = { token: 'tok', home: '/home/me' }

describe('renderMarkdown', () => {
  it('渲染常见语法', () => {
    const html = renderMarkdown('# 标题\n\n**粗**与 `码`\n\n- 一\n- 二')
    expect(html).toContain('<h1')
    expect(html).toContain('<strong>粗</strong>')
    expect(html).toContain('<code>码</code>')
    expect(html).toContain('<li>')
  })

  it('松散列表不把空段落留进 HTML', () => {
    const html = renderMarkdown('- 一\n\n- 二\n\n\n')
    expect(html).not.toMatch(/<p>\s*<\/p>/)
    expect(html).toContain('<li>')
    expect(html).not.toMatch(/<li>\s*<p>/)
  })

  it('列表项之间空行会被收紧', () => {
    const html = renderMarkdown('- ✅ a\n\n- ✅ b')
    expect(html).toMatch(/<li>✅ a<\/li>\s*<li>✅ b<\/li>/)
  })

  it('不把中文里的波浪号当删除线', () => {
    // 中文里 ~ 用得很随意，误判成删除线会让正文莫名其妙缺一块
    const html = renderMarkdown('这样~那样~就好了')
    expect(html).not.toContain('<del>')
  })

  it('挡住脚本注入', () => {
    // 正文是模型生成的，而模型读过的网页里可能藏着东西。这个页面手里有图片
    // 令牌、能调所有本地接口，一次 XSS 的代价不小
    const html = renderMarkdown('<script>alert(1)</script>正常内容')
    expect(html).not.toContain('<script')
    expect(html).toContain('正常内容')
  })

  it('挡住事件属性与 style', () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)">')
    expect(html).not.toContain('onerror')

    // 一段 CSS 足以把整个界面盖住做成钓鱼页
    const styled = renderMarkdown('<style>body{display:none}</style>还在')
    expect(styled).not.toContain('<style')
    expect(styled).toContain('还在')
  })

  it('挡住换了花样的注入', () => {
    // 正文是模型生成的，而模型读过的网页里可能藏着这些。挨个试一遍，别只测
    // 最直白的那种 <script>
    const attempts = [
      '[点我](javascript:alert(1))',
      '<a href="javascript:alert(1)">点我</a>',
      '<svg onload="alert(1)"></svg>',
      '<iframe src="https://evil.example"></iframe>',
      '<object data="evil.swf"></object>',
      '<img src="x" onmouseover="alert(1)">',
      '<a href="data:text/html,<script>alert(1)</script>">点我</a>',
      '<form action="https://evil.example"><input name="a"></form>',
    ]
    for (const attempt of attempts) {
      const html = renderMarkdown(attempt)
      expect(html, attempt).not.toMatch(/javascript:/i)
      expect(html, attempt).not.toMatch(/\son\w+=/i)
      expect(html, attempt).not.toMatch(/<(script|iframe|object|embed|form)\b/i)
    }
  })

  it('本地图片端点的令牌不会被别的地址顺走', () => {
    // 令牌只该出现在指向 /api/local-image 的地址里。要是它跟着一个外部地址
    // 出去了，等于把访问家目录图片的钥匙发给了别人
    const html = renderMarkdown(
      '![](https://evil.example/x.png)\n\n![](/home/me/a.png)',
      IMAGES,
    )
    const withToken = [...html.matchAll(/src="([^"]*tok[^"]*)"/g)].map((m) => m[1]!)
    expect(withToken.every((src) => src.startsWith('/api/local-image?'))).toBe(true)
  })

  it('把家目录内的图片改写成后端接口', () => {
    const html = renderMarkdown('![猫](/home/me/图片/猫.png)', IMAGES)
    expect(html).toContain('/api/local-image?')
    expect(html).toContain('token=tok')
    expect(html).toContain(encodeURIComponent('/home/me/图片/猫.png'))
  })

  it('file:// 也认', () => {
    const html = renderMarkdown('![](file:///home/me/a.png)', IMAGES)
    expect(html).toContain('/api/local-image?')
  })

  it('家目录外的路径不改写', () => {
    // 后端也会拒，提前挡掉省一次必然失败的往返
    const html = renderMarkdown('![](/etc/passwd.png)', IMAGES)
    expect(html).not.toContain('/api/local-image')
  })

  it('远程图片原样放行', () => {
    const html = renderMarkdown('![](https://example.com/a.png)', IMAGES)
    expect(html).toContain('https://example.com/a.png')
    expect(html).not.toContain('/api/local-image')
  })

  it('没有令牌时不硬拼一个必然 403 的地址', () => {
    const html = renderMarkdown('![](/home/me/a.png)')
    expect(html).not.toContain('/api/local-image')
  })

  it('localImageUrl 把家目录内路径转成接口地址', () => {
    const url = localImageUrl('/home/me/Pictures/icon.jpg', IMAGES)
    expect(url).toContain('/api/local-image?')
    expect(url).toContain('token=tok')
  })
})
