import type { FormQuestion, HitlBlock } from '../api/wire'

export type FormCall = Extract<HitlBlock, { type: 'form' }>

/** 从 form 调用的 arguments 里还原表单结构。 */
export function parseFormCall(args: unknown): FormCall | null {
  if (!args || typeof args !== 'object') return null
  const raw = args as Record<string, unknown>
  const formId = stringField(raw, 'form_id')
  const title = stringField(raw, 'title')
  const questions = parseQuestions(raw['questions'])
  if (!formId || !title || !questions?.length) return null
  return { type: 'form', form_id: formId, title, questions }
}

function stringField(raw: Record<string, unknown>, key: string): string | null {
  const value = raw[key]
  return typeof value === 'string' && value.trim() ? value.trim() : null
}

function parseQuestions(raw: unknown): FormQuestion[] | null {
  if (!Array.isArray(raw) || raw.length === 0) return null
  const questions: FormQuestion[] = []
  for (const item of raw) {
    const question = parseQuestion(item)
    if (!question) return null
    questions.push(question)
  }
  return questions
}

function parseQuestion(raw: unknown): FormQuestion | null {
  if (!raw || typeof raw !== 'object') return null
  const item = raw as Record<string, unknown>
  const id = stringField(item, 'id')
  const text = stringField(item, 'text')
  const kind = item['kind']
  if (!id || !text) return null
  if (kind !== 'single' && kind !== 'multi' && kind !== 'text') return null

  const options = parseOptions(item['options'])
  if ((kind === 'single' || kind === 'multi') && !options?.length) return null
  if (kind === 'text' && options?.length) return null

  return {
    id,
    text,
    kind,
    ...(options?.length ? { options } : {}),
    ...(item['allow_note'] === true ? { allow_note: true } : {}),
  }
}

function parseOptions(raw: unknown) {
  if (raw == null) return undefined
  if (!Array.isArray(raw)) return null
  const options = []
  for (const item of raw) {
    if (!item || typeof item !== 'object') return null
    const option = item as Record<string, unknown>
    const key = stringField(option, 'key')
    const label = stringField(option, 'label')
    if (!key || !label) return null
    options.push({ key, label })
  }
  return options
}
