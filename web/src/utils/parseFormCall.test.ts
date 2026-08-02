import { describe, expect, it } from 'vitest'

import { parseFormCall } from './parseFormCall'

describe('parseFormCall', () => {
  it('解析完整表单参数', () => {
    const form = parseFormCall({
      form_id: 'test-form',
      title: '测试表单喵～',
      questions: [
        {
          id: 'pet',
          text: '你喜欢猫咪还是狗狗？',
          kind: 'single',
          options: [
            { key: 'cat', label: '猫咪' },
            { key: 'dog', label: '狗狗' },
          ],
        },
        {
          id: 'note',
          text: '随便说点啥',
          kind: 'text',
        },
      ],
    })

    expect(form).toEqual({
      type: 'form',
      form_id: 'test-form',
      title: '测试表单喵～',
      questions: [
        {
          id: 'pet',
          text: '你喜欢猫咪还是狗狗？',
          kind: 'single',
          options: [
            { key: 'cat', label: '猫咪' },
            { key: 'dog', label: '狗狗' },
          ],
        },
        {
          id: 'note',
          text: '随便说点啥',
          kind: 'text',
        },
      ],
    })
  })

  it('单选/多选缺 options 时返回 null', () => {
    expect(
      parseFormCall({
        form_id: 'x',
        title: 't',
        questions: [{ id: 'q', text: '题干', kind: 'single' }],
      }),
    ).toBeNull()
  })
})
