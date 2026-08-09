/**
 * 把传输层、store 与界面接起来。
 *
 * 实现拆在 `./chat/` 下按职责分模块；此文件只做 re-export，保持 import 路径不变。
 *
 * **这里不导出 `client`。** 门面的用处是「界面不必认识传输层」，把 `client` 原封不动摆在
 * 门面上等于把这件事撤销了——而它真的被撤销过：十个视图从这儿取到 client 自己发请求、自己
 * 管 loading、自己拼错误文案，同一份配置被四处各拉一遍且改完互不刷新。要发请求就去
 * `app/` 下按领域分的那几个模块（`useConfig`、`useCatalog`、`useMemories`、`sessionActions`
 * 以及 `chat/` 里那些）；确实只有一处用、包一层没意义的，直接 import `app/client`。
 */

export {
  bootstrap,
  canSend,
  closeSession,
  createSession,
  currentId,
  defaultModel,
  defaultWorkMode,
  defaultApiMode,
  readComposerDraft,
  writeComposerDraft,
  deleteMessage,
  editAndResend,
  elapsed,
  focusedHitlId,
  hydrating,
  imageContext,
  loadModels,
  loadTools,
  loadTree,
  meta,
  models,
  openSession,
  pendingHitl,
  pendingHitlBatch,
  canSubmitFocusedHitl,
  canNavHitlPrev,
  canNavHitlNext,
  navigateHitlBatch,
  pendingHitlId,
  phase,
  readOnly,
  regenerate,
  refreshRuntimeDefaults,
  refreshSessions,
  resetSessionTools,
  removeSession,
  rename,
  replyHitl,
  round,
  running,
  send,
  sessions,
  sessionsLoading,
  archivedSessions,
  setArchived,
  setMode,
  setModel,
  setApiMode,
  setIdentity,
  setStyle,
  state,
  stop,
  switchBranch,
  switchToBranch,
  timeline,
  toggleTool,
  tools,
  tree,
} from './chat'
