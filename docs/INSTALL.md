# 安装与接入

GBK MCP 是本地 stdio MCP 服务，每个进程绑定一个已存在的项目根目录。
发布包包含可执行程序、配置示例、文档及上游许可证；运行时不依赖 Rust、Python、
Node.js 或独立安装的 ripgrep。

## 下载 v0.2.0

Claude Code 用户也可以直接安装自带默认 Hook 的[三平台离线插件](OFFLINE_PLUGIN.md)。
下面介绍适用于 Claude、Codex 等客户端的独立 MCP 包。

在 [v0.2.0 Release](https://github.com/JAX1024Dev/MCP-GBK/releases/tag/v0.2.0)
下载适合本机的 ZIP，同时下载该页面的 `SHA256SUMS`。

| 系统与架构 | ZIP 名称 | Rust 目标 |
| --- | --- | --- |
| Windows 64 位 Intel/AMD | `gbk-mcp-0.2.0-windows-x86_64.zip` | `x86_64-pc-windows-msvc` |
| Linux 64 位 Intel/AMD | `gbk-mcp-0.2.0-linux-x86_64.zip` | `x86_64-unknown-linux-gnu` |
| macOS Apple Silicon | `gbk-mcp-0.2.0-darwin-aarch64.zip` | `aarch64-apple-darwin` |

Linux 包使用 GNU libc，面向构建工作流对应的 Ubuntu 环境；不提供 musl/Alpine
兼容承诺。其他架构可从源码构建。发布附件是否齐全以 Release 页面为准。

## Windows：校验、解压与启动

将 ZIP 与 `SHA256SUMS` 放在同一目录，在该目录打开 PowerShell：

```powershell
$archive = 'gbk-mcp-0.2.0-windows-x86_64.zip'
$entry = Get-Content .\SHA256SUMS | Where-Object { ($_ -split '\s+', 2)[1] -eq $archive }
if (-not $entry) { throw 'SHA256SUMS 中缺少目标 ZIP' }
$expected = ($entry -split '\s+')[0]
$actual = (Get-FileHash -LiteralPath ".\$archive" -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw 'SHA-256 不一致，请重新下载' }
Expand-Archive -LiteralPath ".\$archive" -DestinationPath 'C:\tools\gbk-mcp'
& 'C:\tools\gbk-mcp\gbk-mcp.exe' --version
& 'C:\tools\gbk-mcp\gbk-mcp.exe' --help
```

项目目录必须已经存在。直接启动示例：

```powershell
& 'C:\tools\gbk-mcp\gbk-mcp.exe' --root 'C:\project'
```

程序等待 MCP JSON-RPC 消息属于正常行为，终端没有交互提示。用 `Ctrl+C` 退出；
日常使用应由客户端按下方配置启动服务，不需要手工常驻一个终端。

## Linux / macOS

解压到固定目录。Linux 用 `sha256sum`，macOS 用 `shasum -a 256`，将输出与
`SHA256SUMS` 中对应文件名的摘要比较。例如 macOS：

```sh
shasum -a 256 gbk-mcp-0.2.0-darwin-aarch64.zip
unzip gbk-mcp-0.2.0-darwin-aarch64.zip -d "$HOME/tools/gbk-mcp"
chmod +x "$HOME/tools/gbk-mcp/gbk-mcp"
"$HOME/tools/gbk-mcp/gbk-mcp" --version
```

本程序未作 Apple 签名或公证；若系统阻止运行，可在核验下载来源和摘要后通过
macOS「系统设置 → 隐私与安全性」允许该程序。不要关闭系统整体安全保护。

## Claude Code / Codex

配置中的 `command` 指向解压后的程序绝对路径，`--root` 指向目标项目绝对路径。
Windows 可写 `C:/tools/gbk-mcp/gbk-mcp.exe`，JSON 中也可以双反斜杠转义。

| 客户端 | 配置模板 | 项目指令片段 |
| --- | --- | --- |
| Claude Code | [examples/claude.mcp.json](../examples/claude.mcp.json)，合并至项目 `.mcp.json` | [examples/CLAUDE.md](../examples/CLAUDE.md) |
| Codex | [examples/codex.config.toml](../examples/codex.config.toml)，合并至项目 `.codex/config.toml` 或用户配置 | [examples/AGENTS.md](../examples/AGENTS.md) |

保留现有配置，只合并 `gbk` 服务和适用的项目指令，然后重启客户端会话或重新
连接 MCP 服务。客户端应能列出 `read_file`、`write_file`、`edit_file`、`glob`、
`grep` 五个工具。按客户端策略批准服务和写操作。

可让 Agent 完成一次“查找文件 → 搜索中文 → 读取 → 预览修改 → 修改 → 复读”。
已有文件的修改需要上次读取返回的 `revision`；项目指令会引导 Agent 使用本
服务，但不会禁用客户端内置工具。多项目可配置不同服务名和根目录。

## 从源码构建

需要 Rust 1.88+；Windows 使用带 C++ 工具链的 MSVC 构建环境。

```sh
git clone https://github.com/JAX1024Dev/MCP-GBK.git
cd MCP-GBK
cargo build --release --locked
```

产物为 `target/release/gbk-mcp`，Windows 为 `target\release\gbk-mcp.exe`。
开发验证命令见 [贡献指南](../AGENTS.md)，已有验证范围见 [验证记录](VALIDATION.md)。

## 故障排查

| 现象 | 处理 |
| --- | --- |
| 客户端启动失败或找不到程序 | 用绝对路径；先执行 `--version`；确认 ZIP 架构、执行权限和 `--root` 目录存在。若提示缺失 MSVC 运行库，安装微软官方 Visual C++ x64 Redistributable。 |
| 服务启动后没有输出 | stdio 服务正在等待客户端。不要向 stdout 添加日志；检查客户端的 MCP 错误日志。 |
| Windows 提示拒绝访问或文件被占用 | 检查目录权限、文件只读属性和其他程序的独占句柄；释放后重新读取再提交。 |
| 编码歧义或 ASCII 文件加入中文失败 | 根据项目实际编码显式传 `encoding: "gbk"` 或 `"utf8"`，不要猜测或先转码文件。 |
| `conflict` | 文件已变化，重新读取、重新生成修改并使用新的 revision。 |
| `invalid_encoding` 或不可表示字符 | 检查是否是 UTF-16、GB18030、混合编码或非法字节；GBK 不能表示的字符必须调整输入。 |
| 无搜索结果且 `complete=false` | 扫描尚未完整完成。检查 stop_reason 和文件错误，缩小 path/glob 或提高 timeout_ms。 |
| `outside_root` / 链接相关错误 | 使用根目录内的普通文件；首版不跟随项目内符号链接或 junction。 |

升级前退出相关客户端会话，将新包解压至新目录、校验摘要，再修改 `command`
路径。项目文件与客户端配置独立于发布包，无需随程序覆盖。
