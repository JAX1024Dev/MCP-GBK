# GBK MCP — Windows 离线插件

适用于 Claude Code 2.1.308+、Windows x64。插件内置本地 EXE、五个 MCP
工具和默认保护 Hook；安装后无需运行 `claude mcp add` 或手动配置 root。

完整离线包的外层是本地 marketplace，在固定目录解压后执行：

```powershell
claude plugin marketplace add "C:/tools/gbk-plugin"
claude plugin install gbk-mcp@jax1024dev-gbk --scope user
```

从目标项目启动新会话，使用 `/mcp` 和 `/hooks` 检查服务及规则。
`/gbk-mcp:gbk-files` 可查看工具使用指引。安装和运行不下载插件依赖，Claude
自身的模型连接仍使用你已有的网络配置。

默认拦截当前项目内的内置 Read/Edit/Write/Grep，提示使用 MCP；Read 的图片、
PDF、Notebook 保留。Glob 仅引导，不拦截。Shell 保留构建、Git 和程序运行
用途，规则不解析或封锁所有 Shell 命令，因此这不是完整的安全沙箱。

Hook 失败时文本操作被阻止，不自动退回内置读取。用 `/hooks` 检查错误、
`/mcp` 检查连接。受限子代理如果没有 MCP 工具，应把文件任务交回主代理。
企业策略必须允许插件 Hook。不要同时保留重复注册的旧 GBK MCP 服务或 Hook。

升级时退出所有相关 Claude 会话，将新包解压至临时目录、验证 SHA-256，
替换固定 marketplace 目录，再执行：

```powershell
claude plugin update gbk-mcp@jax1024dev-gbk
```

重启后生效。卸载使用 `claude plugin uninstall gbk-mcp@jax1024dev-gbk`。
这不会修改项目文件。源码目录是插件模板，须先打包生成带 EXE 和许可证的完整包。
