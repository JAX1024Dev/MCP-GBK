# GBK MCP Hook 与离线插件实施备忘

方案来源：2026-10-23 会话确认。目标版本：0.2.0。Windows x64 优先，插件自带
EXE，通过本地 marketplace 安装，安装和运行不下载依赖。

## 交付与结构

先实现 Rust Hook、独立 MCP 配置示例、测试和文档；Windows 离线插件携带并默认
启用相同 Hook。现有五工具和原有 MCP 接入继续保留。

```text
gbk-plugin/
├── .claude-plugin/marketplace.json
├── README.md
└── plugins/gbk-mcp/
    ├── .claude-plugin/plugin.json
    ├── .mcp.json
    ├── hooks/hooks.json
    ├── bin/gbk-mcp.exe
    ├── skills/gbk-files/SKILL.md
    ├── README.md
    ├── LICENSE
    ├── THIRD_PARTY_NOTICES.md
    └── third-party-licenses/
```

marketplace 名称固定 `jax1024dev-gbk`，插件名固定 `gbk-mcp`。程序以
`${CLAUDE_PLUGIN_ROOT}/bin/gbk-mcp.exe` 启动，MCP 与 Hook 都以
`${CLAUDE_PROJECT_DIR}` 为 root。Hook 只在 `hooks/hooks.json` 声明，避免重复。

## 默认策略

| 内置操作 | root 内策略 |
| --- | --- |
| Read | 要求 MCP read_file；普通文件的 png/jpg/jpeg/gif/webp/pdf/ipynb 放行，扩展名不区分大小写 |
| Edit | 拒绝并提示 MCP edit_file |
| Write | 拒绝并提示 MCP write_file，包括新建 |
| Grep | 拒绝并提示 MCP grep；缺省 path 使用事件 cwd |
| Glob | 仅引导优先 MCP glob |
| Bash / PowerShell | 保留构建、Git、程序运行，指令限制文本读取、检索、改写用途 |

UTF-8 和 GBK 文本统一经过 MCP，不在 Hook 中全量判断编码。明确 root 外的
操作交回 Claude 权限流程；根内链接/junction、外部指向根内的链接和无法确认
的路径拒绝。Hook 不替代模型调用 MCP、不批准写权限，也不是 Shell 沙箱。

## 程序与事件

```text
gbk-mcp --root <项目目录> --hook session-start
gbk-mcp --root <项目目录> --hook subagent-start
gbk-mcp --root <项目目录> --hook pre-tool-use
```

- 同一 Rust EXE，Hook 分支不启动 Tokio/MCP，不联网、不修改文件。
- SessionStart 覆盖 startup/resume/clear/compact/fork，注入 root、工具映射、
  revision、编码歧义与搜索未完成规则。
- SubagentStart 注入相同规则；没有 MCP 工具的子代理须交回主代理。
- PreToolUse 精确匹配 `^(Read|Edit|Write|Grep)$`，不拦截 MCP 自身。
- stdin JSON 最大 64 MiB；放行不返回批准，拒绝输出结构化原因；解析、路径、
  root 异常和超限默认拒绝。
- exec 形式 command + args，支持中文与空格路径；PreToolUse 设置五秒超时和
  `onFailure: "block"`，EXE 缺失、崩溃或超时不会静默放行。

兼容基线为 Claude Code 2.1.308+；管理员须允许插件 Hook，插件不绕过该策略。

## 验证与发布门槛

1. 测试四类拦截、特殊读取放行、root 外路径、边界、缺省搜索路径、异常输入、
   超限，以及不拦截 MCP/Glob/Shell。
2. Windows 测试中文、空格、大小写、长路径、junction、缺失 EXE 和超时阻止。
3. 回归现有五工具、GBK 字节保留、版本冲突和 stdio。
4. 隔离配置安装完整插件，确认默认 Hook 在主会话及子代理生效。
5. 验证“内置 Read 被拒绝 → MCP 正确读取 GBK → 编辑 → 复读”，记录 Hook 耗时。
6. 确认 Hook 不扫描目录、不读完整文件，插件不下载依赖。真实 Claude 使用与
   无凭证自动检查分别记录，不能把静态校验视为完整使用验收。
7. 离线 ZIP 与统一 SHA256SUMS 交付；保持已有三平台普通 MCP 包。升级退出
   Claude 后替换本地 marketplace，再更新插件并重启。

当前实现、命令和边界见 [Hook 指南](HOOKS.md) 与 [离线安装](OFFLINE_PLUGIN.md)；
实际验证状态以 [验证记录](VALIDATION.md) 为准。


## 0.2.0 三平台交付补充

2026-10-23 确认扩展为 Windows x64、Linux x64 GNU/glibc、macOS Apple Silicon。
复用 Rust 实现、插件模板和默认 Hook，打包时生成平台程序路径，不增加 Shell
启动器或下载依赖。Windows 插件使用 ZIP，Unix 使用 tar.gz 保留可执行权限。
三平台 CI 从最终压缩包解压后运行隔离安装及 Hook/MCP 闭环；通过后发布同一个
v0.2.0 Release，包含三个 MCP ZIP、三个插件包及统一 SHA256SUMS。
发布完成后只验证版本及附件存在，不下载已发布附件复测。


## 0.2.1 GUI ZIP 安装补充

面向 Claude Code 和 Desktop 本机 Code 页签，新增三平台独立 GUI ZIP，保留
marketplace 格式。GUI 包根目录放插件清单、MCP、Skill 和默认 Hook，原生程序
放 `server/`，Unix ZIP 显式保存执行权限。Code 的 Upload local plugin 为本机
安装入口，不依赖 API Key 会话不具备的账户插件同步；本机 3.6556.2 已查看确认
该入口。安装／替换后新建会话，继续自动使用项目 root；不增加运行时下载依赖。

打包器使用 `--format plugin-zip`，冒烟脚本使用 `--archive PATH` 验证具体压缩包。
CLI 验证与 Desktop GUI 实测分别记录，不将 Linux CLI 验证写成 Linux GUI 已通过。
准备 0.2.1 包和文档，不自动发布 Release，也不修改 v0.2.0 附件。
