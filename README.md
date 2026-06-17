<p align="center">
  <img src="media/dashboard.png" alt="Skill-kits Dashboard" width="900" />
</p>

<h1 align="center">Skill-kits</h1>

<p align="center">
  <strong>把散落在各个 Agent 里的 Skills，收进一个本地优先的工作台。</strong><br/>
  一个 Rust 单二进制工具，用来扫描、审计、启停和部署 Codex、Claude Code、Gemini CLI 等 AI Agent 实际读取的 Skill。
</p>

<p align="center">
  <a href="https://github.com/scottcwy/skill-kits"><img src="https://img.shields.io/badge/status-v0.1_local_first-0f0f10?style=flat-square" alt="Project Status" /></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/Rust-1.80%2B-f5f5f5?style=flat-square&logo=rust&logoColor=white&color=1b1b1d" alt="Rust 1.80+" /></a>
  <a href="#快速开始"><img src="https://img.shields.io/badge/platform-macOS_first-1f1f22?style=flat-square" alt="macOS first" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-2a2a2d?style=flat-square" alt="MIT License" /></a>
</p>

<p align="center">
  <a href="#为什么需要-skill-kits">为什么</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;
  <a href="#核心能力">核心能力</a>&nbsp;&nbsp;&bull;&nbsp;&nbsp;
  <a href="#快速开始">快速开始</a>
</p>

<br/>

## 为什么需要 Skill-kits

AI Agent 的能力正在变成一堆本地文件：`~/.codex/skills`、`~/.claude/skills`、`~/.gemini/skills`、项目里的 `.agents/skills`、`.claude/skills`、`.gemini/skills`，还有插件缓存、供应商内置 Skill 和各类临时副本。

问题不在于文件多，而在于你很难快速回答这些问题：

- 哪些 Skill 正在被 Agent 读到？
- 哪些只是缓存、插件或只读来源？
- 哪个项目里有自己的 Skill 副本？
- 启用和禁用会不会误删目录？
- 项目副本和托管副本是否已经漂移？
- Codex 插件到底带来了哪些 runtime capabilities？

Skill-kits 的定位很直接：**它不接管你的 Agent，也不把 Skills 迁进云端。它扫描 Agent 实际读取的目录，并把状态、风险和可执行操作放到同一个本地工作台里。**

<p align="center">
  <img src="media/skills.png" alt="Skill-kits Skills view" width="900" />
</p>

## 核心能力

### 本地 Agent Space 索引

Skill-kits 以 Agent 正在读取的文件系统为准，扫描全局和项目级 Skill 目录，生成可查询的本地索引。

| Agent | 全局 Skill 目录 | 项目级 Skill 目录 |
| --- | --- | --- |
| Codex | `~/.codex/skills` | `<project>/.agents/skills` |
| Claude Code | `~/.claude/skills` | `<project>/.claude/skills` |
| Gemini CLI | `~/.gemini/skills` | `<project>/.gemini/skills` |
| Custom Agent | 本地配置 | 本地配置 |

它会区分：

- `Agent Space`：Agent 原生读取的全局 Skill。
- `Project Agent Space`：项目目录内的原生 Skill。
- `Plugin Cache`：Codex 插件缓存内暴露出的 Skill 或能力。
- `Vendor`：供应商或只读来源。

### 安全启停

启用和禁用 Skill 只做一件可逆的事：

```text
enabled   -> SKILL.md
disabled  -> SKILL.md.disabled
```

Skill-kits 不会为了禁用一个 Skill 删除整个目录。遇到同时存在 `SKILL.md` 和 `SKILL.md.disabled`，或两个文件都不存在的情况，会标记为 invalid toggle，阻止危险操作。

### 项目级 Skill 管理

项目是 Skill-kits 的一等场景。你可以把当前项目加入 Recent Projects，扫描项目内已有的 Skill，并对 Codex、Claude Code、Gemini CLI 或自定义 Agent 的项目 Skill 目录做独立管理。

已实现的项目流程包括：

- 扫描项目内原生 Skill。
- 将项目已有 Skill adopt 为 managed copy。
- 从 managed copy 部署到指定项目和 Agent。
- 启用、禁用、移除项目 Skill。
- 检测 drift、outdated、missing managed source。
- 对有本地改动的项目副本执行 overwrite 或 promote。

### Codex 插件与 Runtime Capabilities

Skill-kits 可以扫描 Codex 插件缓存，读取插件 manifest，展示插件状态，并把插件提供的能力展开成独立条目。

