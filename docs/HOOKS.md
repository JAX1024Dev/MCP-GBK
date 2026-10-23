# Claude Code Hook 保护

适用于 GBK MCP 0.2.0+、Claude Code 2.1.308+。Hook 在本地按确定规则执行，
只检查事件、路径和文件类型，不读取或检测文件正文，不联网、不修改文件。
MCP 的五个工具与原有 `--root` 启动方式保持兼容。

## 默认规则

| Claude 工具 | 项目 root 内行为 |
| --- | --- |
| Read | 要求使用 MCP read_file；普通文件的 png/jpg/jpeg/gif/webp/pdf/ipynb 扩展名放行，比较不区分大小写 |
| Edit / Write | 要求使用 MCP edit_file / write_file，包括新建文件 |
| Grep | 要求使用 MCP grep，省略 path 时按事件 cwd 判断范围 |
| Glob | 引导优先使用 MCP glob，不拦截 |
| Bash / PowerShell / NotebookEdit | 不拦截 |

文本无论 GBK 还是 UTF-8 都走 MCP。Read 的特殊格式例外按扩展名识别，不承诺
验证文件真实格式。未知格式默认要求 MCP；二进制文件应使用对应格式工具。
明确在 root 外的路径继续使用 Claude 的原权限流程，不返回自动批准。
根内链接/junction、从外部指向根内的链接、无法确认的路径均拒绝。

Hook 拒绝不会执行原操作，也不会自动调用 MCP。拒绝原因告诉 Claude 下一步
使用哪个 MCP 工具。MCP 不可用时先修复连接；受限子代理没有相应工具时应交回
主代理，不使用 Shell 绕过。Shell 规则靠指令约束，因此这不是完整的文件访问
沙箱；root 外操作、其他程序和管理员策略仍由客户端及系统管理。

## 普通 MCP 用户

1. 保持现有 MCP 配置，安装 0.2.0+ 程序。
2. 将 `examples/claude.hooks.settings.json` 中 `hooks` 合并到项目
   `.claude/settings.json` 或 `.claude/settings.local.json`，保留其他配置。
3. 替换所有 `command` 路径与 `--root` 参数；root 必须与该项目 MCP 的 root
   完全一致。示例使用固定绝对路径，避免 shell 在注册时展开动态变量。
4. 重启 Claude，用 `/hooks` 查看 SessionStart、SubagentStart、PreToolUse，
   再通过 `/mcp` 确认服务连接。按原权限策略批准 MCP 写入。

macOS/Linux 也可使用原生二进制处理 Hook，把 command 改为其绝对路径即可。
普通项目配置与插件配置只选一套，避免重复注册 Hook 和 MCP。

## 插件用户

Windows 离线插件自带 `hooks/hooks.json`，启用插件后自动注册，无需合并项目
settings 或调用 Skill。MCP 与 Hook 都使用 `${CLAUDE_PROJECT_DIR}`；程序使用
`${CLAUDE_PLUGIN_ROOT}/bin/gbk-mcp.exe`。安装参见 [离线插件](OFFLINE_PLUGIN.md)。
企业环境需要管理员允许插件及其 Hook；插件不能绕过该策略。

## CLI 与故障处理

```text
gbk-mcp --root <项目目录> --hook session-start
gbk-mcp --root <项目目录> --hook subagent-start
gbk-mcp --root <项目目录> --hook pre-tool-use
```

每次从 stdin 读取一个 JSON 事件，最大 64 MiB。启动和子代理事件向 stdout
输出 `hookSpecificOutput.additionalContext`。PreToolUse 精确匹配
`^(Read|Edit|Write|Grep)$`；放行时 stdout 为空，拒绝时输出 `permissionDecision:
"deny"`。格式错误、输入超限、路径或 root 错误返回拒绝。

配置使用 exec 形式（command + args），没有 PowerShell、Git Bash 或额外解释器
依赖。PreToolUse 设置 `timeout: 5`、`onFailure: "block"`，保证 EXE 缺失、崩溃
或未在五秒内读完 stdin 时不会静默放行。同步 stdin 的硬超时由 Claude 执行器
负责；直接手工调用时需自行结束输入。诊断不混入 MCP stdout。

排查时检查 `/hooks` 错误、文件是否被系统阻止执行、root 是否存在，以及 `/mcp`
是否可用。root 在每次服务启动后固定；切换到 root 外的 worktree 时应在该目录
重启会话，或另配服务实例。不要仅改变 Hook 的 root 而保留旧 MCP root。

机制参考 [Claude Hook](https://code.claude.com/docs/en/hooks)、
[插件 Hook](https://code.claude.com/docs/en/plugins/components#hooks)。

## 开发验证

```sh
cargo test --locked
python3 scripts/plugin_smoke.py --binary target/debug/gbk-mcp
# 可选：实际模型验证，需要正常登录，使用客户端额度
python3 scripts/plugin_smoke.py --binary target/debug/gbk-mcp --live
```

Windows 的 binary 路径使用 `.exe`。默认脚本在隔离配置中安装插件，不传
`--plugin-dir` 也验证自动加载；仅用回环地址上的模拟模型驱动真实 Claude
执行器，覆盖主/子代理、EXE 缺失、超时和 GBK 读改。它不证明真实模型会自主
选择工具，所以 `--live` 使用现有认证另外验证选择行为。模拟测试不会调用
外部模型，也不需要登录；运行前须已安装 Claude Code。
