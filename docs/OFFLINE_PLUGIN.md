# Windows 离线插件安装

本指南适用于 `gbk-mcp-0.2.0-claude-plugin-windows-x86_64.zip`。
这是带 EXE 的完整本地 marketplace，适用于 Windows x64 和 Claude Code
2.1.308+。安装和启动不会下载依赖；Claude 自身模型连接仍按现有网络策略运行。

## 校验与安装

在可联网环境取得完整 ZIP 和同批构建的 `SHA256SUMS`，拷入受限环境。在 ZIP
所在目录用 PowerShell 校验并解压；ZIP 内已经有顶层 `gbk-plugin/`：

```powershell
$archive = 'gbk-mcp-0.2.0-claude-plugin-windows-x86_64.zip'
$entry = Get-Content .\SHA256SUMS | Where-Object { ($_ -split '\s+', 2)[1] -eq $archive }
if (-not $entry) { throw '校验清单缺少目标 ZIP' }
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne ($entry -split '\s+')[0]) { throw 'SHA-256 不一致' }
Expand-Archive -LiteralPath $archive -DestinationPath 'C:\tools'
claude plugin marketplace add "C:/tools/gbk-plugin"
claude plugin install gbk-mcp@jax1024dev-gbk --scope user
```

使用本地目录注册，不要在受限机器上把 marketplace 换成 GitHub URL。
固定目录需要保留，升级也在该位置更新。企业策略必须允许该插件及 Hook。
若旧版已通过 `claude mcp add` 或 settings 配置过 GBK 服务/Hook，请先按原 scope
移除重复配置；安装脚本不会擅自改写这些内容。

## 使用与检查

从目标项目目录启动 Claude。插件以该会话的项目目录为 root，自动启动 MCP
和三个 Hook；无需手动设置绝对项目路径、复制 CLAUDE.md 或先调用 Skill。

- `/mcp`：确认 `plugin:gbk-mcp:gbk` 可用，包含五个工具。
- `/hooks`：确认 SessionStart、SubagentStart、PreToolUse 已注册。
- `/gbk-mcp:gbk-files`：查看操作说明。

内置 Read/Edit/Write/Grep 对项目文本会收到拒绝原因，应随后调用对应 MCP；
Read 的图片、PDF、Notebook 保留，Glob 仅引导。写入仍按 Claude 原有权限策略
审批；Hook 不会自动批准写入。MCP 不可用时先修复，不退回内置文本读取。

## 升级、卸载与构建

升级前关闭相关 Claude 会话。在临时目录解压新版本并校验，然后替换固定的
`C:\tools\gbk-plugin` 目录，再执行：

```powershell
claude plugin update gbk-mcp@jax1024dev-gbk
```

重启后使用新版本。手动移入新包是离线更新，不依赖自动从外网获取版本。
卸载用 `claude plugin uninstall gbk-mcp@jax1024dev-gbk`，不会修改项目文本。

开发者在 Windows 完成 release 构建后运行：

```powershell
cargo build --release --locked
python scripts/package_plugin.py --binary target/release/gbk-mcp.exe
```

打包器校验 x64 Windows PE 格式，并带入同版本 Hook、Skill、项目及依赖许可证。
源码插件目录只是模板，不包含已编译 EXE；面向用户分发必须使用完整 ZIP。
CI 的 Windows 作业会生成离线 ZIP，标签发布流程会将其一并上传 Release。
