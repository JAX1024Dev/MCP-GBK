# 验证记录

验证日期：2026-10-23，Asia/Shanghai。

## 0.2.0 Hook 与插件验证

在 macOS / Apple Silicon、Claude Code 2.1.308 上完成：

- 原有 17 项 MCP 集成测试、新增 7 项 Hook 测试、格式、clippy 与真实 stdio
  冒烟通过。Windows 目标交叉 clippy 通过；Hook 不启动 Tokio/MCP 服务。
- 隔离配置下安装本地 marketplace 插件，不使用 `--plugin-dir` 也自动加载服务
  和 Hook；安装、更新、卸载均通过，保留开发者原客户端配置。
- 用仅监听回环地址的模拟模型驱动真实 Claude 执行器，确认主代理及子代理的
  内置 Read 被拒绝，MCP 读改复读成功；EXE 缺失和 Hook 超时均阻止原操作。
- 使用实际已认证模型另做使用闭环：内置 Read 被拒绝后自主选择 MCP 读取、
  修改、复读，最终文件逐字节符合预期 GBK。真实模型验证使用正常客户端额度。
- 20 次 debug 原生 Hook 子进程启动与处理的本机中位耗时约 3.35 ms、最大约
  4.10 ms；含进程启动，不代表 Windows 性能，也不是百万文件查询基准。

脱敏摘要见 [plugin-tests.json](plugin-tests.json)。脚本为
`scripts/plugin_smoke.py --binary target/debug/gbk-mcp`；默认不访问外部模型，
`--live` 才额外验证真实模型选择行为。原始日志保留在被忽略的
`benchmark-results/plugin-live/`，不随源码提交。

