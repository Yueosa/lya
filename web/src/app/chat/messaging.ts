import { report } from './errors'
import { client } from './client'
import { currentId } from './state'

/** 发一条消息。 */
export async function send(text: string): Promise<void> {
  const id = currentId.value
  if (!id || !text.trim()) return
  try {
    await client.sendMessage(id, text)
  } catch (error) {
    report(error, '发送')
  }
}

/** 停掉正在跑的这一轮。 */
export async function stop(): Promise<void> {
  const id = currentId.value
  if (!id) return
  try {
    await client.stop(id)
  } catch (error) {
    report(error, '停止')
  }
}
