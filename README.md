# 115ShareHub - 115网盘资源库

一款基于 Tauri 2 + Rust + Vue 3 的桌面应用，用于解析115网盘分享链接中的文件元数据，构建本地可搜索的资源库。

## 功能

- **分享链接解析**：输入115分享链接，自动递归解析所有文件/目录信息
- **本地资源库**：文件元数据存储在本地 SQLite 数据库，不依赖网络即可搜索
- **高级筛选搜索**：按文件名、类型（视频/音频/图片/文档等）、大小范围、来源链接多维度筛选
- **实时进度**：解析过程中实时推送进度，大目录不阻塞 UI
- **全文搜索**：基于 SQLite FTS5，支持中文文件名模糊匹配

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2 |
| 后端 | Rust |
| 前端 | Vue 3 + TypeScript + Naive UI |
| 数据库 | SQLite (rusqlite) + FTS5 |
| HTTP | reqwest (rustls) |

## 项目结构

```
├── src/                          # Vue 3 前端
│   ├── views/                    # 页面：仪表盘、分享链接、搜索、设置
│   ├── stores/                   # Pinia 状态管理
│   ├── router/                   # Vue Router (hash mode)
│   ├── types/                    # TypeScript 类型定义
│   └── utils/                    # 工具函数 & Tauri invoke 封装
├── src-tauri/                    # Rust 后端
│   └── src/
│       ├── commands/             # Tauri 命令（分享链接管理、搜索）
│       ├── db/                   # SQLite 数据层（Schema、CRUD、FTS5）
│       └── pan115/               # 115 API 客户端（解析、类型、限速）
└── README.md
```

## 快速开始

### 环境要求

- Node.js 18+
- Rust 1.75+（安装：`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`）
- macOS / Windows / Linux

### 安装 & 运行

```bash
# 克隆仓库
git clone https://github.com/a55ure/115ShareHub.git
cd 115ShareHub

# 安装前端依赖
npm install

# 开发模式
npm run tauri dev

# 生产构建
npm run tauri build
```

## 使用方式

1. **添加分享链接**：在「分享链接」页面点击「添加链接」，输入115分享链接URL和提取码
2. **自动解析**：应用会自动递归解析链接中的所有文件和目录，存入本地数据库
3. **浏览文件**：在「仪表盘」直接浏览所有已解析的文件列表，支持按类型和关键词筛选
4. **高级搜索**：在「搜索」页面使用多维度筛选（类型、大小、来源等）精确查找

## 开发计划 (TODO)

### MVP (当前版本)
- [x] 项目脚手架 (Tauri 2 + Vue 3 + TypeScript)
- [x] SQLite 数据库层 + FTS5 全文搜索
- [x] 115 分享链接 API 客户端（灵活反序列化、限速）
- [x] 递归目录遍历解析（栈式，避免栈溢出）
- [x] 分享链接管理（添加/删除/刷新/实时进度）
- [x] 仪表盘 - 文件列表展示（表格 + 筛选）
- [x] 高级搜索页面（类型/大小/来源/排序）
- [x] 设置页面

### 下一阶段
- [ ] 115 网盘登录（Cookie管理 / 扫码登录）
- [ ] 搜索结果一键转存到自己的115网盘
- [ ] 批量添加分享链接
- [ ] 文件类型图标优化（视频缩略图）
- [ ] 暗色模式完善
- [ ] 导出/导入数据库
- [ ] 搜索历史记录
- [ ] 重复文件检测（基于 SHA1）

### 未来规划
- [ ] 离线下载管理
- [ ] 多网盘支持（阿里云盘、百度网盘等）
- [ ] 资源评分/收藏功能
- [ ] 自动更新（Tauri updater）
- [ ] Windows / Linux 适配测试

## 数据库设计

- `share_links` — 分享链接（share_code, receive_code, title, status, 文件统计）
- `files` — 文件元数据（name, size, sha1, file_type, full_path, thumbnail）
- `files_fts` — FTS5 全文搜索虚拟表（支持中文 unicode61 分词）
- `settings` — 应用设置（键值对）

## 致谢

- [AList](https://github.com/alist-org/alist) — 115 API 接口分析参考
- [Tauri](https://tauri.app/) — 跨平台桌面应用框架
- [Naive UI](https://www.naiveui.com/) — Vue 3 组件库

## License

MIT
