# 会话媒体缓存（img_cache）

活文档。描述 `~/.lya/sessions/{session_id}/` 下的媒体缓存布局与 HTTP 接口。

## 目录布局

```
~/.lya/sessions/{session_id}/
├── img_cache/
│   ├── local/     # 家目录内本地图片（硬链接或复制）
│   └── web/       # 远程 https 图片（下载）
├── vdo_cache/     # （预留）视频
└── ado_cache/     # （预留）音频
```

- **local**：Markdown 引用家目录绝对路径时，首次访问通过 `/api/sessions/{id}/media/image` 写入缓存；原文件移动/删除后仍可从缓存显示。
- **web**：Markdown 引用 `http(s)://` 图片时，服务端抓取并缓存；SSRF 规则与 `web_fetch` 一致（仅公网）。
- 缓存文件名：`{sha256 前 16 hex}{原扩展名}`，同一会话内同一路径/URL 复用同一文件。

## HTTP 接口

```
GET /api/sessions/{session_id}/media/image
  ?kind=local|web
  &src=<url-encoded 绝对路径或 URL>
  &token=<bootstrap 下发的 image_token>
  &meta=1          # 可选，返回 JSON 元数据而非字节流
```

### 响应（图片）

- `Content-Type`：按扩展名或远端 `Content-Type`
- `Cache-Control: private, max-age=86400`（会话内缓存文件不变）

### 响应（`meta=1`）

```json
{
  "kind": "local",
  "filename": "photo.jpg",
  "copy_path": "/home/user/photo.jpg",
  "copy_url": null,
  "display_url": "/api/sessions/…/media/image?…"
}
```

远程图 `copy_url` 为原始 https URL，`copy_path` 为 null。

## 前端

- `ImageContext.sessionId` 存在时，Markdown 图片改写为上述会话媒体 URL（本地与远程均走缓存端点）。
- 点击图片 → lightbox：复制图片（ClipboardItem）、复制路径/URL、保存（同源 fetch + download）。
- 旧端点 `/api/local-image` 保留，供无 session 上下文场景；聊天内优先会话缓存。

## 安全

- 令牌与 `/api/local-image` 相同机制。
- local：仅家目录内，canonicalize 后校验，符号链接解析后再比对。
- web：仅 `http`/`https`；字面 + DNS 解析 SSRF 检查；重定向落地地址再查一遍。
