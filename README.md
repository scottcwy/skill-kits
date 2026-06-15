# Skill-kits

Skill-kits是一个零依赖 - 单二进制文件 - 任何LLM - 多智能体的 AI Agent Skills 管理工具

它用一个 Rust 单二进制文件，扫描并管理 Codex、Claude Code、Gemini CLI 等Agent的 Skill 目录，帮助你处理 Skill 的启动、禁用以及实现Skill的项目级启用。

## 特性

- Rust 原生应用，CLI 和 GUI 共用同一套核心逻辑。
- 本地优先，不依赖云服务，也不绑定任何 LLM 供应商。
- 支持全局和项目级 Skill 扫描。
- 通过 `SKILL.md` / `SKILL.md.disabled` 安全启用和禁用 Skill。
- 区分可写 Skill、只读插件缓存和供应商内置来源。
- 支持 Codex 插件状态和 runtime capabilities 查看。

## 安装与运行

要求：

- Rust 1.80 或更高版本
- macOS 为当前首发 GUI 目标

从源码运行：

```bash
cargo run -- status
cargo run -- list
cargo run -- --gui
```

安装到本机：

```bash
cargo install --path .
skill-kits status
skill-kits --gui
```

## 常用命令

```bash
skill-kits status
skill-kits scan
skill-kits list
skill-kits enable <skill>
skill-kits disable <skill>
skill-kits doctor
```

项目级 Skill：

```bash
skill-kits project status --project /path/to/project
skill-kits project enable <skill> --agent codex --project /path/to/project
skill-kits project disable <skill> --agent codex --project /path/to/project
```

Codex 插件：

```bash
skill-kits plugin list
skill-kits plugin status <plugin>
skill-kits plugin enable <plugin>
skill-kits plugin disable <plugin>
```

## 本地数据

Skill-kits 的配置和索引默认存放在：

```text
~/.skill-kits/
```

索引只是扫描缓存。如果缓存和磁盘不一致，以磁盘上的实际 Skill 文件为准，重新运行 `scan` 即可。

## 开发

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

当前仓库尚未包含 license 文件。
