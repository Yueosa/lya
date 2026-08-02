/** 从 JSON Schema 抽出参数字段，供工具页展示。 */
export interface SchemaField {
  name: string
  type: string
  required: boolean
  description?: string
}

export function schemaFields(schema: Record<string, unknown> | undefined): SchemaField[] {
  if (!schema) return []
  const required = new Set(
    Array.isArray(schema['required']) ? (schema['required'] as string[]) : [],
  )
  const props = schema['properties']
  if (!props || typeof props !== 'object') return []
  return Object.entries(props as Record<string, Record<string, unknown>>).map(([name, spec]) => {
    const field: SchemaField = {
      name,
      type: formatType(spec),
      required: required.has(name),
    }
    if (typeof spec['description'] === 'string') field.description = spec['description']
    return field
  })
}

function formatType(spec: Record<string, unknown>): string {
  if (typeof spec['type'] === 'string') return spec['type']
  if (Array.isArray(spec['type'])) return spec['type'].join(' | ')
  return 'any'
}
