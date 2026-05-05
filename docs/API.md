# fufu-rs API 文档

基础路径：`/api`

---

## 认证

`Authorization: Bearer <access_token>`

| Token | 有效期 | 用途 |
|-------|--------|------|
| Access Token | 15 分钟 | 请求认证 |
| Refresh Token | 7 天 | 刷新 token |
| Temp Token | 5 分钟 | 2FA 临时凭证 |

**认证级别：**
- `Claims` — 强制认证，无效 token 返回 401
- `OptionalClaims` — 可选，有则解析无则略过

---

## 错误码

```json
{ "error": { "code": 1001, "message": "错误描述" } }
```

| Code | HTTP | 说明 |
|------|------|------|
| 1001 | 400 | 请求参数错误 |
| 1002 | 401 | 认证失败（邮箱密码错误/token 无效） |
| 1004 | 404 | 资源不存在 |
| 1005 | 409 | 资源冲突 |
| 2001 | 401 | TOTP 验证码错误 |
| 2002 | 401 | 临时令牌已过期 |
| 5001 | 500 | 服务器内部错误 |
| 5002 | 502 | 外部 API 调用失败 |

---

## 1. 系统检查

### GET /api/health

认证：无

```json
// 200
{
  "status": "ok",
  "uptime": 3600,
  "checks": {
    "d1": { "status": "ok", "latency_ms": 12 },
    "kv": { "status": "ok", "latency_ms": 8 },
    "bangumi_api": { "status": "skipped" },
    "anime_garden_api": { "status": "skipped" }
  }
}
```

---

## 2. 公共统计

### GET /api/stats

认证：无。Umami 不可用时返回全零。

```json
// 200
{
  "active_visitors": 5,
  "today": { "pageviews": 120, "visitors": 45, "visits": 60 },
  "last_30_days": { "pageviews": 5000, "visitors": 2000, "visits": 2500 },
  "pageviews_timeline": [
    { "date": "2026-04-05", "pageviews": 150, "sessions": 80 }
  ],
  "deploy_info": {
    "deployed_at": "2026-05-01 08:00:00",
    "deployed_at_epoch": 1746086400,
    "uptime_seconds": 86400,
    "uptime_human": "1天0小时0分0秒"
  }
}
```

---

## 3. 身份认证

### POST /api/auth/register

认证：无（仅无管理员时可用）

```json
// request
{ "username": "admin", "email": "admin@example.com", "password": "your_password" }
// 201
{ "id": "0194f...", "username": "admin", "email": "admin@example.com", "totp_enabled": false, "role": "admin", "created_at": "2026-01-01T00:00:00Z" }
```

### POST /api/auth/login

```json
// request
{ "email": "admin@example.com", "password": "your_password" }
// 200 — 无 2FA
{ "access_token": "eyJ...", "refresh_token": "eyJ..." }
// 200 — 有 2FA
{ "temp_token": "eyJ...", "require_2fa": true }
```

### POST /api/auth/login/2fa

```json
// request
{ "temp_token": "eyJ...", "code": "123456" }
// 200
{ "access_token": "eyJ...", "refresh_token": "eyJ..." }
```

### POST /api/auth/2fa/setup

认证：需登录

```json
// 200
{ "secret": "JBSWY3DPEHPK3PXP", "uri": "otpauth://totp/...?secret=..." }
```

### POST /api/auth/2fa/verify

```json
// request
{ "code": "123456" }
// 200
{ "message": "2FA 已开启" }
```

### POST /api/auth/2fa/disable

```json
// request
{ "password": "your_password" }
// 200
{ "message": "2FA 已关闭" }
```

### GET /api/auth/me

认证：需登录。返回同 `register` 格式的 AdminInfo。

### POST /api/auth/logout

```json
// request
{ "refresh_token": "eyJ..." }
// 200
{ "message": "已登出" }
```

### POST /api/auth/refresh