最终代码提交 `533d210` 的 [三平台 CI](https://github.com/JAX1024Dev/MCP-GBK/actions/runs/39971867188)
全部通过，覆盖格式、clippy、MCP/Hook 单元测试、真实 stdio、插件隔离安装与
自动加载、主/子代理拦截、失败阻止、release 构建和打包：

| 平台 | MCP / Hook 测试 | 原生 Hook 子进程中位耗时 |
| --- | --- | --- |
| Windows x64 / MSVC | 18 / 7 项通过 | 8.82 ms |
| Linux x64 / GNU | 17 / 7 项通过 | 1.50 ms |
| macOS | 17 / 7 项通过 | 3.23 ms |

Windows 另外验证中文空格、大小写、长路径和 junction。大小写兼容还核实目录
身份，无法核实时拒绝，避免把不同目录扩展为 root。Windows CI 生成了完整
x64 PE 离线插件 ZIP 及 SHA256SUMS，插件只在一处声明默认 Hook。
远端脱敏摘要见 [plugin-ci-tests.json](plugin-ci-tests.json)。上述计时含 debug
进程启动，采样 20 次；CI 的模型是本地模拟服务，真实模型选择行为单独由前述
本机 `--live` 验证，不将二者混同。

以下历史性能和客户端记录来自 0.1.0，不能直接视为 0.2.0 性能复测。

## 功能与协议

在 macOS / Apple Silicon、Rust 1.99.1 上完成：

- `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`。
- 17 项 Rust 集成测试、release 构建。
- 真实 stdio MCP 冒烟：2025-11-25 协议握手、五工具发现、查找、中文搜索、
  读取、预览、GBK 修改、版本冲突、UTF-8 新建、编码歧义与取消。
- GBK 非规范映射字节保留、CRLF/LF、无末尾换行、UTF-8 BOM、显式 GBK 覆盖
  BOM 外观字节、原版本多处修改、重叠修改拒绝、GBK 不可表示字符拒绝。
- 编码字符跨读取块、非法尾字节、GB18030 四字节拒绝、百万级目录过滤。
- 只读文件、路径越界、符号链接、多个硬链接、长路径、输出限额和超时。
- CRLF 文件的 `^中文匹配$` 整行匹配，以及长行中较靠后的命中片段。

测试入口：`tests/core.rs`、`scripts/smoke_test.py`。

## Claude Code / Codex

实际客户端在临时项目中完成了全部五类文件操作，内置文件工具保持可用。
通过项目指令优先调用本 MCP，成功完成查找、检索、读取、修改、复读和新建。
修改后逐字节验证原文件仍为 GBK，新文件为 UTF-8。

| 客户端 | 版本 | 结果 |
| --- | --- | --- |
| Claude Code | 2.1.308 | 五工具路由、原编码修改、UTF-8 新建通过 |
| Codex CLI | 0.163.0-alpha.11 | 五工具路由、原编码修改、UTF-8 新建通过 |

摘要与实际调用序列见 [agent-tests.json](agent-tests.json)。测试脚本为
`scripts/agent_smoke.py`；需要已安装、已登录的客户端，使用正常模型额度。
Codex 测试仅为临时工作目录中的两个 MCP 写工具预先授权，未关闭客户端沙箱。

## 百万文件性能

使用真实生成的 1,000,000 个文件，每文件 10,240 字节，总内容
10,240,000,000 字节（约 9.54 GiB）；GBK 和 UTF-8 各 500,000 个。
全局线程预算 8，同一请求分配 4 个搜索工作线程，最多两个请求并行。

测量通过真实 MCP stdio 调用完成；内存为 MCP 服务进程的采样 RSS，不含系统
文件缓存。未清理操作系统缓存，报告为重复运行、缓存状态未严格控制的结果，
不标记为冷缓存。硬件与发布二进制 SHA-256 见性能 JSON。

| 用例 | 实测 |
| --- | --- |
| 返回前 100 条命中 | 0.184 秒 |
| 自动检测完整扫描 100 万文件 | 36.628 秒 |
| 显式 GBK 完整扫描 50 万文件 | 17.058 秒 |
| 显式 UTF-8 完整扫描 50 万文件 | 15.736 秒 |
| 两个请求并发扫描全部文件 | 18.127 秒 |
| 服务进程峰值 RSS | 13.94 MiB |
| GBK 扫描 / ripgrep 耗时 | 1.11 倍 |
| UTF-8 扫描 / ripgrep 耗时 | 1.05 倍 |

原始指标、结果完整性和二进制摘要见 [macos-million.json](benchmarks/macos-million.json)。


复现：

```sh
cargo build --release --locked
python3 scripts/benchmark.py --generate --files 1000000 --root benchmark-data/scale-1000000 --output benchmark-results/million.json
```

结果达到数量限制时 `complete=false`，属于预期的有界查询行为；完整扫描用例
必须 `complete=true` 且 visited_files 与数据规模一致，才能计入全量性能结果。

## 平台验收边界

Windows 专用代码与 Windows 条件测试已在本机通过交叉 clippy / 类型检查：

```sh
cargo clippy --locked --all-targets --target x86_64-pc-windows-gnu --features blake3/pure -- -D warnings
```

这里的 blake3/pure 仅用于避免在 macOS 交叉检查时依赖 Windows 汇编工具链；
原生 release 构建沿用默认优化实现。

## 首版发布前的原生 CI

2026-10-23，在 [GitHub Actions CI](https://github.com/JAX1024Dev/MCP-GBK/actions/runs/39818614236)
对提交 `ccc9931` 执行了三平台原生检查。格式、clippy、Rust 集成测试和真实
stdio MCP 冒烟均通过：

| 平台 | Rust 集成测试 | stdio 冒烟 |
| --- | --- | --- |
| Windows x64 / MSVC | 18 项通过 | 通过 |
| Linux x64 / GNU | 17 项通过 | 通过 |
| macOS | 17 项通过 | 通过 |

首次 Windows CI 暴露了替换源文件仍持有写句柄导致的共享冲突，已在调用
`ReplaceFileW` 前关闭该句柄。回归覆盖普通 GBK 编辑、完整覆盖、外部文件占用
时保留原内容、临时文件清理，以及释放占用后的重试成功。

CI 同时配置 release 构建与 ZIP 打包，完整作业状态以链接为准。版本标签触发
的 [Release 工作流](https://github.com/JAX1024Dev/MCP-GBK/actions/workflows/release.yml)
负责构建和发布 Windows x64、Linux x64 与 macOS Apple Silicon 的附件。

上述结果是托管 runner 的功能检查，尚未在 Windows 11 / 8 核 / 16 GiB /
本地 NVMe 的参考硬件上完成百万文件性能测量，也没有单独覆盖自定义 NTFS ACL
策略。macOS 性能指标不能替代 Windows 性能验收。本次发布后只确认版本和附件
存在，不下载 Release 二进制重复测试。
