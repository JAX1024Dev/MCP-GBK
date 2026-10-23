# 三平台离线插件安装

适用于 Claude Code 2.1.308+。每个包都是携带原生程序、Skill 和默认 Hook 的完整
本地 marketplace；安装和启动不会下载插件依赖。模型连接按现有网络配置运行。

| 平台 | v0.2.0 插件包 |
| --- | --- |
| Windows x64 | `gbk-mcp-0.2.0-claude-plugin-windows-x86_64.zip` |
| Linux x64（GNU/glibc） | `gbk-mcp-0.2.0-claude-plugin-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `gbk-mcp-0.2.0-claude-plugin-darwin-aarch64.tar.gz` |

希望直接在 Desktop Code 上传 ZIP，可使用 0.2.1 的[GUI 专用包](GUI_PLUGIN.md)。
本页介绍原有 marketplace 包；两种格式不可互相替代上传。

## Windows 校验与安装

取得对应安装包及同版本 `SHA256SUMS`，可以拷入离线环境。以下命令从安装包
所在目录执行；压缩包内已经有顶层 `gbk-plugin/`：

```powershell
$archive = 'gbk-mcp-0.2.0-claude-plugin-windows-x86_64.zip'
$entry = Get-Content .\SHA256SUMS | Where-Object { ($_ -split '\s+', 2)[1] -eq $archive }
if (-not $entry) { throw '校验清单缺少目标包' }
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne ($entry -split '\s+')[0]) { throw 'SHA-256 不一致' }
Expand-Archive -LiteralPath $archive -DestinationPath 'C:\tools'
claude plugin marketplace add "C:/tools/gbk-plugin"
claude plugin install gbk-mcp@jax1024dev-gbk --scope user
```

## macOS 与 Linux 校验及安装

macOS 使用 `shasum -a 256`，Linux 使用 `sha256sum`，将结果与 `SHA256SUMS`
对应文件名所在行比较。一致后解压；tar.gz 会保留程序执行权限，无需 sudo：

```sh
# macOS Apple Silicon
archive=gbk-mcp-0.2.0-claude-plugin-darwin-aarch64.tar.gz
shasum -a 256 "$archive"
# Linux x64 则使用下面两行替代上面两行
# archive=gbk-mcp-0.2.0-claude-plugin-linux-x86_64.tar.gz
# sha256sum "$archive"

mkdir -p "$HOME/.local/share/gbk-mcp"
tar -xzf "$archive" -C "$HOME/.local/share/gbk-mcp"
claude plugin marketplace add "$HOME/.local/share/gbk-mcp/gbk-plugin"
claude plugin install gbk-mcp@jax1024dev-gbk --scope user
```

若 macOS 提示下载程序无法验证，先核实来源和 SHA-256，再按系统安全提示允许
该程序执行。插件不自动移除 quarantine 或绕过系统安全策略。

使用本地目录注册，受限机器不使用 GitHub URL。固定目录需要保留，升级也在
该位置更新；企业策略必须允许插件和 Hook。若旧版已通过 `claude mcp add` 或
settings 配置 GBK 服务/Hook，请按原 scope 移除重复配置；插件不改写这些内容。

## 使用与检查

从目标项目目录启动 Claude。插件以会话项目目录为 root，自动启动 MCP 和
SessionStart、SubagentStart、PreToolUse 三个 Hook；无需复制 CLAUDE.md 或先调用 Skill。

- `/mcp`：确认 `plugin:gbk-mcp:gbk` 可用，包含五个工具。
- `/hooks`：确认三个默认 Hook 已注册。
- `/gbk-mcp:gbk-files`：查看操作说明。

内置 Read/Edit/Write/Grep 对项目文本会收到拒绝原因，应改用对应 MCP；图片、
PDF、Notebook 保留，Glob 仅引导。Hook 不自动批准写入。MCP 不可用时先修复，
不退回内置文本读取；子代理没有 MCP 工具时应将任务交回主代理。

## 升级、卸载与构建

升级前关闭相关 Claude 会话，在临时目录解压并校验新包，替换原固定目录，运行
`claude plugin update gbk-mcp@jax1024dev-gbk`，重启后生效。离线升级不依赖外网。
卸载用 `claude plugin uninstall gbk-mcp@jax1024dev-gbk`，不会修改项目文本。

开发者在对应原生平台构建并打包：

```sh
cargo build --release --locked
python3 scripts/package_plugin.py
# 指定 Cargo target 构建时传入相同 --target；Windows 使用 python
python3 scripts/plugin_smoke.py --archive-dir dist
```

打包器检查平台与架构，带入同版本配置、许可证和依赖声明。源码目录只是模板，
不能直接安装；使用完整平台包。CI 从压缩包解压安装并验证，标签发布流程上传
三平台 MCP 包、插件包和统一 SHA256SUMS。插件没有额外的 Python/Node 运行时依赖。
