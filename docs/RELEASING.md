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
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

`.github/workflows/release.yml` 校验标签与 Cargo 版本一致，构建以下目标：

| 平台 | Rust target | 包名后缀 |
| --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-x86_64.zip` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `linux-x86_64.zip` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `darwin-aarch64.zip` |

所有构建成功后，工作流汇总 ZIP，重新生成统一 `SHA256SUMS`，从变更记录提取
当前版本说明，再创建 GitHub Release。发布包包含程序、安装和架构说明、
客户端示例以及项目和依赖许可证。源代码归档由 GitHub 自动提供。

## 检查发布状态

```sh
gh run list --workflow release.yml
gh release view v0.1.0 --repo JAX1024Dev/MCP-GBK --json url,tagName,assets
```

确认版本存在、不是 draft，且包含 Windows ZIP 与 `SHA256SUMS`。本次首版按
发布要求只核实版本和附件，不下载发布后的二进制进行功能测试。

失败时查看 `gh run view RUN_ID --log-failed`，修复构建问题并重新触发失败的
工作流。已发布标签不应移动或覆盖；代码有变化应增加补丁版本。工作流通过
GitHub 提供的 `GITHUB_TOKEN` 发布，不在仓库或示例中保存个人访问令牌。