```json
// request
{ "refresh_token": "eyJ..." }
// 200
{ "access_token": "eyJ...", "refresh_token": "eyJ..." }
```

---

## 4. 管理仪表盘

### GET /api/auth/dashboard

认证：需登录。数据来源：Umami（analytics.fufu.moe）。

```json
// 200
{
  "analytics": {
    "active_visitors": 5,
    "today": { "pageviews": 120, "visitors": 45, "visits": 60 },
    "this_month": { "pageviews": 3000, "visitors": 1200, "visits": 1500 },
    "last_30_days": { "pageviews": 5000, "visitors": 2000, "visits": 2500 },
    "pageviews_timeline": [
      { "date": "2026-04-05", "pageviews": 150, "sessions": 80 }
    ],
    "top_pages": [
      { "name": "/", "count": 500 }
    ],
    "top_referrers": [
      { "name": "Google", "count": 200 }
    ],
    "browsers": [
      { "name": "Chrome", "count": 400 }
    ],
    "os": [
      { "name": "macOS", "count": 300 }
    ],
    "devices": [
      { "name": "Desktop", "count": 450 }
    ],
    "countries": [
      { "name": "CN", "count": 500 }
    ]
  },
  "health": {
    "status": "ok",
    "uptime": 86400,
    "version": "0.1.0",
    "kv": { "status": "ok" }
  },
  "stats": {
    "posts": 10, "friends": 5, "links": 20, "galleries": 3, "bangumi_records": 15
  },
  "deploy_info": {
    "deployed_at": "2026-05-01 08:00:00",
    "deployed_at_epoch": 1746086400,
    "uptime_seconds": 86400,
    "uptime_human": "1天0小时0分0秒"
  },
  "external_apis": [
    { "name": "Bangumi", "status": "ok", "latency_ms": 120 },
    { "name": "Anime Garden", "status": "ok", "latency_ms": 80 },
    { "name": "Baidu Translate", "status": "error", "latency_ms": null }
  ],
  "databases": [
    { "name": "Auth", "binding": "DB_AUTH", "status": "ok", "latency_ms": 4 }
  ]
}
```

---

## 5. 站点设置

CRUD 规则：GET 公开，PUT/POST/DELETE 需登录。删除均为逻辑删除（设 `deleted_at`）。

### 站点信息

#### GET /api/settings/profile
#### PUT /api/settings/profile

```json
// PUT request — 所有字段可选
{ "site_name": "博客名", "subtitle": "副标题", "logo_url": "https://...", "description": "描述", "keywords": "关键词", "icp_beian": "京ICP备xxxxxxxx号" }
// 响应
{
  "data": {
    "id": "0194f...", "site_name": "...", "subtitle": "...", "logo_url": "...",
    "description": "...", "keywords": "...", "icp_beian": "...",
    "created_at": "...", "updated_at": "..."
  }
}
```

### 页脚

#### GET /api/settings/footer
#### PUT /api/settings/footer

```json
// PUT request
{ "content": "HTML 内容", "copyright_text": "© 2026" }
// 响应 { "data": { "id": "...", "content": "...", "copyright_text": "...", "created_at": "...", "updated_at": "..." } }
```

### 页脚链接

#### GET /api/settings/footer-links — 返回数组
#### POST /api/settings/footer-links
#### PUT/DELETE /api/settings/footer-links/{id}

```json
// POST/PUT request
{ "name": "关于", "url": "/about", "sort_order": 0 }
// 响应
{ "id": "0194f...", "name": "...", "url": "...", "sort_order": 0, "created_at": "...", "updated_at": "...", "deleted_at": null }
```

### 社交链接

#### GET /api/settings/social-links — 返回数组
#### POST /api/settings/social-links
#### PUT/DELETE /api/settings/social-links/{id}

```json
// POST/PUT request
{ "platform": "github", "label": "GitHub", "url": "https://...", "icon": "icon-name", "sort_order": 0 }
// 响应
{ "id": "0194f...", "platform": "...", "label": "...", "url": "...", "icon": "...", "sort_order": 0, "created_at": "...", "updated_at": "...", "deleted_at": null }
```

