import { ApiError } from '../../api/client'
import { toast } from '../../ui/useToast'

/** 把后端报的错说给用户听。 */
export function report(error: unknown, what: string): void {
  const detail = error instanceof ApiError ? `${error.status} ${error.message}` : String(error)
  toast(`${what}失败：${detail}`, 'error')
}