```bash
skill-kits plugin list
skill-kits plugin status <plugin>
skill-kits plugin enable <plugin>
skill-kits plugin disable <plugin>
skill-kits list --kind runtime-capability
```

这适合用来回答：某个 Codex 插件是否启用、来自哪个 provider、带了哪些 Skill、命令、Agent、app 或 asset。

### 轻量风险扫描

Skill-kits 内置 advisory scan，用来检查 Skill 文档和 shell 文件里的高风险指令。它不会替你做最终安全判断，也不会阻止安装，但会把风险提前抬到桌面上。

当前会提示的风险包括：

- `curl | sh` / `wget | sh`
- `rm -rf`
- `sudo`、`chmod +x`
- token、secret、credential、env、API key 访问
- 网络下载指令
- shell fence 中的未知二进制执行

### 原生 GUI + CLI

Skill-kits 是一个 Rust 单二进制应用。CLI 和 GUI 共用同一套核心逻辑。

GUI 提供五个工作台视图：

- **Dashboard**：Agent Space、项目、插件和健康状态总览。
- **Skills**：全局与项目 Skill 实例列表，支持状态、来源和路径检查。
- **Agents**：Codex、Claude Code、Gemini CLI 与 Custom Agent 的项目 Skill 目录配置。
- **Projects**：Recent Projects、项目 Skill 扫描、adopt、启停和 drift 处理。
- **Plugins**：Codex 插件、启停状态与 runtime capability 明细。

<p align="center">
  <img src="media/agents.png" alt="Skill-kits Agents view" width="900" />
</p>

## 快速开始

### 环境要求

- Rust 1.80 或更高版本
- macOS 是当前首发 GUI 目标
- 至少安装或配置一个支持 Skill 目录的 Agent

### 从源码运行

```bash
git clone https://github.com/scottcwy/skill-kits.git
cd skill-kits

cargo run -- status
cargo run -- scan
cargo run -- --gui
```

### 安装到本机

```bash
cargo install --path .

skill-kits status
skill-kits scan
skill-kits --gui
```

## 常用命令

### Agent Space

```bash
skill-kits status
skill-kits scan
skill-kits list
skill-kits enable <skill-or-instance-id>
skill-kits disable <skill-or-instance-id>
```

### Managed Copy

```bash
skill-kits install local <path-to-skill>
skill-kits uninstall <skill>
skill-kits scan <skill>
skill-kits adopt --global-agent codex
```

### Project

```bash
skill-kits project status --project /path/to/project
skill-kits project adopt <skill> --agent codex --project /path/to/project
skill-kits project deploy <skill> --agent codex --project /path/to/project
skill-kits project enable <skill> --agent codex --project /path/to/project
skill-kits project disable <skill> --agent codex --project /path/to/project
skill-kits project redeploy <skill> --agent codex --project /path/to/project --overwrite
skill-kits project redeploy <skill> --agent codex --project /path/to/project --promote
skill-kits project remove <skill> --agent codex --project /path/to/project --force
```

### Codex Plugins

```bash
skill-kits plugin list
skill-kits plugin status <plugin>
skill-kits plugin enable <plugin>
skill-kits plugin disable <plugin>
skill-kits plugin scan
skill-kits list --kind runtime-capability
```

所有主要 list/status/scan 命令都支持 JSON 输出：

```bash
skill-kits status --format json
skill-kits list --format json
skill-kits project status --format json
skill-kits plugin list --format json
```

## 本地数据

Skill-kits 的配置、索引、托管副本和 registry 默认存放在：

```text
~/.skill-kits/
├─ config.toml
├─ registry/
├─ skills/
├─ cache/
└─ locks/
```

Agent Space 索引只是扫描缓存。真正的启用状态以磁盘上的 `SKILL.md` / `SKILL.md.disabled` 为准。缓存过期时重新运行：

```bash
skill-kits scan
```

## 开发

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Roadmap

- 发布 macOS arm64 可下载包。
- 更完整的 GUI 截图、演示 GIF 和 release smoke 文档。
- 更丰富的 Agent 适配与自定义 Agent 配置体验。
- 更细的 Skill risk policy 与可解释扫描报告。
- 项目级工作流的导入、冲突处理和批量操作打磨。

## Contributing

欢迎提交 issue 和 PR。当前项目还在 v0.1 阶段，最需要的贡献包括：

- 真实 Agent 目录适配测试。
- macOS GUI 使用反馈。
- Skill 风险扫描规则补充。

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
