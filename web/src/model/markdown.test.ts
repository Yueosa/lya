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

import { MATH_CLASS } from './math'
import { localImageUrl, renderMarkdown } from './markdown'

const IMAGES = { token: 'tok', home: '/home/me' }
const SESSION = { ...IMAGES, sessionId: 'sess-1' }

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
    // 令牌只该出现在指向图片 API 的地址里
    const html = renderMarkdown(
      '![](https://evil.example/x.png)\n\n![](/home/me/a.png)',
      IMAGES,
    )
    const withToken = [...html.matchAll(/src="([^"]*tok[^"]*)"/g)].map((m) => m[1]!)
    expect(withToken.every((src) => src.startsWith('/api/local-image?'))).toBe(true)
  })

  it('有 sessionId 时本地与远程都走会话媒体端点', () => {
    const html = renderMarkdown(
      '![](https://example.com/a.png)\n\n![](/home/me/a.png)',
      SESSION,
    )
    expect(html).toContain('/api/sessions/sess-1/media/image')
    expect(html).toContain('kind=local')
    expect(html).toContain('kind=web')
    expect(html).not.toContain('/api/local-image')
    expect(html).toContain('class="lya-chat-image"')
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

  it('Markdown 尖括号路径也认', () => {
    const html = renderMarkdown('![x](</home/me/a.png>)', IMAGES)
    expect(html).toContain('/api/local-image?')
    expect(html).toContain(encodeURIComponent('/home/me/a.png'))
  })

  it('percent 编码路径解码后再请求', () => {
    const html = renderMarkdown('![x](/home/me/foo%20bar.png)', IMAGES)
    const match = html.match(/path=([^&"]+)/)
    expect(match).not.toBeNull()
    const path = decodeURIComponent(match![1]!.replace(/\+/g, ' '))
    expect(path).toBe('/home/me/foo bar.png')
  })

  it('有 sessionId 时视频走会话 media 端点并渲染 controls', () => {
    const html = renderMarkdown('![clip](/home/me/a.mp4)', SESSION)
    expect(html).toContain('/api/sessions/sess-1/media/video')
    expect(html).toContain('<video')
    expect(html).toContain('controls')
    expect(html).toContain('class="lya-chat-video"')
  })

  it('有 sessionId 时音频走会话 media 端点', () => {
    const html = renderMarkdown('![song](/home/me/a.mp3)', SESSION)
    expect(html).toContain('/api/sessions/sess-1/media/audio')
    expect(html).toContain('<audio')
    expect(html).toContain('class="lya-chat-audio"')
  })

  it('无 sessionId 时本地音视频不改写', () => {
    const html = renderMarkdown('![clip](/home/me/a.mp4)\n\n![song](/home/me/a.mp3)', IMAGES)
    expect(html).toContain('src="/home/me/a.mp4"')
    expect(html).toContain('src="/home/me/a.mp3"')
    expect(html).not.toContain('/api/sessions/')
  })
})

/** 取出占位元素里的公式原文，模拟 KaTeX 拿到的 `textContent`。 */
function mathNodes(html: string): { display: boolean; text: string }[] {
  const host = document.createElement('div')
  host.innerHTML = html
  return Array.from(host.querySelectorAll('.lya-math')).map((node) => ({
    display: node.getAttribute('data-display') === '1',
    text: node.textContent ?? '',
  }))
}

describe('数学公式', () => {
  it('四种定界符都认，并分清行内与展示', () => {
    // 反斜杠那两种也得认：不少模型默认就吐这个，只认 $ 的话它们的公式全是原文
    expect(mathNodes(renderMarkdown('质能 $E=mc^2$ 关系'))).toEqual([
      { display: false, text: 'E=mc^2' },
    ])
    expect(mathNodes(renderMarkdown('$$a^2+b^2=c^2$$'))).toEqual([
      { display: true, text: 'a^2+b^2=c^2' },
    ])
    expect(mathNodes(renderMarkdown('行内 \\(x+y\\) 好了'))).toEqual([
      { display: false, text: 'x+y' },
    ])
    expect(mathNodes(renderMarkdown('\\[x+y\\]'))).toEqual([{ display: true, text: 'x+y' }])
  })

  it('公式原文一字不差地传下去', () => {
    // KaTeX 拿到的就是这段 textContent，少一个反斜杠就是另一个公式
    const source = '\\frac{1}{2} \\times \\sum_{i=0}^{n} x_i'
    expect(mathNodes(renderMarkdown(`$${source}$`))[0]?.text).toBe(source)
  })

  it('不把中文里的美元号当公式', () => {
    // 「$5 到 $10」被认成公式的话，中间那段文字会整段消失在一个渲染失败的
    // 公式里——比不渲染糟得多
    for (const text of ['这本书 $5 到 $10 不等', '涨到 $100 了', '把 $PATH 打出来', '$ x $']) {
      expect(mathNodes(renderMarkdown(text)), text).toEqual([])
      expect(renderMarkdown(text), text).not.toContain(MATH_CLASS)
    }
  })

  it('代码里的美元号不受影响', () => {
    const inline = renderMarkdown('用 `$HOME` 这个变量')
    expect(inline).toContain('<code>$HOME</code>')
    expect(inline).not.toContain(MATH_CLASS)

    const fenced = renderMarkdown('```bash\necho $$ $PATH\n```')
    expect(fenced).not.toContain(MATH_CLASS)
    expect(fenced).toContain('$PATH')
  })

  it('公式里塞 HTML 也只是文本', () => {
    // 占位元素里是模型写的东西。这里要是漏了转义，公式就成了注入通道
    const html = renderMarkdown('$\\text{<img src=x onerror=alert(1)>}$')

    // 要断言的是它没变成元素，而不是 HTML 里找不到 onerror 这串字符——
    // 那串字符本来就是公式的一部分，转义之后原样待在文本里才是对的
    const host = document.createElement('div')
    host.innerHTML = html
    expect(host.querySelector('img')).toBeNull()
    expect(host.querySelectorAll('*')).toHaveLength(2) // 只有 <p> 和占位的 <span>
    expect(mathNodes(html)[0]?.text).toBe('\\text{<img src=x onerror=alert(1)>}')
  })

  it('没闭合的公式保持原样，不吞后面的正文', () => {
    // 流式输出时公式总会有半截的一刻，那一刻不能把剩下的正文吃掉
    const html = renderMarkdown('先看 $$a+b 然后还有很多话要说')
    expect(html).not.toContain(MATH_CLASS)
    expect(html).toContain('然后还有很多话要说')
  })
})

/**
 * 波浪号与尖号：删除线、下标、上标。
 *
 * 这一组的难点全在中文里 `~` 是个语气符号。GFM 的删除线单个波浪号也认，于是「这样~那样~就
 * 好了」会被当成删除线，正文里莫名其妙缺一块——读的人不会想到是渲染问题。先前的处理是把删除
 * 线整个关掉，但那连没有歧义的 `~~这样~~` 也一起关了。现在按数量分开，见 model/inlineMarks.ts。
 */
describe('行内记号', () => {
  it('双波浪号是删除线', () => {
    // 报上来的 bug：这条一直渲染不出来，因为删除线被整个关掉了
    expect(renderMarkdown('~~删除线~~')).toContain('<del>删除线</del>')
  })

  it('单波浪号包着中文时还是字', () => {
    // 这条是关掉删除线的原因，放开双波浪号之后它必须继续成立
    const html = renderMarkdown('这样~那样~就好了')
    expect(html).not.toContain('<del>')
    expect(html, '也不该变成下标').not.toContain('<sub>')
    expect(html).toContain('这样~那样~就好了')
  })

  it('句尾的语气号原样留着', () => {
    // 「好耶~」「等一下~~」是中文里最常见的用法，一个都不能动
    expect(renderMarkdown('好耶~')).toContain('好耶~')
    expect(renderMarkdown('等一下~~')).toContain('等一下~~')
  })

  it('下标和上标', () => {
    expect(renderMarkdown('H~2~O')).toContain('H<sub>2</sub>O')
    expect(renderMarkdown('E=mc^2^')).toContain('E=mc<sup>2</sup>')
    expect(renderMarkdown('a~n+1~ 和 x~(i)~')).toContain('a<sub>n+1</sub>')
  })

  it('下标只认像下标的内容，中文和长串都不算', () => {
    // 这条就是「~那样~ 不变下标」的一般化：界线画在内容上，不是画在有没有配对上
    expect(renderMarkdown('~中文~')).not.toContain('<sub>')
    expect(renderMarkdown('~这是一个很长的内容超过十二个字符~')).not.toContain('<sub>')
    expect(renderMarkdown('~有 空格~')).not.toContain('<sub>')
  })

  it('删除线里面还能有别的记号', () => {
    // 删除线的正文要再走一遍行内分词，否则里面的粗体和下标都变成原文
    expect(renderMarkdown('~~带 **粗体** 的~~')).toContain('<del>带 <strong>粗体</strong> 的</del>')
    expect(renderMarkdown('~~H~2~O 整段~~'), '懒匹配要吃到最后那对波浪号').toContain(
      '<del>H<sub>2</sub>O 整段</del>',
    )
  })

  it('公式里的尖号还是公式，不被上标抢走', () => {
    // 上标和行内公式都盯着同一段文本，抢错了公式会变成一段带 <sup> 的碎片
    const html = renderMarkdown('$E=mc^2$')
    expect(html).not.toContain('<sup>')
    expect(html).toContain('lya-math')
  })

  it('代码里的波浪号和尖号不受影响', () => {
    expect(renderMarkdown('`H~2~O`')).toContain('<code>H~2~O</code>')
    expect(renderMarkdown('`a^2^`')).toContain('<code>a^2^</code>')
  })

  it('转义之后是字面量', () => {
    const html = renderMarkdown('转义 \\~2\\~ 和 \\^2\\^')
    expect(html).not.toContain('<sub>')
    expect(html).not.toContain('<sup>')
  })
})
