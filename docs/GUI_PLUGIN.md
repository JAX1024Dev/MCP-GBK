# Desktop Code：通过 GUI 上传安装插件

0.2.1 新增单插件 GUI ZIP，供 Claude Code 和 Claude Desktop 的**本机 Code 会话**
使用。下载 ZIP 后直接上传，无需解压、执行终端安装命令、注册 marketplace 或填写
项目 root。它与已有 marketplace 安装包包含相同的五个 MCP 工具、Skill 和三个 Hook。

从 [v0.2.1 Release](https://github.com/JAX1024Dev/MCP-GBK/releases/tag/v0.2.1)
下载对应 GUI ZIP 和同版本 SHA256SUMS。

## 选择安装包

| 运行 Claude Code 的系统 | GUI ZIP |
| --- | --- |
| Windows x64 | `gbk-mcp-0.2.1-claude-plugin-gui-windows-x86_64.zip` |
| macOS Apple Silicon | `gbk-mcp-0.2.1-claude-plugin-gui-darwin-aarch64.zip` |
| Linux x64（GNU/glibc） | `gbk-mcp-0.2.1-claude-plugin-gui-linux-x86_64.zip` |

选择本机对应的包，文件名必须包含 `claude-plugin-gui`。其他 ZIP/tar.gz 是独立 MCP
或 marketplace 包，不能直接用作单插件上传。Linux 包适用于 Linux Claude Code；
只有客户端提供本机插件上传入口时才能走该 GUI 流程，CLI 验证不代表 Linux Desktop
GUI 已验证。本次不支持 SSH、WSL、云端或跨系统同步后的运行。

## 上传与启用

1. 打开 Claude Desktop，切换至 **Code** 模式。
2. 打开侧栏 **Customize → Plugins**，选择 **Add plugin → Upload plugin**。
3. 在 **Upload local plugin** 页面选择对应 ZIP。检查插件名称 `gbk-mcp`、作者
   `JAX1024Dev` 和项目主页后，点击 **Upload**，完成客户端要求的确认。
4. 已有同名插件时，客户端可能提示替换；确认后使用新版本。只保留一个已启用的
   `gbk-mcp`，避免旧 marketplace 服务、手工 MCP 配置与新插件重复加载。
5. 回到 Code，选择项目目录并新建**本机会话**。MCP 和 Hook 使用该会话项目目录
   作为 root，无需为每个项目重新安装。

此入口是 Desktop 的本机安装流程，不依靠 claude.ai 账户插件同步，适用于使用
API Key／第三方服务的 Code 会话。已在 macOS Desktop 3.6556.2 完成真实上传、新装、替换、启停、卸载和 GBK
读改复读验证；Windows/Linux 当前为原生 CLI/打包验证。其他客户端版本应以
是否提供 **Upload local plugin** 为准。

安装后在 Plugins 中查看版本、工具和启用状态；会话中的 `/mcp`、`/hooks` 可进一步
检查 MCP 连接及三个默认 Hook。内置文本 Read/Edit/Write/Grep 被引导至 MCP，图片、
PDF 和 Notebook Read 保留。缺失 MCP 工具的子代理应将任务交回主代理。

## 升级、停用和卸载

- 升级：同一入口上传新版本 GUI ZIP，确认同名替换，然后新建会话。
- 停用：在 Code 的 Plugins 管理界面关闭插件。
- 卸载：在插件操作菜单选择卸载／移除。本操作不会删除项目文件。
- 安装到本机的插件供同一用户的本机 Claude Code 使用；自定义配置目录或被企业策略
  隔离的客户端需要单独验证，不承诺自动同步。

## 常见问题

| 现象 | 处理 |
| --- | --- |
| 提示找不到插件清单 | 使用 `claude-plugin-gui-*.zip`，不要上传 marketplace 包、源码 ZIP 或 tar.gz。 |
| macOS 提示 Apple 无法验证程序 | 确认包来自本项目并核对 SHA256SUMS，再到系统设置 → 隐私与安全性，对 `gbk-mcp` 选择“仍要打开”，随后新建会话。未做开发者签名与公证，不自动修改隔离属性。 |
| 插件已安装但当前会话没有生效 | 新建本机 Code 会话，检查是否启用及是否有同名旧插件；查看客户端错误。 |
| Hook 阻止操作但 MCP 未连接 | 修复服务连接和权限，检查平台包是否匹配；不要退回内置工具改写文本。 |
| 没有 Upload local plugin 页面 | 本机客户端版本或企业策略未提供入口；查看客户端更新／管理员策略。原 marketplace 安装方式仍保留。 |

## 开发者构建

```sh
cargo build --release --locked
python3 scripts/package_plugin.py --format plugin-zip --output-dir dist/gui
python3 scripts/plugin_smoke.py --archive dist/gui/gbk-mcp-0.2.1-claude-plugin-gui-darwin-aarch64.zip
```

Windows 使用 `python`；指定 Cargo target 构建时，打包传入相同 `--target`。
GUI ZIP 根目录直接包含清单、`.mcp.json`、Hook、Skill、许可证和 `server/` 原生程序；
Unix 程序在 ZIP 内显式保存 0755 权限，不依赖安装后手工 chmod。

冒烟测试仅为 CLI 验证临时添加一个测试 catalog，不改变被测 ZIP 内的配置。GUI 上传、
替换和启停行为需另外使用真实 Desktop 验证，结果记录在 [验证说明](VALIDATION.md)。
