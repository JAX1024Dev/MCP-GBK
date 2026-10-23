# GBK MCP — 三平台离线插件

适用于 Claude Code 2.1.308+，支持 Windows x64、Linux x64 GNU/glibc 和 macOS
Apple Silicon。插件自带原生程序、五个 MCP 工具、Skill 和默认保护 Hook。

提供两种安装格式：

- **GUI ZIP**：文件名包含 `claude-plugin-gui`。在 Desktop 的 Code 模式打开
  Customize → Plugins → Add plugin → Upload plugin，直接选择 ZIP。无需解压、
  填 root 或运行终端命令。详见随包 `docs/GUI_PLUGIN.md`。
- **Marketplace 包**：Windows ZIP、Unix tar.gz，按下面方式注册本地目录。

Windows 解压 ZIP；macOS/Linux 用 `tar -xzf` 解压 tar.gz，以保留程序执行权限。
在固定目录解压完整 marketplace 后执行：

```sh
claude plugin marketplace add "/absolute/path/to/gbk-plugin"
claude plugin install gbk-mcp@jax1024dev-gbk --scope user
```

Windows 路径可使用 `C:/tools/gbk-plugin`。完整安装、校验和升级步骤见随包附带的
`docs/OFFLINE_PLUGIN.md`。安装和运行不下载插件依赖。

从目标项目启动新会话，使用 `/mcp` 和 `/hooks` 检查服务与默认规则。
`/gbk-mcp:gbk-files` 可查看工具使用指引。不需要另行运行 `claude mcp add`。

默认拦截项目内的内置 Read/Edit/Write/Grep，引导使用 MCP；保留图片、PDF 和
Notebook 读取。Glob 仅引导。Shell 用于构建、Git 和程序运行；Hook 不解析所有
Shell 命令，也不自动批准写入。企业策略必须允许插件 Hook。

Hook 失败会阻止文本操作，MCP 不可用时先修复；子代理缺少工具时将任务交回
主代理。不要同时保留重复注册的旧 MCP 服务或 Hook。

升级时退出相关 Claude 会话，校验新包并替换固定目录，再运行
`claude plugin update gbk-mcp@jax1024dev-gbk` 并重启。卸载使用
`claude plugin uninstall gbk-mcp@jax1024dev-gbk`。源码模板需要打包后再分发。
