# 115ShareHub - 115网盘资源库

一款基于 Tauri 2 + Rust + Vue 3 的桌面应用，用于解析115网盘分享链接中的文件元数据，构建本地可搜索的资源库。

## 功能

- **分享链接解析**：输入115分享链接，自动递归解析所有文件/目录信息
- **本地资源库**：文件元数据存储在本地 SQLite 数据库，不依赖网络即可搜索
- **高级筛选搜索**：按文件名、类型（视频/音频/图片/文档等）、大小范围、来源链接多维度筛选；FTS5 + LIKE 双路模糊搜索，支持部分文件名匹配
- **智能限速 + 反封控**：可配置请求频率（1-10次/秒），WAF 检测自动指数退避（15s/30s/45s），支持代理轮换
- **多代理支持**：HTTP / HTTPS / SOCKS5 代理，配置多个代理后可在被封时自动切换
- **账号登录**：Cookie 登录，登录后可解析带权限的分享、一键转存文件到自己的115网盘
- **一键转存**：登录后，在账号管理中选择目标文件夹，在仪表盘/搜索结果中点击「转存」，文件直接保存到指定目录（非根目录）
- **分页浏览**：仪表盘和搜索页支持分页，可自定义每页条数（10/20/50/100）
- **Mock 模式**：设置 `MOCK_115_API=record` 缓存 API 响应到本地，`=playback` 离线回放，无需网络即可开发调试
- **实时进度**：解析过程中实时推送进度，大目录不阻塞 UI

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2 |
| 后端 | Rust |
| 前端 | Vue 3 + TypeScript + Naive UI |
| 数据库 | SQLite (rusqlite) + FTS5 |
| HTTP | reqwest (rustls + socks) |

## 项目结构

```
├── src/                          # Vue 3 前端
│   ├── views/                    # 页面：仪表盘、分享链接、搜索、账号管理、设置
│   ├── stores/                   # Pinia 状态管理
│   ├── router/                   # Vue Router (hash mode)
│   ├── types/                    # TypeScript 类型定义
│   └── utils/                    # 工具函数 & Tauri invoke 封装
├── src-tauri/                    # Rust 后端
│   └── src/
│       ├── commands/             # Tauri 命令（分享链接、搜索、认证、设置）
│       ├── db/                   # SQLite 数据层（Schema、CRUD、FTS5）
│       └── pan115/               # 115 API 客户端（解析、类型、限速、模拟、认证）
└── README.md
```

## 快速开始

### 环境要求

- Node.js 18+
- Rust 1.75+（安装：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`）
- macOS / Windows / Linux

### 安装 & 运行

```bash
git clone https://github.com/a55ure/115ShareHub.git
cd 115ShareHub
npm install
npm run tauri dev
```

### 生产构建

```bash
npm run tauri build
```

### Mock 模式（离线开发）

```bash
# 录制模式：请求115 API 并缓存响应
MOCK_115_API=record npm run tauri dev

# 回放模式：只读本地缓存，不联网
MOCK_115_API=playback npm run tauri dev
```

## 使用方式

1. **配置代理**：在「设置」→「代理设置」添加代理（HTTP/SOCKS5），避免被115封 IP。支持添加多个代理自动轮换
2. **账号登录**：在「账号管理」粘贴从浏览器复制的115 Cookie，验证后可解锁转存等高级功能
3. **添加分享链接**：在「分享链接」页面点击「添加链接」，输入115分享链接URL和提取码
4. **自动解析**：应用自动递归解析链接中的所有文件和目录存入本地数据库（大型分享可能较慢，可调低限速避免被封）
5. **浏览文件**：在「仪表盘」浏览所有已解析文件，按类型和关键词筛选
6. **高级搜索**：在「搜索」页面多维度筛选（类型、大小、来源等），支持模糊匹配
7. **选择转存目录**：在「账号管理」→「转存目标目录」展开目录树，点击「选择」设定转存目的地
8. **一键转存**：在仪表盘或搜索结果中点击文件行的「转存」按钮，文件直接保存到指定目录

## 开发计划

### 已完成
- [x] 项目脚手架 (Tauri 2 + Vue 3 + TypeScript)
- [x] SQLite 数据库层 + FTS5 全文搜索
- [x] 115 分享链接 API 客户端 + 递归解析
- [x] 分享链接管理（添加/删除/刷新/编辑/实时进度）
- [x] 可配置限速 + WAF 退避 + 代理轮换
- [x] 仪表盘 + 高级搜索页面
- [x] Cookie 登录 + 账号管理
- [x] 一键转存到指定目录（`/share/receive` 接口 cid 即为目标文件夹）
- [x] 多代理支持（HTTP/HTTPS/SOCKS5）
- [x] Mock/录制模式离线开发
- [x] FTS5 + LIKE 双路模糊搜索
- [x] 分页浏览（仪表盘/搜索，支持切换每页条数）

### 下一阶段
- [ ] 扫码登录（qrcodeapi.115.com 端点已配置，待测试）
- [ ] 批量添加分享链接
- [ ] 文件类型图标优化
- [ ] 暗色模式完善
- [ ] 导出/导入数据库
- [ ] 重复文件检测（基于 SHA1）

### 未来规划
- [ ] 离线下载管理
- [ ] 多网盘支持（阿里云盘、百度网盘等）
- [ ] 自动更新（Tauri updater）
- [ ] Windows / Linux 适配测试

## 数据库设计

- `share_links` — 分享链接（share_code, receive_code, title, status, 文件统计）
- `files` — 文件元数据（name, size, sha1, file_type, full_path, thumbnail, parent_id）
- `files_fts` — FTS5 全文搜索虚拟表（unicode61 分词）
- `settings` — 应用设置（键值对）

## 相关参考

- [AList](https://github.com/alist-org/alist) — 115 API 接口分析参考（端点、登录、转存）
- [115driver](https://github.com/SheltonZhu/115driver) — Go 语言 115 驱动
- [TgtoDrive](https://github.com/walkingddd/TgtoDrive) — 115 转存 API 参考（`/share/receive` cid 为目标文件夹）
- [115sharebatchsave](https://github.com/AAlexDing/115sharebatchsave) — 115 批量转存参考
- [Tauri](https://tauri.app/) — 跨平台桌面应用框架
- [Naive UI](https://www.naiveui.com/) — Vue 3 组件库

## License

MIT
