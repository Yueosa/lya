# 会话媒体缓存（img / vdo / ado）

活文档。描述 `~/.lya/sessions/{session_id}/` 下的媒体缓存布局与 HTTP 接口。

## 目录布局

```
~/.lya/sessions/{session_id}/
├── img_cache/
│   ├── local/     # 家目录内本地图片（硬链接或复制）
│   └── web/       # 远程 https 图片（下载）
├── vdo_cache/
│   ├── local/     # 本地视频
│   └── web/       # 远程视频
└── ado_cache/
    ├── local/     # 本地音频
    └── web/       # 远程音频
```

- **local**：Markdown 引用家目录绝对路径时，首次访问通过会话 media 端点写入缓存；原文件移动/删除后仍可从缓存播放。
- **web**：Markdown 引用 `http(s)://` 时，服务端抓取并缓存；SSRF 规则与 `web_fetch` 一致（仅公网）。
- 缓存文件名：`{sha256 前 8 hex}{原扩展名}`，同一会话内同一路径/URL 复用同一文件。

## 模型如何引用

每轮 system prompt 注入 `=== [界面] 聊天媒体 ===`（见 `lya-prompt::CHAT_MEDIA_HINT`），教模型用 `![描述](路径或 URL)` 引用；扩展名决定渲染为图片、`<video controls>` 或 `<audio controls>`。不依赖专用 tool。

## HTTP 接口

```
GET /api/sessions/{session_id}/media/image
GET /api/sessions/{session_id}/media/video
GET /api/sessions/{session_id}/media/audio
  ?kind=local|web
  &src=<url-encoded 绝对路径或 URL>
  &token=<bootstrap 下发的 image_token>
  &meta=1          # 可选，返回 JSON 元数据而非字节流
```

### 响应（媒体字节）

- `Content-Type`：按扩展名或远端 `Content-Type`
- `Cache-Control: private, max-age=86400`（会话内缓存文件不变）
- 视频/音频支持 **HTTP Range**（`Accept-Ranges: bytes`），供浏览器拖进度条

### 响应（`meta=1`）

```json
{
  "kind": "local",
  "filename": "clip.mp4",
  "copy_path": "/home/user/clip.mp4",
  "copy_url": null,
  "display_url": "/api/sessions/…/media/video?…"
}
```

远程 `copy_url` 为原始 https URL，`copy_path` 为 null。

## 前端

- `ImageContext.sessionId` 存在时，Markdown 媒体改写为上述会话 URL（本地与远程均走缓存端点）。
- 图片：点击 → lightbox（复制路径/URL、保存）。
- 视频/音频：原生 `<video controls>` / `<audio controls>`（暂停、进度、全屏、倍速、下载由浏览器提供）。
- 旧端点 `/api/local-image` 保留，仅图片、且无 session 时兜底。

## 配置（`runtime.toml`）

```toml
[media.image]
max_bytes = 33554432
cache_local = true
cache_web = true

[media.video]
max_bytes = 536870912
cache_local = true
cache_web = true

[media.audio]
max_bytes = 134217728
cache_local = true
cache_web = true
```

## 安全

- 令牌与 `/api/local-image` 相同机制。
- local：仅家目录内，canonicalize 后校验，符号链接解析后再比对。
- web：仅 `http`/`https`；字面 + DNS 解析 SSRF 检查；重定向落地地址再查一遍。
