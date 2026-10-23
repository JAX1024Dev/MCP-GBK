# GBK MCP

供 Claude Code、Codex 等 Agent 使用的本地文件工具。Rust 单一可执行程序，
通过 MCP stdio 暴露 **read_file / write_file / edit_file / glob / grep** 五个工具。

已有 GBK 文件原编码保存，未修改区域保留原始字节。UTF-8 文件保持 UTF-8，
新文件默认 UTF-8。解码仅发生在内存中，不生成 UTF-8 副本。

## 下载与安装

开发中的 **0.2.1** 新增三平台 GUI ZIP：在 Desktop **Code → Customize → Plugins →
Add plugin → Upload plugin** 直接上传，无需终端安装命令；使用本机安装入口，
不依靠账户插件同步。[GUI 安装指南](docs/GUI_PLUGIN.md) 说明包名、替换和系统提示。
0.2.1 尚未发布 Release，可从 [CI 构建产物](https://github.com/JAX1024Dev/MCP-GBK/actions/workflows/ci.yml)
取得 `claude-plugin-gui-*.zip`；原 marketplace 格式保留。

当前版本为 **v0.2.0**。从 [GitHub Releases](https://github.com/JAX1024Dev/MCP-GBK/releases/tag/v0.2.0)
下载对应平台的包和 `SHA256SUMS`。独立 MCP 包适合 Claude、Codex 等客户端；
Claude 插件包含同版本程序、Skill、本地 marketplace 和默认 Hook。

| 平台 | 独立 MCP 包 | Claude 离线插件包 |
| --- | --- | --- |
| Windows x64（MSVC） | `gbk-mcp-0.2.0-windows-x86_64.zip` | `gbk-mcp-0.2.0-claude-plugin-windows-x86_64.zip` |
| Linux x64（GNU/glibc） | `gbk-mcp-0.2.0-linux-x86_64.zip` | `gbk-mcp-0.2.0-claude-plugin-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `gbk-mcp-0.2.0-darwin-aarch64.zip` | `gbk-mcp-0.2.0-claude-plugin-darwin-aarch64.tar.gz` |

插件安装见 [离线插件指南](docs/OFFLINE_PLUGIN.md)。Unix 插件使用 tar.gz 保留执行权限。

Windows 启动示例：

```powershell
& 'C:\tools\gbk-mcp\gbk-mcp.exe' --root 'C:\project'
```

详细的校验、解压、客户端配置和故障排查见 [安装指南](docs/INSTALL.md)。
运行时无需安装 ripgrep、Python、Node.js 或其他服务。

## 从源码构建

需要 Rust 1.88 或更新版本。

```sh
cargo build --release --locked
./target/release/gbk-mcp --root /absolute/path/to/project
```

Windows 使用 `target\release\gbk-mcp.exe`。直接启动后会等待 MCP 客户端消息；
这不是交互式命令行。`--help` 查看参数，`--threads 1..8` 设置全局搜索线程预算。

## 文档导航

- [Desktop Code GUI 安装](docs/GUI_PLUGIN.md)：直接上传三平台单插件 ZIP。
- [Claude Hook 保护](docs/HOOKS.md)：0.2.0 的默认文本工具拦截和独立 MCP 配置。
- [三平台离线插件](docs/OFFLINE_PLUGIN.md)：自带原生程序、默认 Hook、本地 marketplace 安装。
- [插件实施备忘](docs/CLAUDE_PLUGIN_PLAN.md)：已确认方案、验收要求和后续维护依据。
- [安装与故障排查](docs/INSTALL.md)：下载校验、启动与 Agent 接入。
- [架构与实现边界](docs/ARCHITECTURE.md)：编码、原字节编辑、提交与检索设计。
- [验证记录](docs/VALIDATION.md)：实际功能、客户端和性能验证范围。
- [变更记录](CHANGELOG.md)：版本功能与已知限制。
- [贡献指南](AGENTS.md)：项目结构、开发命令与提交要求。
- [维护与发布](docs/RELEASING.md)：版本标签、三平台打包和 Release 流程。

## 接入 Agent

Claude Code：将 `examples/claude.mcp.json` 内容合并到项目 `.mcp.json`，替换
程序和项目的绝对路径，再将 `examples/CLAUDE.md` 指令片段合并到项目指令。

Codex：将 `examples/codex.config.toml` 合并到项目 `.codex/config.toml` 或用户
配置，将 `examples/AGENTS.md` 片段合并到目标项目的 `AGENTS.md`。

Windows JSON 路径可以写成 `C:/tools/gbk-mcp.exe` 和 `C:/project`；TOML 使用
单引号字符串也能避免反斜杠转义。每个服务进程绑定一个工作根目录，多项目可
配置多个服务实例。现有客户端配置和项目指令需要合并，不要整体覆盖。

MCP 初始化 instructions 与项目指令用于引导 Agent 优先选择本服务，不会自动
禁用客户端内置工具。Shell 仍用于构建和测试。配置格式参考
[Claude MCP](https://code.claude.com/docs/en/mcp) 和
[Codex MCP](https://learn.chatgpt.com/docs/extend/mcp)。

## 工具

| 工具 | 主要参数 | 行为 |
| --- | --- | --- |
| `read_file` | `path, offset?, limit?, encoding?` | 按行读取，返回 content、编码和 revision；默认 200 行 |
| `write_file` | `path, content, expected_revision?, encoding?, dry_run?` | 新建或完整覆盖；已有文件必须提供版本 |
| `edit_file` | `path, edits, expected_revision, encoding?, dry_run?` | edits 包含 old_string/new_string/replace_all；精确匹配 |
| `glob` | `pattern, path?, entry_type?, exclude?, max_results?` | 路径匹配与目录列举，entry_type 为 file/directory/any |
| `grep` | `pattern, path?, glob?, encoding?, output_mode?, context?` | 内容/文件名/匹配行计数；支持 fixed_strings、ignore_case |

所有路径支持根目录内的绝对或相对路径。完整参数通过 MCP `tools/list` 的
JSON Schema 查询。未知参数会报错。

典型调用：

```json
{"name":"grep","arguments":{"pattern":"中文","glob":"*.c","output_mode":"files"}}
{"name":"read_file","arguments":{"path":"src/legacy.c","offset":1,"limit":200}}
{"name":"edit_file","arguments":{"path":"src/legacy.c","expected_revision":"从读取结果复制","encoding":"gbk","edits":[{"old_string":"原文","new_string":"修改"}],"dry_run":true}}
```

确认预览后使用相同参数并将 dry_run 设为 false。预览结果的 revision 是预期
写入结果的摘要，不代表已经写入；实际提交仍使用读取时的 expected_revision。

编辑默认要求 old_string 唯一；replace_all=true 替换所有非重叠匹配。一次调用
的修改都以原文件为基准，重叠修改失败。old_string 必须包含原有的真实换行，
CRLF 为 `\r\n`，不会自动规范化。输入的 new_string/content 也按原样保留换行。
完整覆盖会重新编码全部输入文本；局部修改保留未修改区域的原字节。
write_file 新建时会创建缺失的父目录；dry_run 不创建目录或文件。实际提交失败
时可能留下已创建的空父目录。

## 编码规则

- encoding 支持 auto（默认）、utf8/utf-8、gbk/cp936。
- 显式 encoding 优先；自动模式完整验证 UTF-8 与 GBK，UTF-8 BOM 优先；两者都合法时默认 GBK 并
  标记 ambiguous。纯 ASCII 标记 ascii，读取结果用 UTF-8 展示。
- ambiguous 文件修改前必须明确指定 encoding；ASCII 文件加入非 ASCII 内容
  也需要明确指定。指定编码表示如何解释原文件，已有文件仍按该编码保存。
- GBK 使用 encoding_rs 的映射，支持 0x80 欧元符号，额外进行单/双字节结构
  校验。UTF-16、UTF-32、GB18030 四字节、非法字节、NUL 和单文件混合编码不支持。
- 无法用 GBK 表示的字符（如多数 emoji）直接报错，不替换为问号或 HTML 实体。
- 自动检测无法证明双合法文件的作者意图；必要时按文件指定 encoding。

## 性能与资源限制

目录遍历、忽略规则、正则、上下文由 ripgrep 的 ignore/grep 模块承担。
搜索并行、结果有界，不预先收集整个项目，也不建立持久化索引。

- 最多两个活动搜索，共享最多 min(CPU 逻辑核数, 8) 个工作线程。
- 查询默认 10 秒，timeout_ms 范围为 1..300000；队列等待计入预算。
- 默认最多 100 条结果，可设为 1..10000；上下文记录也占结果数量。
- 单次工具正文最多 64 KiB；长匹配行返回首个命中附近最多 4096 字节的片段并
  标记截断。text_start_utf8_byte 表示片段在解码文本行中的起点，不是原文件字节位置。
- `complete=false` 可能由超时、结果上限、输出上限或文件错误造成，不能据此
  判定无匹配。没有后台会话或续查游标，请缩小范围或提高时间预算。
- 搜索采用逐行语义，正则锚点兼容 LF/CRLF；count 统计匹配行，不是每行匹配次数。正则不支持 PCRE2
  的回溯引用和 look-around，非法表达式返回错误。
- 显式编码可直接流式搜索；自动检测要先完整验证文件，再进行匹配，因此存在
  额外读取开销。MCP 取消及结果上限会传递到正在读取的工作线程。
- 编辑和完整写入的输入/输出上限为 32 MiB，读取与搜索支持更大文件的流式处理。
  读取的单行上限为 1 MiB，检索缓冲约 1 MiB；超限明确报错或标记未完成。
- read_file 的 next_offset 从 1 开始；若 line_truncated=true，该行正文不完整，
  应用 grep 定位相关片段。未读取部分不会被暗示为完整视图。

默认遵守 .gitignore/.ignore 并跳过隐藏文件。include_hidden=true 可包括隐藏
文件，respect_ignore=false 可关闭忽略规则；.git/.hg/.svn 元数据和本服务临时
文件始终跳过。glob 的 `*` 不跨路径分隔符，递归用 `**/*`；grep 的 `*.c`
文件过滤默认递归。exclude 按搜索目录相对路径匹配。

## 提交与边界

原始字节的 BLAKE3 摘要用于乐观并发检查。同服务的写操作串行；提交前重新
校验版本，再使用同目录临时文件进行原子替换。新建采用禁止覆盖提交。
临时文件也使用目标编码。只读文件、多个硬链接及符号链接/junction 拒绝修改。
Windows 替换使用 ReplaceFileW，以保留文件安全属性；Unix 保留权限位并刷新目录。
原子替换会改变文件身份，Unix 不保证保留原 owner/group、扩展属性或 ACL。

版本复核无法提供与不配合锁的外部编辑器之间的跨进程事务；最后核对到替换之间
存在竞争窗口。工作根目录应为受信任的本地目录，不应由对抗性进程并发重命名
目录或替换目录链接。网络盘、云占位文件和特殊文件不属于首版性能验收范围。

## 验证

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
python3 scripts/smoke_test.py --binary target/debug/gbk-mcp
cargo build --release --locked
python3 scripts/benchmark.py --generate --files 10000
```

百万文件 / 约 9.54 GiB 真实内容测试：

```sh
python3 scripts/benchmark.py --generate --files 1000000 --root benchmark-data/scale-1000000 --output benchmark-results/million.json
```

生成器会保留数据便于复测，需要约 10 GiB 内容空间及额外文件系统开销，完成后
可删除 benchmark-data。报告包含自动检测、显式编码、双查询、RSS（系统允许时）
和可用的 ripgrep 对照；不自动清除操作系统缓存，不把结果标成冷缓存。
Windows 的 RSS 需另用系统性能工具采集。

可选的真实客户端验证（需要已安装并登录 Claude Code/Codex，消耗正常模型额度）：

```sh
python3 scripts/agent_smoke.py --client both
```

脚本在临时目录运行固定文件任务，保持内置工具可用，验证项目指令能将五类操作
路由至 MCP；Codex 测试仅对临时项目服务的 edit_file/write_file 预先授权。
正常交互接入保留客户端审批策略，遇到写工具审批应在客户端完成授权。需要无人值守
运行时，可按自己的权限策略为这两个工具设置 approval_mode；不要将它们标成只读。

CI 在 Windows、Linux、macOS 执行格式检查、clippy、Rust 测试、真实 stdio
冒烟测试和 release 构建。平台运行与性能数据的实际状态见 docs/VALIDATION.md。

## 开源组件

使用 rmcp、Tokio、ignore、globset、grep-regex、grep-searcher、encoding_rs、
similar、tempfile、blake3 等组件。依赖由 Cargo.lock 锁定，许可证汇总及随包
携带的上游许可证见 THIRD_PARTY_NOTICES.md 和 third-party-licenses/。