### 公告

#### GET /api/settings/announcements — 返回数组
#### POST /api/settings/announcements
#### PUT/DELETE /api/settings/announcements/{id}

```json
// POST/PUT request
{ "content": "公告内容", "active": true, "sort_order": 0 }
// 响应
{ "id": "0194f...", "content": "...", "active": true, "sort_order": 0, "created_at": "...", "updated_at": "...", "deleted_at": null }
```

---

## 6. 博客文章

### GET /api/posts — 列表

认证：可选（已登录可查非 published 状态）

query: `?page=1&page_size=10&tag=rust&year=2026&status=published`

```json
// 200
{
  "data": [
    {
      "id": "0194f...", "title": "标题", "slug": "article-slug",
      "excerpt": "摘要", "tags": ["rust"], "status": "published",
      "view_count": 42, "created_at": "...", "updated_at": "...",
      "published_at": "..."
    }
  ],
  "total": 100, "page": 1, "page_size": 10, "total_pages": 10
}
```

### POST /api/posts — 创建

认证：需登录。slug 冲突自动加随机后缀。

```json
// request
{ "title": "标题", "slug": "custom-slug", "content": "markdown", "excerpt": "摘要", "tags": ["rust"], "status": "draft", "github_discussion_number": 1 }
// 200 — 完整 Post（含 content）
{ "id": "0194f...", "title": "...", "slug": "...", "content": "...", "excerpt": "...", "tags": ["rust"], "status": "draft", "view_count": 0, "github_discussion_number": 1, "created_at": "...", "updated_at": "...", "published_at": null }
```

### GET /api/posts/{slug} — 详情

公开。返回完整 `Post`（含 `content`）。

### PUT /api/posts/{slug} — 更新

认证：需登录。所有字段可选。

```json
// request
{ "title": "新标题", "content": "新内容", "tags": ["rust"], "status": "published", "published_at": "2026-01-01T00:00:00Z", "github_discussion_number": 1 }
// 200 — 完整 Post
```

### DELETE /api/posts/{slug}

认证：需登录。逻辑删除。**200:** `{ "message": "已删除" }`

### POST /api/posts/{slug}/views

公开。**200:** `{ "message": "ok" }`

### GET /api/posts/{slug}/comments-count

公开。KV 缓存 1 小时。**200:** `{ "count": 5 }`

---

## 7. 全站搜索

### GET /api/search

认证：无。`?q=关键词&page=1&page_size=10`

搜索范围：文章(published)、链接、相册、友人帐(approved)、公告(active)

```json
// 200
{
  "data": [
    { "type": "post", "title": "标题", "url": "/posts/xxx", "snippet": "摘要...", "published_at": "..." },
    { "type": "link", "title": "链接名", "url": "https://...", "snippet": "描述", "published_at": "..." },
    { "type": "gallery", "title": "相册名", "url": null, "snippet": "相册名", "published_at": "..." },
    { "type": "friend", "title": "友链名", "url": "https://...", "snippet": "描述", "published_at": "..." },
    { "type": "announcement", "title": "公告截取", "url": null, "snippet": "完整内容", "published_at": "..." }
  ],
  "total": 50, "page": 1, "page_size": 10, "total_pages": 5, "query": "关键词"
}
```

---

## 8. 点赞

### GET /api/likes/{target_type}/{target_id}
### POST /api/likes/{target_type}/{target_id}

认证：无（基于访客标识，KV 30 天去重）

```json
// 响应
{ "count": 42, "liked": true }
```

---

## 9. 友人帐

### GET /api/friends — 列表

认证：可选。未登录固定返回 `status=approved`。

query: `?page=1&page_size=20&status=approved`

