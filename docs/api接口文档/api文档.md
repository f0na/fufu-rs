# fufu-rs API 文档

> 基于 Cloudflare Workers + axum 的个人博客后端 API

**基础路径**: `/api`

---

## 目录

1. [通用说明](#通用说明)
2. [健康检查](#1-健康检查)
3. [身份验证](#2-身份验证)
4. [站点设置](#3-站点设置)
5. [博客文章](#4-博客文章)
6. [点赞系统](#5-点赞系统)
7. [友人帐](#6-友人帐)
8. [链接收藏](#7-链接收藏)
9. [相册](#8-相册)
10. [法律文档](#9-法律文档)
11. [番剧记录](#10-番剧记录)
12. [外部 API 代理](#11-外部-api-代理)
13. [全站搜索](#12-全站搜索)
14. [翻译](#13-翻译)
15. [仪表盘](#14-仪表盘)
16. [垃圾桶](#15-垃圾桶)

---

## 通用说明

### 认证方式

- **Bearer Token**: 在请求头中添加 `Authorization: Bearer <access_token>`
- **access_token** 有效期 15 分钟
- **refresh_token** 有效期 7 天，用于刷新 access_token

### 通用错误响应格式

```json
{
  "error": {
    "code": 1001,
    "message": "错误描述"
  }
}
```

### 业务错误码

| code | 说明 |
|------|------|
| 1001 | 请求参数错误 |
| 1002 | 认证失败（邮箱/密码错误） |
| 1003 | 需要 TOTP 第二步验证 |
| 1004 | 资源不存在 |
| 1005 | 资源冲突（重复注册等） |
| 1006 | 请求频率超限 |
| 2001 | TOTP 验证码错误 |
| 2002 | 邮箱验证码错误或已过期 |
| 2003 | 临时令牌已过期 |
| 5001 | 服务器内部错误 |
| 5002 | 外部 API 调用失败 |

### HTTP 状态码

| 状态码 | 说明 |
|--------|------|
| 200 | 成功 |
| 400 | 请求参数错误 |
| 401 | 未认证/认证失败 |
| 404 | 资源不存在 |
| 409 | 冲突 |
| 429 | 请求频率超限 |
| 500 | 服务器内部错误 |
| 502 | 外部 API 调用失败 |

### 分页说明

列表接口统一支持分页参数：

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `page` | int | 1 | 页码，从 1 开始 |
| `page_size` | int | 10~20 | 每页数量，最大 100 |

分页响应统一格式：

```json
{
  "data": [...],
  "total": 100,
  "page": 1,
  "page_size": 20,
  "total_pages": 5
}
```

---

## 1. 健康检查

### GET /api/health

健康检查（检测 D1、KV 连通性）。

**认证**: 不需要

**响应**:

```json
{
  "status": "ok",
  "uptime": 12345,
  "checks": {
    "d1": { "status": "ok", "latency_ms": 12 },
    "kv": { "status": "ok", "latency_ms": 5 },
    "bangumi_api": { "status": "skipped" },
    "anime_garden_api": { "status": "skipped" }
  }
}
```

---

## 2. 身份验证

### POST /api/auth/register

首次注册管理员（仅限尚无管理员时使用）。

**认证**: 不需要

**请求**:

```json
{
  "username": "admin",
  "email": "admin@example.com",
  "password": "your-password"
}
```

**响应** `201`:

```json
{
  "id": "uuid-v7",
  "username": "admin",
  "email": "admin@example.com",
  "totp_enabled": false,
  "role": "admin",
  "created_at": "2026-01-01T00:00:00Z"
}
```

### POST /api/auth/login

第一步：邮箱密码验证。根据管理员配置，返回需要 TOTP 或邮箱验证码。

**认证**: 不需要

**请求**:

```json
{
  "email": "admin@example.com",
  "password": "your-password"
}
```

**响应**（需要 TOTP）:

```json
{
  "temp_token": "jwt-temp-token",
  "require_2fa": true,
  "require_email_verify": false
}
```

**响应**（需要邮箱验证码）:

```json
{
  "temp_token": "jwt-temp-token",
  "require_2fa": false,
  "require_email_verify": true
}
```

### POST /api/auth/login/2fa

第二步：TOTP 验证登录。

**认证**: 不需要

**请求**:

```json
{
  "temp_token": "jwt-temp-token",
  "code": "123456"
}
```

**响应**:

```json
{
  "access_token": "jwt-access-token",
  "refresh_token": "jwt-refresh-token"
}
```

### POST /api/auth/login/verify

第二步：邮箱验证码验证登录。

**认证**: 不需要

**请求**:

```json
{
  "temp_token": "jwt-temp-token",
  "code": "123456"
}
```

**响应**:

```json
{
  "access_token": "jwt-access-token",
  "refresh_token": "jwt-refresh-token"
}
```

### POST /api/auth/refresh

刷新 access_token。

**认证**: 不需要

**请求**:

```json
{
  "refresh_token": "jwt-refresh-token"
}
```

**响应**:

```json
{
  "access_token": "new-access-token",
  "refresh_token": "new-refresh-token"
}
```

### GET /api/auth/me

获取当前登录管理员信息。

**认证**: Bearer Token

**响应**:

```json
{
  "id": "uuid-v7",
  "username": "admin",
  "email": "admin@example.com",
  "totp_enabled": false,
  "role": "admin",
  "created_at": "2026-01-01T00:00:00Z"
}
```

### POST /api/auth/logout

登出（将 refresh_token 加入黑名单）。

**认证**: Bearer Token

**请求**:

```json
{
  "refresh_token": "jwt-refresh-token"
}
```

**响应**:

```json
{
  "message": "已登出"
}
```

### POST /api/auth/2fa/setup

生成 TOTP 密钥和 URI（用于绑定 Authenticator App）。

**认证**: Bearer Token

**响应**:

```json
{
  "secret": "BASE32SECRET",
  "uri": "otpauth://totp/fufu-rs:admin@example.com?secret=...&issuer=fufu-rs"
}
```

### POST /api/auth/2fa/verify

确认开启 2FA（需先调用 setup）。

**认证**: Bearer Token

**请求**:

```json
{
  "code": "123456"
}
```

**响应**:

```json
{
  "message": "2FA 已开启"
}
```

### POST /api/auth/2fa/disable

关闭 2FA（需验证密码）。

**认证**: Bearer Token

**请求**:

```json
{
  "password": "your-password"
}
```

**响应**:

```json
{
  "message": "2FA 已关闭"
}
```

---

## 3. 站点设置

### GET /api/settings/profile

获取站点基本信息。

**认证**: 不需要

**响应**:

```json
{
  "data": {
    "id": "uuid-v7",
    "site_name": "我的博客",
    "subtitle": "个人博客",
    "logo_url": "https://example.com/logo.png",
    "description": "个人技术博客",
    "keywords": "技术,博客,Rust",
    "icp_beian": "京ICP备00000000号",
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z"
  }
}
```

### PUT /api/settings/profile

更新站点基本信息。

**认证**: Bearer Token

**请求**（所有字段可选）:

```json
{
  "site_name": "我的博客",
  "subtitle": "个人博客",
  "logo_url": "https://example.com/logo.png",
  "description": "个人技术博客",
  "keywords": "技术,博客,Rust",
  "icp_beian": "京ICP备00000000号"
}
```

**响应**: 同 GET profile

---

### GET /api/settings/footer

获取页脚配置。

**认证**: 不需要

**响应**:

```json
{
  "data": {
    "id": "uuid-v7",
    "content": "页脚内容",
    "copyright_text": "© 2026 My Blog",
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z"
  }
}
```

### PUT /api/settings/footer

更新页脚配置。

**认证**: Bearer Token

**请求**（所有字段可选）:

```json
{
  "content": "页脚内容",
  "copyright_text": "© 2026 My Blog"
}
```

**响应**: 同 GET footer

---

### GET /api/settings/footer-links

获取页脚链接列表。

**认证**: 不需要

**响应**: `FooterLink[]`

```json
[
  {
    "id": "uuid-v7",
    "name": "关于我们",
    "url": "/about",
    "sort_order": 1,
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z",
    "deleted_at": null
  }
]
```

### POST /api/settings/footer-links

添加页脚链接。

**认证**: Bearer Token

**请求**:

```json
{
  "name": "关于我们",
  "url": "/about",
  "sort_order": 1
}
```

**响应**: `FooterLink`

### PUT /api/settings/footer-links/{id}

更新页脚链接。

**认证**: Bearer Token

**请求**:

```json
{
  "name": "关于我们（更新）",
  "url": "/about",
  "sort_order": 2
}
```

**响应**: `FooterLink`

### DELETE /api/settings/footer-links/{id}

删除页脚链接（逻辑删除）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

### GET /api/settings/social-links

获取社交链接列表。

**认证**: 不需要

**响应**: `SocialLink[]`

```json
[
  {
    "id": "uuid-v7",
    "platform": "GitHub",
    "label": "GitHub",
    "url": "https://github.com/username",
    "icon": "github",
    "sort_order": 1,
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z",
    "deleted_at": null
  }
]
```

### POST /api/settings/social-links

添加社交链接。

**认证**: Bearer Token

**请求**:

```json
{
  "platform": "GitHub",
  "label": "GitHub",
  "url": "https://github.com/username",
  "icon": "github",
  "sort_order": 1
}
```

**响应**: `SocialLink`

### PUT /api/settings/social-links/{id}

更新社交链接。

**认证**: Bearer Token

**请求**:

```json
{
  "platform": "GitHub",
  "label": "GitHub",
  "url": "https://github.com/username",
  "icon": "github",
  "sort_order": 1
}
```

**响应**: `SocialLink`

### DELETE /api/settings/social-links/{id}

删除社交链接（逻辑删除）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

### GET /api/settings/announcements

获取公告列表。

**认证**: 不需要

**响应**: `Announcement[]`

```json
[
  {
    "id": "uuid-v7",
    "content": "站点维护通知",
    "active": true,
    "sort_order": 1,
    "created_at": "2026-01-01T00:00:00Z",
    "updated_at": "2026-01-01T00:00:00Z",
    "deleted_at": null
  }
]
```

### POST /api/settings/announcements

添加公告。

**认证**: Bearer Token

**请求**:

```json
{
  "content": "站点维护通知",
  "active": true,
  "sort_order": 1
}
```

**响应**: `Announcement`

### PUT /api/settings/announcements/{id}

更新公告。

**认证**: Bearer Token

**请求**:

```json
{
  "content": "站点维护通知（更新）",
  "active": true,
  "sort_order": 1
}
```

**响应**: `Announcement`

### DELETE /api/settings/announcements/{id}

删除公告（逻辑删除）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

## 4. 博客文章

### GET /api/posts

获取文章列表。

**认证**: 可选（未登录只能看 published）

**查询参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| page | int | 1 | 页码 |
| page_size | int | 10 | 每页数量（最大 100） |
| tag | string | - | 按标签筛选 |
| year | string | - | 按年份筛选（如 `2026`） |
| status | string | published | 按状态筛选（需登录） |

**响应**: `PaginatedPosts`

```json
{
  "data": [
    {
      "id": "uuid-v7",
      "title": "测试文章",
      "slug": "test-post",
      "excerpt": "文章摘要",
      "tags": ["rust", "web"],
      "status": "published",
      "view_count": 100,
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z",
      "published_at": "2026-01-01T00:00:00Z"
    }
  ],
  "total": 10,
  "page": 1,
  "page_size": 10,
  "total_pages": 1
}
```

### GET /api/posts/{slug}

获取文章详情（含正文 Markdown 内容）。

**认证**: 不需要

**路径参数**: `slug` - 文章 slug

**响应**: `Post`

```json
{
  "id": "uuid-v7",
  "title": "测试文章",
  "slug": "test-post",
  "content": "# 正文 Markdown",
  "excerpt": "文章摘要",
  "tags": ["rust", "web"],
  "status": "published",
  "view_count": 100,
  "github_discussion_number": 1,
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "published_at": "2026-01-01T00:00:00Z"
}
```

### POST /api/posts

创建文章。

**认证**: Bearer Token

**请求**:

```json
{
  "title": "测试文章",
  "slug": "test-post",
  "content": "# 测试\n\n这是一篇测试文章",
  "excerpt": "测试文章摘要",
  "tags": ["rust", "web"],
  "status": "published",
  "github_discussion_number": 1
}
```

> 说明: `slug` 可选，不传则自动从 title 生成；如有冲突自动追加随机后缀。`status` 默认 `draft`，设为 `published` 时自动设置发布时间。

**响应**: `Post`（201）

### PUT /api/posts/{slug}

更新文章。

**认证**: Bearer Token

**路径参数**: `slug` - 文章 slug

**请求**（所有字段可选）:

```json
{
  "title": "测试文章（更新）",
  "slug": "updated-slug",
  "content": "# 更新\n\n内容已更新",
  "excerpt": "更新摘要",
  "tags": ["rust"],
  "status": "published",
  "published_at": "2026-01-01T00:00:00Z",
  "github_discussion_number": 1
}
```

**响应**: `Post`

### DELETE /api/posts/{slug}

逻辑删除文章（移入垃圾桶）。

**认证**: Bearer Token

**路径参数**: `slug` - 文章 slug

**响应**:

```json
{
  "message": "已删除"
}
```

### POST /api/posts/{slug}/views

增加文章浏览量。

**认证**: 不需要

**路径参数**: `slug` - 文章 slug

**响应**:

```json
{
  "message": "ok"
}
```

### GET /api/posts/{slug}/comments-count

获取文章的 GitHub Discussion 评论数。

**认证**: 不需要

**路径参数**: `slug` - 文章 slug

**响应**:

```json
{
  "count": 5
}
```

---

## 5. 点赞系统

### GET /api/likes/{target_type}/{target_id}

获取点赞数和当前访问者是否已点赞。

**认证**: 不需要

**路径参数**: `target_type` - 目标类型（如 `post`），`target_id` - 目标 ID

**响应**:

```json
{
  "count": 10,
  "liked": false
}
```

### POST /api/likes/{target_type}/{target_id}

切换点赞/取消点赞（基于 KV 的访客标识）。

**认证**: 不需要

**路径参数**: `target_type` - 目标类型（如 `post`），`target_id` - 目标 ID

**响应**:

```json
{
  "count": 11,
  "liked": true
}
```

---

## 6. 友人帐

### GET /api/friends

获取友链列表。

**认证**: 可选（未登录只能看 `approved` 状态的）

**查询参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| page | int | 1 | 页码 |
| page_size | int | 20 | 每页数量（最大 100） |
| status | string | approved | 按状态筛选（需登录） |

**响应**: `PaginatedFriends`

```json
{
  "data": [
    {
      "id": "uuid-v7",
      "name": "友链名称",
      "url": "https://example.com",
      "avatar_url": "https://example.com/avatar.png",
      "description": "友链描述",
      "email": "webmaster@example.com",
      "status": "approved",
      "sort_order": 1,
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z",
      "deleted_at": null
    }
  ],
  "total": 5,
  "page": 1,
  "page_size": 20,
  "total_pages": 1
}
```

### GET /api/friends/{id}

获取友链详情。

**认证**: 不需要

**响应**: `Friend`

### POST /api/friends

添加友链（被认证，可改为公开提交）。

**认证**: Bearer Token

**请求**:

```json
{
  "name": "友链名称",
  "url": "https://example.com",
  "avatar_url": "https://example.com/avatar.png",
  "description": "友链描述",
  "email": "webmaster@example.com"
}
```

**响应**: `Friend`（状态自动设为 `pending`）

### PUT /api/friends/{id}

更新友链。

**认证**: Bearer Token

**请求**（所有字段可选）:

```json
{
  "name": "友链名称（更新）",
  "url": "https://example.com",
  "avatar_url": "https://example.com/avatar.png",
  "description": "更新描述",
  "email": "webmaster@example.com",
  "sort_order": 1
}
```

**响应**: `Friend`

### PATCH /api/friends/{id}/status

审核友链状态（通过/拒绝）。

**认证**: Bearer Token

**请求**:

```json
{
  "status": "approved"
}
```

> `status` 取值: `approved` | `rejected`

**响应**: `Friend`

### DELETE /api/friends/{id}

逻辑删除友链（移入垃圾桶）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

## 7. 链接收藏

### GET /api/links

获取收藏链接列表。

**认证**: 可选（未登录只能看非收藏链接，即 `favorite = 0`）

**查询参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| page | int | 1 | 页码 |
| page_size | int | 20 | 每页数量（最大 100） |
| tag | string | - | 按标签筛选 |
| favorite | int | - | 按收藏状态筛选 |

**响应**: `PaginatedLinks`

```json
{
  "data": [
    {
      "id": "uuid-v7",
      "title": "Rust 官方文档",
      "url": "https://www.rust-lang.org/",
      "description": "Rust 编程语言官方网站",
      "favicon_url": "",
      "tags": ["rust", "programming"],
      "favorite": 0,
      "sort_order": 1,
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z",
      "deleted_at": null
    }
  ],
  "total": 20,
  "page": 1,
  "page_size": 20,
  "total_pages": 1
}
```

### GET /api/links/meta

获取链接标签统计信息。

**认证**: 不需要

**响应**:

```json
{
  "tags": [
    { "tag": "rust", "count": 5 },
    { "tag": "programming", "count": 3 }
  ]
}
```

### GET /api/links/{id}

获取链接详情。

**认证**: 不需要

**响应**: `Link`

### POST /api/links

添加收藏链接。

**认证**: Bearer Token

**请求**:

```json
{
  "title": "Rust 官方文档",
  "url": "https://www.rust-lang.org/",
  "description": "Rust 编程语言官方网站",
  "favicon_url": "https://www.rust-lang.org/favicon.ico",
  "tags": ["rust", "programming"],
  "favorite": 0,
  "sort_order": 1
}
```

**响应**: `Link`

### PUT /api/links/{id}

更新收藏链接。

**认证**: Bearer Token

**请求**（所有字段可选）:

```json
{
  "title": "Rust 官方文档（更新）",
  "url": "https://www.rust-lang.org/",
  "description": "更新后的描述",
  "favicon_url": "https://www.rust-lang.org/favicon.ico",
  "tags": ["rust"],
  "favorite": 1,
  "sort_order": 2
}
```

**响应**: `Link`

### DELETE /api/links/{id}

逻辑删除链接（移入垃圾桶）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

## 8. 相册

### GET /api/galleries

获取相册列表。

**认证**: 可选

**查询参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| page | int | 1 | 页码 |
| page_size | int | 20 | 每页数量（最大 100） |
| tag | string | - | 按标签筛选 |

**响应**: `PaginatedGalleries`

```json
{
  "data": [
    {
      "id": "uuid-v7",
      "title": "我的旅行相册",
      "cover_path": "/images/travel/cover.jpg",
      "tags": ["旅行", "摄影"],
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z",
      "deleted_at": null
    }
  ],
  "total": 3,
  "page": 1,
  "page_size": 20,
  "total_pages": 1
}
```

### GET /api/galleries/{id}

获取相册详情（含照片列表）。

**认证**: 不需要

**响应**: `GalleryDetail`

```json
{
  "id": "uuid-v7",
  "title": "我的旅行相册",
  "cover_path": "/images/travel/cover.jpg",
  "tags": ["旅行", "摄影"],
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "photos": [
    {
      "id": "uuid-v7",
      "gallery_id": "uuid-v7",
      "path": "/images/travel/photo1.jpg",
      "created_at": "2026-01-01T00:00:00Z",
      "deleted_at": null
    }
  ]
}
```

### POST /api/galleries

创建相册。

**认证**: Bearer Token

**请求**:

```json
{
  "title": "我的旅行相册",
  "cover_path": "/images/travel/cover.jpg",
  "tags": ["旅行", "摄影"]
}
```

**响应**: `Gallery`

### PUT /api/galleries/{id}

更新相册。

**认证**: Bearer Token

**请求**（所有字段可选）:

```json
{
  "title": "我的旅行相册（更新）",
  "cover_path": "/images/travel/new-cover.jpg",
  "tags": ["旅行"]
}
```

**响应**: `Gallery`

### DELETE /api/galleries/{id}

逻辑删除相册（移入垃圾桶）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

### POST /api/galleries/{id}/photos

向相册添加照片。

**认证**: Bearer Token

**请求**:

```json
{
  "paths": ["/images/travel/photo1.jpg", "/images/travel/photo2.jpg"]
}
```

**响应**: `Photo[]`

```json
[
  {
    "id": "uuid-v7",
    "gallery_id": "uuid-v7",
    "path": "/images/travel/photo1.jpg",
    "created_at": "2026-01-01T00:00:00Z",
    "deleted_at": null
  }
]
```

### DELETE /api/photos/{id}

逻辑删除照片。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

## 9. 法律文档

### GET /api/license

获取最新版本的许可证。

**认证**: 不需要

**响应**: `LicenseVersion`

```json
{
  "id": "uuid-v7",
  "version": "v1.0",
  "content": "## 许可证\n\n版权所有 © 2026",
  "created_at": "2026-01-01T00:00:00Z"
}
```

### GET /api/license/versions

获取许可证版本历史。

**认证**: 不需要

**响应**: `LicenseVersion[]`

### POST /api/license

创建新的许可证版本。

**认证**: Bearer Token

**请求**:

```json
{
  "version": "v1.0",
  "content": "## 许可证\n\n版权所有 © 2026"
}
```

**响应**: `LicenseVersion`

---

### GET /api/privacy

获取最新版本的隐私政策。

**认证**: 不需要

**响应**: `PrivacyVersion`

```json
{
  "id": "uuid-v7",
  "version": "v1.0",
  "date": "2026-01-01",
  "content": "## 隐私政策\n\n我们重视您的隐私",
  "created_at": "2026-01-01T00:00:00Z"
}
```

### GET /api/privacy/versions

获取隐私政策版本历史。

**认证**: 不需要

**响应**: `PrivacyVersion[]`

### POST /api/privacy

创建新的隐私政策版本。

**认证**: Bearer Token

**请求**:

```json
{
  "version": "v1.0",
  "date": "2026-01-01",
  "content": "## 隐私政策\n\n我们重视您的隐私"
}
```

**响应**: `PrivacyVersion`

---

## 10. 番剧记录

### GET /api/bangumi/records

获取番剧记录列表。

**认证**: 不需要

**查询参数**:

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| page | int | 1 | 页码 |
| page_size | int | 20 | 每页数量（最大 100） |
| status | string | - | 按状态筛选（如 `watching`） |
| subject_id | int | - | 按 Bangumi 条目 ID 筛选 |

**响应**: `PaginatedRecords`

```json
{
  "data": [
    {
      "id": "uuid-v7",
      "subject_id": 123,
      "title": "测试番剧",
      "status": "watching",
      "progress": "6/12",
      "cover_url": "https://example.com/cover.jpg",
      "fansub": "字幕组名",
      "added_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z",
      "deleted_at": null
    }
  ],
  "total": 15,
  "page": 1,
  "page_size": 20,
  "total_pages": 1
}
```

### POST /api/bangumi/records

添加番剧记录。

**认证**: Bearer Token

**请求**:

```json
{
  "subject_id": 123,
  "title": "测试番剧",
  "status": "watching",
  "progress": "6/12",
  "cover_url": "https://example.com/cover.jpg",
  "fansub": "字幕组名"
}
```

> `status` 默认值: `want_to_watch`

**响应**: `BangumiRecord`

### PUT /api/bangumi/records/{id}

更新番剧记录。

**认证**: Bearer Token

**请求**（所有字段可选）:

```json
{
  "title": "测试番剧",
  "status": "watched",
  "progress": "12/12",
  "cover_url": "https://example.com/cover.jpg",
  "fansub": "字幕组名"
}
```

**响应**: `BangumiRecord`

### DELETE /api/bangumi/records/{id}

逻辑删除番剧记录（移入垃圾桶）。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已删除"
}
```

---

## 11. 外部 API 代理

### POST /api/bangumi/search

搜索 Bangumi 条目（代理 [bgm.tv](https://bgm.tv) 搜索）。

**认证**: 不需要

**请求**:

```json
{
  "keyword": "进击的巨人",
  "sort": "match",
  "filter": { "type": [1] },
  "limit": 20,
  "offset": 0
}
```

> 兼容旧参数 `type`（整数），自动转为 `filter.type`。结果缓存 1 小时。

**响应**: Bangumi API 原始返回数据（透传）

### GET /api/bangumi/subjects/{id}

获取 Bangumi 条目详情。缓存 24 小时。

**认证**: 不需要

**响应**: Bangumi API 原始返回数据（透传）

### GET /api/bangumi/calendar

获取 Bangumi 每日放送表。缓存 1 小时。

**认证**: 不需要

**响应**: Bangumi API 原始返回数据（透传）

### GET /api/bangumi/browse

浏览 Bangumi 条目（支持筛选参数）。缓存 1 小时。

**认证**: 不需要

**查询参数**: 透传到 Bangumi API

**响应**: Bangumi API 原始返回数据（透传）

### GET /api/anime-garden/resources

获取 AnimeGarden 资源列表。缓存 30 分钟。

**认证**: 不需要

**查询参数**: 透传到 AnimeGarden API

**响应**: AnimeGarden API 原始返回数据（透传）

---

## 12. 全站搜索

### GET /api/search

跨模块全文搜索（文章、链接、相册、友人帐、公告）。

**认证**: 不需要

**查询参数**:

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| q | string | 是 | 搜索关键词（至少 2 个字符） |
| page | int | 否 | 页码，默认 1 |
| page_size | int | 否 | 每页数量，默认 10，最大 100 |

**响应**: `SearchResponse`

```json
{
  "data": [
    {
      "type": "post",
      "title": "测试文章",
      "url": "/posts/test-post",
      "snippet": "文章摘要...",
      "published_at": "2026-01-01T00:00:00Z"
    },
    {
      "type": "link",
      "title": "Rust 官方文档",
      "url": "https://www.rust-lang.org/",
      "snippet": "Rust 编程语言官方网站",
      "published_at": "2026-01-01T00:00:00Z"
    },
    {
      "type": "gallery",
      "title": "我的旅行相册",
      "url": null,
      "snippet": "我的旅行相册",
      "published_at": "2026-01-01T00:00:00Z"
    },
    {
      "type": "friend",
      "title": "友链名称",
      "url": "https://example.com",
      "snippet": "友链描述",
      "published_at": "2026-01-01T00:00:00Z"
    },
    {
      "type": "announcement",
      "title": "站点维护通知...",
      "url": null,
      "snippet": "站点维护通知",
      "published_at": "2026-01-01T00:00:00Z"
    }
  ],
  "total": 10,
  "page": 1,
  "page_size": 10,
  "total_pages": 1,
  "query": "测试"
}
```

> `type` 取值: `post` | `link` | `gallery` | `friend` | `announcement`
> 每个模块各取前 50 条，按相关性排序后分页返回。

---

## 13. 翻译

### POST /api/translate

使用百度翻译 API 进行翻译（中译英）。

**认证**: 不需要（需配置 BAIDU_TRANSLATE_APPID / BAIDU_TRANSLATE_SECRET）

**请求**:

```json
{
  "text": "你好世界",
  "from": "auto",
  "to": "en"
}
```

> `from` 默认 `auto`，`to` 默认 `en`

**响应**: 百度翻译 API 原始返回数据（透传）

---

## 14. 仪表盘

### GET /api/auth/dashboard

获取 Cloudflare Analytics 仪表盘数据（需配置 CF_API_TOKEN / CF_ZONE_ID）。

**认证**: Bearer Token

**响应**:

```json
{
  "today": {
    "requests": 1000,
    "bandwidth": "50.0 MB",
    "avg_duration_ms": 120
  },
  "this_month": {
    "requests": 30000,
    "bandwidth": "1.5 GB"
  },
  "total": {
    "requests": 100000,
    "bandwidth": "5.0 GB"
  },
  "status_codes": {
    "2xx": 90000,
    "4xx": 8000,
    "5xx": 2000
  },
  "health": {
    "status": "ok",
    "uptime": 12345,
    "version": "0.1.0",
    "kv": { "status": "ok", "latency_ms": 5 }
  },
  "stats": {
    "posts": 10,
    "friends": 5,
    "links": 20,
    "galleries": 3,
    "bangumi_records": 15
  },
  "deploy_info": {
    "deployed_at": "2026-05-01 14:30:00",
    "deployed_at_epoch": 1746095400,
    "uptime_seconds": 172800,
    "uptime_human": "2天0小时0分0秒"
  },
  "external_apis": [
    { "name": "Bangumi", "status": "ok", "latency_ms": 120 },
    { "name": "Anime Garden", "status": "ok", "latency_ms": 80 },
    { "name": "Baidu Translate", "status": "error", "latency_ms": null }
  ],
  "worker_metrics": {
    "total_requests": 100000,
    "error_count": 500,
    "error_rate_pct": 0.5,
    "avg_cpu_time_ms": 45.23,
    "errors_by_path": [
      { "path": "/api/auth/login", "status_code": 401, "count": 200 },
      { "path": "/api/posts/not-found", "status_code": 404, "count": 150 }
    ]
  },
  "databases": [
    { "name": "Core", "binding": "FUFU_CORE", "status": "ok", "latency_ms": 5 },
    { "name": "Posts", "binding": "FUFU_POSTS", "status": "ok", "latency_ms": 3 },
    { "name": "Media", "binding": "FUFU_MEDIA", "status": "ok", "latency_ms": 4 },
    { "name": "Bangumi", "binding": "FUFU_BANGUMI", "status": "ok", "latency_ms": 3 },
    { "name": "Social", "binding": "FUFU_SOCIAL", "status": "ok", "latency_ms": 4 },
    { "name": "Likes", "binding": "FUFU_LIKES", "status": "ok", "latency_ms": 3 },
    { "name": "Legal", "binding": "FUFU_LEGAL", "status": "ok", "latency_ms": 5 },
    { "name": "Auth", "binding": "FUFU_AUTH", "status": "ok", "latency_ms": 3 }
  ]
}
```

> `health` 字段中的 `kv` 检测 KV 连通性，D1 检测移至 `databases` 字段覆盖全部 8 个数据库。
> `external_apis` 并行检测 Bangumi、Anime Garden、百度翻译三个外部 API 的连通性。
> `worker_metrics` 中的 `errors_by_path` 来自 Cloudflare GraphQL Analytics，非 2xx 响应按路径和状态码分组。
> `databases` 列出全部 8 个 D1 数据库的健康状态和延迟。

---

## 15. 垃圾桶

垃圾桶管理逻辑删除的资源（统一管理 posts / friends / links / galleries / bangumi 的回收）。

### GET /api/trash/{resource}

列出指定资源的垃圾桶内容。

**认证**: Bearer Token

**路径参数**:

| 参数 | 说明 |
|------|------|
| `resource` | 资源类型: `posts`, `friends`, `links`, `galleries`, `bangumi`(或 `bangumi_records`, `bangumi-records`) |

**查询参数**: 支持 `page`、`page_size`

**响应**: `PaginatedTrash`

```json
{
  "data": [
    {
      "id": "uuid-v7",
      "title": "已删除的文章",
      "deleted_at": "2026-01-01T00:00:00Z"
    }
  ],
  "total": 2,
  "page": 1,
  "page_size": 20,
  "total_pages": 1
}
```

### POST /api/trash/{resource}/{id}/restore

从垃圾桶恢复资源。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已恢复"
}
```

### DELETE /api/trash/{resource}/{id}

从数据库中永久删除资源。

**认证**: Bearer Token

**响应**:

```json
{
  "message": "已永久删除"
}
```

---

## 附录

### 全局速率限制

- **100 次请求 / 60 秒窗口**（基于 IP）
- 超出返回 `429 Too Many Requests`（错误码 1006）

### 数据库分库设计

| 数据库绑定 | 用途 |
|-----------|------|
| `FUFU_CORE` | 站点信息、页脚、社交链接、公告 |
| `FUFU_POSTS` | 博客文章 |
| `FUFU_MEDIA` | 相册、照片 |
| `FUFU_BANGUMI` | 番剧记录 |
| `FUFU_SOCIAL` | 友人帐、收藏链接 |
| `FUFU_LIKES` | 点赞数据 |
| `FUFU_LEGAL` | 法律文档 |
| `FUFU_AUTH` | 管理员、登录日志、验证码 |
