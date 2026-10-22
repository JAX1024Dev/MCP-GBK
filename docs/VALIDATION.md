# 本地验证记录

验证日期：2026-10-23，Asia/Shanghai。

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

**本次未在 Windows 或 Linux 原生系统执行测试。** `.github/workflows/ci.yml`
已配置 Windows、Linux、macOS 的格式、clippy、Rust 测试、stdio 冒烟、release
构建和打包。该工作流尚未在远程 CI 运行，Windows 文件占用、NTFS 权限和参考
Windows 硬件上的性能仍待原生验收；macOS 指标不能替代 Windows 验收结果。
