/**
 * 聊天 composable 模块入口。
 *
 * `useChat.ts` 从此 re-export，保持现有 import 路径不变。
 */

export { client } from './client'
export {
  archivedSessions,
  currentId,
  defaultModel,
  defaultWorkMode,
  defaultApiMode,
  focusedHitlId,
  hydrating,
  loading,
  sessions,
  state,
  tree,
} from './state'
export { bootstrap, imageContext, refreshRuntimeDefaults } from './bootstrap'
export { readComposerDraft, writeComposerDraft } from './composerDraft'
export { refreshSessions, createSession } from './sessions'
export {
  timeline,
  meta,
  running,
  canSend,
  pendingHitlId,
  pendingHitl,
  pendingHitlBatch,
  batchPendingHitlIds,
  canSubmitFocusedHitl,
  canNavHitlPrev,
  canNavHitlNext,
  navigateHitlBatch,
  openSession,
  closeSession,
  replyHitl,
} from './subscription'
export { send, stop } from './messaging'
export { elapsed, phase, round } from './turn'
export {
  regenerate,
  editAndResend,
  deleteMessage,
  switchToBranch,
  switchBranch,
  loadTree,
} from './branches'
export {
  models,
  tools,
  loadModels,
  loadTools,
  toggleTool,
  setMode,
  setModel,
  setApiMode,
  readOnly,
} from './settings'
export { setArchived, removeSession, rename, setPersona } from './lifecycle'