```json
// 200
{
  "data": [
    { "id": "0194f...", "name": "站点名", "url": "https://...", "avatar_url": "https://...",
      "description": "描述", "email": "admin@example.com", "status": "approved",
      "sort_order": 0, "created_at": "...", "updated_at": "...", "deleted_at": null }
  ],
  "total": 10, "page": 1, "page_size": 20, "total_pages": 1
}
```

### POST /api/friends

认证：需登录。自动设为 `pending` 状态。

```json
// request
{ "name": "站点名", "url": "https://...", "avatar_url": "https://...", "description": "描述", "email": "admin@example.com" }
// 200 — 返回 Friend
```

### GET /api/friends/{id}

认证：无。返回 `Friend`。

### PUT /api/friends/{id}

认证：需登录。字段可选。返回 `Friend`。

### DELETE /api/friends/{id}

认证：需登录。**200:** `{ "message": "已删除" }`

### PATCH /api/friends/{id}/status — 审核

认证：需登录。

```json
// request
{ "status": "approved" }  // approved | rejected
// 200 — 返回 Friend
```

---

## 10. 链接收藏

### GET /api/links — 列表

认证：可选。未登录自动过滤 `favorite=0`。

query: `?page=1&page_size=20&tag=rust&favorite=1`

```json
// 200
{
  "data": [
    { "id": "0194f...", "title": "标题", "url": "https://...", "description": "描述",
      "favicon_url": "https://...", "tags": ["tech"], "favorite": 0,
      "sort_order": 0, "created_at": "...", "updated_at": "...", "deleted_at": null }
  ],
  "total": 50, "page": 1, "page_size": 20, "total_pages": 3
}
```

### POST /api/links

认证：需登录。

```json
// request
{ "title": "标题", "url": "https://...", "description": "描述", "favicon_url": "https://...", "tags": ["tech"], "favorite": 0, "sort_order": 0 }
// 200 — 返回 Link
```

### GET /api/links/meta — 标签统计

认证：无。

```json
// 200
{ "tags": [ { "tag": "rust", "count": 10 }, { "tag": "web", "count": 5 } ] }
```

### GET /api/links/{id}

认证：无。返回 `Link`。

### PUT /api/links/{id}

认证：需登录。返回 `Link`。

### DELETE /api/links/{id}

认证：需登录。**200:** `{ "message": "已删除" }`

---

## 11. 相册

### GET /api/galleries — 列表

公开。query: `?page=1&page_size=20&tag=travel`

```json
// 200
{
  "data": [
    { "id": "0194f...", "title": "相册", "cover_path": "/images/cover.jpg",
      "tags": ["travel"], "created_at": "...", "updated_at": "...", "deleted_at": null }
  ],
  "total": 10, "page": 1, "page_size": 20, "total_pages": 1
}
```

### POST /api/galleries

认证：需登录。

```json
// request
{ "title": "相册", "cover_path": "/images/cover.jpg", "tags": ["travel"] }
// 200 — 返回 Gallery
```

### GET /api/galleries/{id} — 详情（含照片）

公开。

```json
// 200
{
  "id": "0194f...", "title": "...", "cover_path": "...", "tags": ["travel"],
  "created_at": "...", "updated_at": "...",
  "photos": [
    { "id": "0194f...", "gallery_id": "0194f...", "path": "/images/p1.jpg", "created_at": "...", "deleted_at": null }
  ]
}
```

### PUT /api/galleries/{id}

认证：需登录。字段可选。返回 `Gallery`。

### DELETE /api/galleries/{id}

认证：需登录。**200:** `{ "message": "已删除" }`

### POST /api/galleries/{id}/photos

认证：需登录。

```json
// request
{ "paths": ["/images/p1.jpg", "/images/p2.jpg"] }
// 200
[
  { "id": "0194f...", "gallery_id": "0194f...", "path": "/images/p1.jpg", "created_at": "...", "deleted_at": null }
]
```

### DELETE /api/photos/{id}

认证：需登录。**200:** `{ "message": "已删除" }`

---

## 12. 法律文档

