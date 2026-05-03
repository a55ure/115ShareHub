# Changelog

## [0.5.0] — 2026-05-03

### 新增
- **资源库页面**：新增「资源库」视图，卡片式展示已完成的分享链接，点击进入后可浏览文件夹结构、搜索、转存
- **文件夹转存**：支持转存整个文件夹，直接传递文件夹ID给115 API，保留完整目录结构
- **反WAF优化**：UA轮换（8个真实浏览器UA）、浏览器sec-*指纹头、备用API端点（webapi失败自动切proapi）、native-tls替换rustls（TLS指纹与浏览器一致）
- **WAF拦截提醒**：每次被115拦截时在界面弹出警告提示，显示等待时间和重试进度
- **文件类型扩展**：新增ISO原盘、字幕文件（srt/ass/ssa/sub）类型识别
- **备用API端点**：`webapi.115.com` 返回405时自动切换到 `proapi.115.com`
- **文件夹类型识别**：解析时子文件夹标记为 `folder` 类型，支持按类型筛选和转存
- **URL自动提取提取码**：输入 `https://115cdn.com/s/xxx?password=1234` 格式的链接时，自动从URL参数中提取提取码

### 修复
- **添加链接死锁**：`add_share_link` 中数据库 Mutex 锁未释放导致后续 `get_conn()` 死锁，按钮一直转圈
- **日志页面事件清理**：修复 `onUnmounted` 在 `onMounted` 内部定义导致事件监听器清理不及时
- **删除链接后解析继续**：删除正在解析的链接后，解析任务继续运行导致 FOREIGN KEY constraint failed。现在每次写入数据库前检查链接是否存在
- **日志页面循环引用**：`levelTag` 函数使用 `h()` 返回 VNode 导致循环引用错误，改为模板渲染
- **文件夹筛选无结果**：根目录 `parent_id` 为 `'0'` 但前端传空字符串，导致查询不匹配

### 变更
- 请求间隔从 ~0.35-0.65s 调整为 1.0-4.0s 随机延迟，降低WAF触发概率
- HTTP客户端从 `rustls-tls` 切换为 `native-tls`，TLS指纹更接近真实浏览器
- 退避重试增加±30%随机抖动，避免固定节奏被识别
- reqwest 启用 `gzip` + `brotli` 自动解压

## [0.4.0] — 2026-05-02

### 修复
- **转存到指定目录**：`/share/receive` 接口 `cid` 参数即为目标文件夹 ID，之前误传为分享内的目录 ID 导致文件始终进根目录。参考 TgtoDrive 和 115sharebatchsave 确认接口规范。
- **账号管理页面打不开**：`@vicons/ionicons5` 无 `ChevronRight` 导出（应为 `ChevronForward`），TypeScript 编译报错导致 Vue Router 懒加载失败，点击菜单无反应。
- **分页始终只显示 1 页**：Naive UI `NDataTable` 内建 `pagination` 为客户端分页，根据 `data` 数组长度计算页数而忽略 `itemCount`。改为独立 `NPagination` 组件，正确按服务端返回的 `total` 计算页数。
- **删除正在解析的链接后，后面的链接永远卡在等待中**：每个链接独立 `tokio::spawn` 无协调。改为单任务队列模式——同一时间最多一个链接解析，完成后自动启动下一个 pending 链接，删除链接后也会触发队列。启动时自动恢复崩溃残留的 `parsing` 状态。

### 变更
- 默认每页条数从 50 改为 10
- 仪表盘和搜索页新增每页条数选择器（10/20/50/100）
- `receive_share_file` 移除冗余的 `to_cid` 和 move 兜底逻辑
- 分享链接解析改为单任务队列模式，同一时间最多一个链接解析，避免并发抢锁和 API 限流

### 文档
- README 新增软件截图（review.png）
- README 更新功能描述：转存到指定目录、分页浏览
- README 参考项目新增 TgtoDrive、115sharebatchsave
- 版本号升至 0.4.0

## [0.3.0] — 2026-05-02

### 新增
- **Cookie 登录**：基于 AList 的正确 API 端点（`passportapi.115.com/check/sso` + `my.115.com/?ct=ajax&ac=nav`），支持用户信息展示（用户名、头像、ID）
- **一键转存到网盘**：仪表盘和搜索页面新增「转存」按钮，登录后可一键保存分享文件到自己的115网盘根目录
- **SOCKS5 代理支持**：`reqwest` 启用 `socks` feature，Clash Verge 等本地代理可直接使用
- **FTS5 + LIKE 双路搜索**：模糊搜索同时使用 FTS5 前缀匹配和 LIKE 子串匹配，部分文件名也能命中

### 修复
- **Cookie 登录无用户信息**：原有 `webapi.115.com/user/info` 端点只返回掩码手机号，已替换为 AList 的正确端点
- **SOCKS5 代理无效**：`reqwest` 缺少 `socks` feature 导致 `Proxy::all("socks5://...")` 静默失败回退直连
- **代理配置读取不一致**：`AuthClient` 只读旧格式 `proxy_config`，已改为优先读取新格式 `proxy_configs`
- **二维码端点错误**：`passportapi.115.com` 返回 HTML 拦截页，已改为 `qrcodeapi.115.com`

### 变更
- `AuthClient` 重构：支持代理、WAF 检测、双端点验证

## [0.2.0] — 2026-05-02

### 新增
- **Mock/录制模式**：设置环境变量 `MOCK_115_API=record` 可在真实请求时自动将 API 响应缓存到本地 `test_fixtures/` 目录；`MOCK_115_API=playback` 则完全离线回放，无需网络即可开发和调试。
- **多代理 + 自动轮换**：设置页面支持添加多个代理配置。解析时若某个代理被 115 WAF 拦截，自动切换到下一个代理继续，避免单 IP 被封后整个解析任务中断。
- **Cookie 注入**：已登录状态下，解析分享链接时会自动携带登录 Cookie，享受更高的请求配额。
- **WAF 指数退避**：检测到 115 WAF 拦截（返回 HTML 而非 JSON）后自动等待 15s/30s/45s 重试最多 3 次，而非立即失败。
- **限速持久化**：设置页面的请求限速滑块现在自动保存到数据库，解析时实际生效。

### 修复
- **扫码登录轮询不工作**：前端 `pollQrcodeLogin` 传给 Tauri 的参数名 `uid` 与后端期望的 `token` 不匹配，导致轮询永远失败。已统一为 `token`。
- **限速参数被忽略**：`Pan115Client` 构造函数接收 `requests_per_second` 参数但完全未使用，始终硬编码 500–1500ms 随机间隔。现在根据实际速率动态计算，下限保护 300ms。
- **WAF 拦截导致解析中断**：此前检测到 HTML 响应直接报错退出，现在有退避重试 + 代理轮换双重保护。

### 变更
- 代理存储格式从单个对象升级为数组（`proxy_configs`），同时兼容旧的单对象格式（`proxy_config`）。
- `Pan115Client` 内部重构：HTTP 调用抽取为 `do_fetch()` 方法，响应解析抽取为 `parse_response()`，引入 `ProxyPool` 管理多个 `reqwest::Client`。
- 设置页面代理区域重写，改为可增删的多条代理配置列表。

---

## [0.1.0] — 初始版本

- Tauri 2 + Vue 3 + TypeScript 脚手架
- SQLite 数据库 + FTS5 全文搜索
- 115 分享链接递归解析（栈式遍历）
- 仪表盘、搜索、设置页面
- 基础 HTTP 代理支持（单条）
- 智能限速（Mutex + 时间戳）
