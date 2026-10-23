# 变更记录

## [0.2.1] — 未发布

- 新增三平台单插件 GUI ZIP，Desktop Code 可通过 Upload local plugin 安装；
  程序使用 `server/` 路径，无 marketplace 外层，Unix ZIP 保存执行权限。
- 保留现有 marketplace 格式，打包器新增 `--format plugin-zip`，冒烟测试新增
  `--archive PATH`；两个格式共用 MCP、Skill 和默认 Hook，不增加运行时依赖。
- 新增 GUI 安装、同名替换及系统安全提示说明；CI 验证两种压缩包布局和运行闭环，
  后续标签发布同时提供三个 MCP 包、三个 marketplace 包及三个 GUI ZIP。

## [0.2.0] — 2026-10-23

- 新增本地 Rust Hook 模式：SessionStart、SubagentStart 和 PreToolUse；不启动
  MCP、不联网、不修改文件或读取正文。
- 默认保护 root 内的 Read/Edit/Write/Grep；保留特殊格式读取、Glob 和 Shell，
  路径及输入错误拒绝处理，MCP 调用不会递归拦截。
- Windows x64、Linux x64 GNU 和 macOS Apple Silicon 离线 Claude 插件：
  自带平台原生程序、Skill 和默认 Hook，通过本地
  marketplace 安装；新增独立 MCP Hook 配置、实施备忘、安装和保护说明。
- Windows 插件使用 ZIP，Linux/macOS 使用保留执行权限的 tar.gz；统一提供
  三平台 MCP ZIP、插件包、许可证与 SHA256SUMS。新增 Hook 回归及
  隔离插件安装验证入口。

## [0.1.0] — 2026-10-23

首个公开版本：为 Claude Code、Codex 等 Agent 提供本地 MCP stdio 文件接口。

### 功能

- 提供 `read_file`、`write_file`、`edit_file`、`glob`、`grep` 五个工具，支持项目
  根目录内的绝对和相对路径。
- 严格验证 UTF-8 / GBK；自动标注编码歧义，拒绝非法序列及不可表示字符。
- 已有文件保留编码；局部编辑只编码替换片段，未修改区域逐字节保留，支持
  CRLF/LF、UTF-8 BOM 和无末尾换行。新文件默认 UTF-8，可显式指定 GBK。
- 通过 BLAKE3 revision 防止覆盖过期内容，支持差异预览、同文件多处精确编辑、
  重叠检查、同目录临时文件与原子提交。
- 基于 ripgrep 组件并行遍历和搜索，支持忽略规则、glob、正则/字面量、上下文、
  文件名与匹配行计数模式，以及查询超时、取消、结果和输出限额。
- 提供 Claude Code / Codex 配置与项目指令示例、安装文档、架构说明、协议
  冒烟脚本及性能测试工具；发布包携带依赖许可证与 SHA-256 校验清单。

### 发布与验证

- 发布 Windows x86_64 MSVC 二进制，并提供 Linux x86_64 GNU、macOS aarch64
  构建配置；实际可下载的平台以 [Release 附件](https://github.com/JAX1024Dev/MCP-GBK/releases/tag/v0.1.0) 为准。
- 已记录 macOS 本地功能、真实 Claude/Codex 使用闭环和百万文件性能实测。
  各平台原生验证状态以 [验证记录](docs/VALIDATION.md) 及 GitHub Actions 为准。

### 已知边界

- 不支持 UTF-16/32、GB18030 四字节、混合编码或二进制文件；不跟随项目内链接。
- 修改上限为 32 MiB；搜索结果受时间、数量和 64 KiB 输出限制。
  `complete=false` 不能用于判定匹配不存在。
- 不提供持久化索引、跨文件事务、远程服务或强制替换客户端内置工具的机制。
- 外部程序不配合锁时仍有最终版本检查与替换之间的竞争窗口；Unix 原子替换
  不保证保留原 owner/group、扩展属性和 ACL。