### GET /api/license — 最新版本
### POST /api/license — 创建（需登录）
### GET /api/license/versions — 版本历史

```json
// LicenseVersion
{ "id": "0194f...", "version": "1.0", "content": "许可证内容...", "created_at": "..." }
// POST request
{ "version": "1.0", "content": "许可证内容..." }
```

### GET /api/privacy — 最新版本
### POST /api/privacy — 创建（需登录）
### GET /api/privacy/versions — 版本历史

```json
// PrivacyVersion
{ "id": "0194f...", "version": "1.0", "date": "2026-01-01", "content": "隐私政策内容...", "created_at": "..." }
// POST request
{ "version": "1.0", "date": "2026-01-01", "content": "隐私政策内容..." }
```

---

## 13. 番剧记录

### GET /api/bangumi/records

公开。`?page=1&page_size=20&status=watching&subject_id=12345`

状态枚举：`want_to_watch` / `watching` / `completed` / `on_hold` / `dropped`

```json
// 200
{
  "data": [
    { "id": "0194f...", "subject_id": 12345, "title": "番剧名", "status": "watching",
      "progress": "第12话", "cover_url": "https://...", "fansub": "字幕组",
      "added_at": "...", "updated_at": "...", "deleted_at": null }
  ],
  "total": 50, "page": 1, "page_size": 20, "total_pages": 3
}
```

### POST /api/bangumi/records

认证：需登录。

```json
// request
{ "subject_id": 12345, "title": "番剧名", "status": "watching", "progress": "第1话", "cover_url": "https://...", "fansub": "字幕组" }
// 200 — 返回 BangumiRecord
```

### PUT /api/bangumi/records/{id}

认证：需登录。字段可选。返回 `BangumiRecord`。

### DELETE /api/bangumi/records/{id}

认证：需登录。**200:** `{ "message": "已删除" }`

---

## 14. 外部 API 代理

所有代理接口将上游响应原样返回，通过 KV 缓存减少调用。

| 端点 | 上游 | 缓存 |
|------|------|------|
| `POST /api/bangumi/search` | `POST https://api.bgm.tv/v0/search/subjects` | 2h |
| `GET /api/bangumi/subjects/{id}` | `GET https://api.bgm.tv/v0/subjects/{id}` | 24h |
| `GET /api/bangumi/calendar` | `GET https://api.bgm.tv/calendar` | 4h |
| `GET /api/bangumi/browse` | `GET https://api.bgm.tv/v0/subjects` 透传 query | 2h |
| `GET /api/anime-garden/resources` | `GET https://api.animes.garden/resources` 透传 query | 2h |
| `POST /api/translate` | `POST https://fanyi-api.baidu.com/api/trans/vip/translate` | 无 |

### POST /api/bangumi/search

```json
// request
{ "keyword": "搜索词", "sort": "match", "filter": { "type": [1, 2] }, "limit": 20, "offset": 0 }
```

### POST /api/translate

```json
// request
{ "text": "要翻译的文本", "from": "auto", "to": "en" }
```

---

## 15. 垃圾桶

所有接口需登录。

**支持资源：** `posts` / `friends` / `links` / `galleries` / `bangumi`（或 `bangumi_records`/`bangumi-records`）

### GET /api/trash/{resource}

query: `?page=1&page_size=20`

返回已逻辑删除的记录（`deleted_at IS NOT NULL`），按 `deleted_at` 降序。字段按资源类型预定义（不含大字段如文章 content）。

```json
// 200
{
  "data": [
    { "id": "...", "title": "...", "deleted_at": "...", ... }
  ],
  "total": 10, "page": 1, "page_size": 20, "total_pages": 1
}
```

### DELETE /api/trash/{resource}/{id}

真删除（仅对已逻辑删除的记录生效）。**200:** `{ "message": "已永久删除" }`

### POST /api/trash/{resource}/{id}/restore

恢复（清空 `deleted_at`）。**200:** `{ "message": "已恢复" }`
