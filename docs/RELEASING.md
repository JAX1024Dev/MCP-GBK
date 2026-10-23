# 维护与发布

发布仓库为 [JAX1024Dev/MCP-GBK](https://github.com/JAX1024Dev/MCP-GBK)。
首版为 `v0.1.0`；正式版本使用 `vMAJOR.MINOR.PATCH` 标签。

## 发布前

1. 更新 `Cargo.toml` 版本及 `Cargo.lock` 中本项目的版本，补充 `CHANGELOG.md`
   对应的 `## [版本号]` 章节；工具参数变化时同步 README、示例和说明。
2. 按 [AGENTS.md](../AGENTS.md) 执行格式、lint、Rust 测试和 stdio 冒烟。
   等待主分支三平台 CI 完成。真实 Agent 和百万文件测试不是每次发布的必跑项；
   修改相关实现时再运行，并在 [VALIDATION.md](VALIDATION.md) 如实记录。
3. 依赖变化时更新锁文件，再运行 `python3 scripts/collect_licenses.py`，提交
   `THIRD_PARTY_NOTICES.md` 和对应许可证文本。

## 创建版本

确认代码和文档已提交、远端分支已同步，然后执行：

```sh
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

`.github/workflows/release.yml` 校验标签与 Cargo 版本一致，构建以下目标：

| 平台 | Rust target | 包名后缀 |
| --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-x86_64.zip` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `linux-x86_64.zip` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `darwin-aarch64.zip` |

所有构建成功后，工作流汇总 ZIP 和 tar.gz，重新生成统一 `SHA256SUMS`，从变更记录提取
当前版本说明，再创建 GitHub Release。发布包包含程序、安装和架构说明、
客户端示例以及项目和依赖许可证。源代码归档由 GitHub 自动提供。

从 0.2.0 起，每个平台同时生成离线 Claude 插件。Windows 为
`gbk-mcp-0.2.0-claude-plugin-windows-x86_64.zip`；Linux 和 macOS 分别为
`gbk-mcp-0.2.0-claude-plugin-linux-x86_64.tar.gz` 和
`gbk-mcp-0.2.0-claude-plugin-darwin-aarch64.tar.gz`，保留程序执行权限。
打包命令为 `python scripts/package_plugin.py`，指定 target 构建时传入相同
`--target`。插件与 Cargo 版本必须一致，打包器检查 PE/ELF/Mach-O 及架构。

发布前 CI 使用 `python scripts/plugin_smoke.py --archive-dir dist`，从最终插件
压缩包解压后测试隔离安装、主/子代理 Hook 和 MCP 闭环、程序缺失和超时拒绝。
CI 安装固定版本 Claude Code 是开发验证依赖，不是插件运行时下载依赖。

## 0.2.1 GUI ZIP

0.2.1 仍处于开发状态，本次适配不创建标签或 Release。后续标签工作流每个平台
额外执行 `python scripts/package_plugin.py --format plugin-zip`，生成
`gbk-mcp-0.2.1-claude-plugin-gui-<平台>.zip`。统一检查九个包（三个 MCP、三个
marketplace、三个 GUI ZIP）和 SHA256SUMS，全部齐全才允许发布；已有 v0.2.0
附件保持不变。

CI 运行 `python scripts/test_package_plugin.py` 验证布局及权限，再分别指定
`python scripts/plugin_smoke.py --archive PATH` 验证两个真实压缩包；`--archive-dir`
仅用于目录内恰好一个插件包时。Desktop GUI 验证状态另见 VALIDATION.md。

## 检查发布状态

```sh
gh run list --workflow release.yml
gh release view v0.2.0 --repo JAX1024Dev/MCP-GBK --json url,tagName,assets
```

确认 v0.2.0 存在、不是 draft，包含三个 MCP ZIP、三个插件包与 `SHA256SUMS`。
按发布要求只核实版本和附件，不下载已发布附件进行功能测试。

失败时查看 `gh run view RUN_ID --log-failed`，修复构建问题并重新触发失败的
工作流。已发布标签不应移动或覆盖；代码有变化应增加补丁版本。工作流通过
GitHub 提供的 `GITHUB_TOKEN` 发布，不在仓库或示例中保存个人访问令牌。
