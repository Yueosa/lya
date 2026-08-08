/**
 * 全局 API 客户端单例。
 *
 * 放在 `app/` 而不是 `app/chat/` 下：会话之外的东西（配置、记忆、工具目录、存储）同样要
 * 发请求，而它们原先是从 `useChat` 那个门面里把 `client` 再导出一次拿到的——门面的用处
 * 是「界面不必认识传输层」，把传输层原封不动地摆在门面上，等于把这件事撤销了。
 *
 * 界面不该 import 这个文件；该 import `app/` 下那几个按领域分的模块。
 */

import { LyaClient } from '../api/client'

export const client = new LyaClient()
