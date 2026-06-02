# Skill-kits Implementation Design v0.1

## Purpose

This document turns `PRD-v0.1.md` into an implementation shape. It defines the Rust module boundaries, core data structures, TOML registry schema, command handlers, main workflows, error model, and test strategy for v0.1.

Update: `AGENT-SPACE-MANAGEMENT-SPEC.md` supersedes the managed-inventory-first parts of this implementation design. Keep the existing project deployment and registry machinery, but add an Agent Space read model where filesystem toggle state is the enablement truth and Tranche 1 scans do not write `skills.toml`.

The implementation should stay single-binary first. CLI and GUI must call the same core services rather than duplicating behavior.

## Architecture

Use one Rust crate for v0.1:

```text
skill-kits/
├─ Cargo.toml
└─ src/
   ├─ main.rs
   ├─ lib.rs
   ├─ app.rs
   ├─ cli/
   │  ├─ mod.rs
   │  ├─ args.rs
   │  ├─ output.rs
   │  └─ handlers.rs
   ├─ gui/
   │  ├─ mod.rs
   │  ├─ state.rs
   │  ├─ dashboard.rs
   │  ├─ skills.rs
   │  ├─ agents.rs
   │  └─ projects.rs
   └─ core/
      ├─ mod.rs
      ├─ paths.rs
      ├─ ids.rs
      ├─ fs.rs
      ├─ hash.rs
      ├─ config.rs
      ├─ registry.rs
      ├─ lock.rs
      ├─ agents.rs
      ├─ skills.rs
      ├─ install.rs
      ├─ adopt.rs
      ├─ project.rs
      ├─ scan.rs
      ├─ doctor.rs
      ├─ status.rs
      └─ error.rs
```

Keep the split simple:

- `main.rs` chooses GUI or CLI.
- `cli/*` parses commands, formats output, and calls core services.
- `gui/*` owns egui state and rendering only.
- `core/*` owns all business behavior and filesystem changes.

Avoid creating workspace crates until the single crate becomes painful. v0.1 does not need plugin crates, server crates, or database crates.

## Dependencies

Recommended v0.1 dependencies:

```toml
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
serde_json = "1"
camino = "1"
dirs = "5"
walkdir = "2"
fs_extra = "1"
sha2 = "0.10"
hex = "0.4"
tempfile = "3"
tracing = "0.1"
tracing-subscriber = "0.3"
comfy-table = "7"
eframe = "0.29"
egui = "0.29"
```

Do not add SQLite, reqwest, zip, tar, server, or YAML dependencies in v0.1.

## Runtime Paths

Default data root:

```text
~/.skill-kits/
├─ config.toml
├─ registry/
│  ├─ skills.toml
│  └─ deployments.toml
├─ skills/
├─ cache/
└─ locks/
   └─ state.lock
```

Path handling rules:

- Use `camino::Utf8PathBuf` internally when practical.
- Normalize paths before storing them in registry files.
- Project commands default to current working directory.
- `--project <path>` overrides the project command scope.
- Agent Space toggle may write only by renaming the selected writable instance's `SKILL.md` / `SKILL.md.disabled`.
- Do not write to plugin/cache/vendor roots by default.

## Core Types

Use small typed wrappers for identifiers:

```rust
pub struct SkillId(String);
pub struct AgentId(String);
pub struct ProjectId(String);
```

Suggested data structures:

```rust
pub struct ManagedSkill {
    pub id: SkillId,
    pub name: String,
    pub source: SkillSource,
    pub managed_path: Utf8PathBuf,
    pub content_hash: String,
    pub metadata: Option<SkillMetadata>,
    pub created_at: String,
    pub updated_at: String,
}

pub enum SkillSource {
    Local { source_path: Utf8PathBuf },
    GlobalAgentAdopt { agent_id: AgentId, source_path: Utf8PathBuf },
    ProjectAdopt { agent_id: AgentId, project_path: Utf8PathBuf, source_path: Utf8PathBuf },
    PromotedFromProject { deployment_id: String, project_path: Utf8PathBuf },
}

pub struct SkillMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub frontmatter: toml::value::Table,
}

pub struct AgentConfig {
    pub id: AgentId,
    pub label: String,
    pub kind: AgentKind,
    pub global_skill_dirs: Vec<Utf8PathBuf>,
    pub project_skill_dirs: Vec<Utf8PathBuf>,
    pub enabled: bool,
}

pub enum AgentKind {
    BuiltIn,
    Custom,
}

pub struct ProjectScope {
    pub name: String,
    pub path: Utf8PathBuf,
}

pub struct DeploymentRecord {
    pub id: String,
    pub skill_id: SkillId,
    pub agent_id: AgentId,
    pub project_name: String,
    pub project_path: Utf8PathBuf,
    pub deployment_path: Utf8PathBuf,
    pub skill_name: String,
    pub baseline_hash: String,
    pub deployed_from_hash: String,
    pub created_at: String,
    pub updated_at: String,
}

pub enum ToggleState {
    Enabled,
    Disabled,
    InvalidBothPresent,
    InvalidBothMissing,
}

pub struct DeploymentStatus {
    pub record: DeploymentRecord,
    pub toggle: ToggleState,
    pub current_hash: Option<String>,
    pub drift: bool,
    pub outdated: bool,
    pub missing_managed_source: bool,
}
```

Agent Space adds a scan/render model:

```rust
pub struct SkillInstance {
    pub id: String,
    pub stable_id: Option<SkillId>,
    pub name: String,
    pub agent_id: AgentId,
    pub scope: SkillInstanceScope,
    pub skill_dir: Utf8PathBuf,
    pub enabled_path: Utf8PathBuf,
    pub disabled_path: Utf8PathBuf,
    pub toggle_state: ToggleState,
    pub source_kind: SkillInstanceSourceKind,
    pub managed: bool,
    pub writable: bool,
    pub metadata: Option<SkillMetadata>,
    pub content_hash: Option<String>,
    pub updated_at: Option<String>,
}
```

`SkillInstance.id` is derived from `agent_id + scope_key + canonical skill_dir path`, not from name or content hash.

Time format can be RFC3339 strings in v0.1 to keep TOML and JSON output simple.

## Registry TOML Schema

`skills.toml`:

```toml
version = 1

[[skills]]
id = "frontend-design-a1b2c3d4"
name = "frontend-design"
managed_path = "/Users/me/.skill-kits/skills/frontend-design-a1b2c3d4"
content_hash = "..."
created_at = "2026-05-31T10:00:00Z"
updated_at = "2026-05-31T10:00:00Z"

[skills.source]
kind = "local"
source_path = "/Users/me/dev/frontend-design"

[skills.metadata]
title = "Frontend Design"
description = "Design production-grade frontend interfaces."
```

`deployments.toml`:

```toml
version = 1

[[deployments]]
id = "codex-frontend-design-a1b2c3d4-projecthash"
skill_id = "frontend-design-a1b2c3d4"
agent_id = "codex"
project_name = "my-app"
project_path = "/Users/me/dev/my-app"
deployment_path = "/Users/me/dev/my-app/.agents/skills/frontend-design"
skill_name = "frontend-design"
baseline_hash = "..."
deployed_from_hash = "..."
created_at = "2026-05-31T10:00:00Z"
updated_at = "2026-05-31T10:00:00Z"
```

`config.toml`:

```toml
version = 1
theme = "system"

[[agents]]
id = "codex"
label = "Codex"
kind = "built_in"
enabled = true
project_skill_dirs = [".agents/skills"]
global_skill_dirs = ["~/.codex/skills"]

[[agents]]
id = "claude"
label = "Claude Code"
kind = "built_in"
enabled = true
project_skill_dirs = [".claude/skills"]
global_skill_dirs = ["~/.claude/skills"]

[[agents]]
id = "gemini"
label = "Gemini CLI"
kind = "built_in"
enabled = true
project_skill_dirs = [".gemini/skills"]
global_skill_dirs = ["~/.gemini/skills"]

[[recent_projects]]
name = "my-app"
path = "/Users/me/dev/my-app"
last_opened_at = "2026-05-31T10:00:00Z"
```

Schema rules:

- Unknown fields should be ignored where possible.
- Missing registry files should initialize to empty registries.
- Invalid TOML is an error and should be reported by `doctor`.
- Registry writes must use lock plus atomic rename.

## ID and Hash Rules

Skill name:

- Derived from imported directory name.
- `SKILL.md` metadata is optional and does not define identity.

Skill ID:

- Generate from `name + short content hash`.
- Example: `frontend-design-a1b2c3d4`.
- If a collision occurs, extend the hash suffix.

Content hash:

- Hash the full Skill directory content.
- Include relative file paths and file bytes.
- Treat `SKILL.md` and `SKILL.md.disabled` as equivalent when hashing Agent Space instances and project deployment drift, so enable/disable rename does not change content identity.
- Keep Managed Skill hashing behavior compatible unless the managed-copy workflow is explicitly migrated.
- Ignore common filesystem noise: `.DS_Store`, editor swap files, and Skill-kits temp files.

Deployment baseline:

- `baseline_hash` records the project copy hash when deployed or adopted.
- `deployed_from_hash` records the Managed Skill hash used for deployment.
- `drift = current_project_hash != baseline_hash`.
- `outdated = managed_skill.content_hash != deployed_from_hash`.

## State Locking

All `~/.skill-kits` state mutations must:

1. Create or acquire `~/.skill-kits/locks/state.lock`.
2. Read the latest `config.toml`, `skills.toml`, or `deployments.toml` after acquiring the lock.
3. Write changed TOML files to temp files in the same directory.
4. Flush and atomically rename over each target file.
5. Release the lock.

Use a lock directory or lock file strategy that works on macOS without extra services. If a stale lock is detected, normal commands should report `registry busy`; `doctor --fix` may clear clearly stale locks.

## Agent Adapters

Define a trait for project directory behavior:

```rust
pub trait AgentAdapter {
    fn id(&self) -> AgentId;
    fn label(&self) -> &'static str;
    fn default_global_skill_dirs(&self) -> Vec<Utf8PathBuf>;
    fn default_project_skill_dirs(&self) -> Vec<Utf8PathBuf>;
    fn global_skill_dirs(&self, config: &AgentConfig) -> Vec<Utf8PathBuf>;
    fn project_skill_dirs(&self, config: &AgentConfig) -> Vec<Utf8PathBuf>;
}
```

Built-in adapters:

- Codex: global `~/.codex/skills`, project `.agents/skills`
- Claude Code: global `~/.claude/skills`, project `.claude/skills`
- Gemini CLI: global `~/.gemini/skills`, project `.gemini/skills`

Custom Agents use configured `project_skill_dirs`.

Adapters must not launch Agents, modify global Agent directories, inspect Agent auth, or read Agent runtime telemetry in v0.1.

## Filesystem Operations

Centralize filesystem writes in `core::fs`:

- `copy_dir_clean_source_to_empty_target`.
- `remove_dir_checked`.
- `rename_toggle_file`.
- `atomic_write_toml`.
- `ensure_dir`.
- `safe_read_to_string`.

Deletion rules:

- `uninstall` deletes only the Managed Skill directory under `~/.skill-kits/skills/`.
- `project remove` deletes only one selected deployment directory.
- `project remove --force` is required when drift exists.
- No operation deletes the original local source path.
- No operation deletes real global Agent skill directories.

## Workflows

### Local Install

Command:

```bash
skill-kits install local <path>
```

Flow:

1. Validate `<path>` is a directory.
2. Validate it contains `SKILL.md`.
3. Compute content hash.
4. Derive skill name from directory name.
5. Generate Skill ID.
6. Copy source into `~/.skill-kits/skills/<skill_id>/`.
7. Parse optional metadata from `SKILL.md`.
8. Write `skills.toml`.
9. Run advisory scan and attach risk summary for output.

Failure behavior:

- If target Managed Skill path already exists, fail with conflict.
- Never modify the source directory.

### Global Agent Adopt

Command:

```bash
skill-kits adopt --global-agent <agent>
```

Flow:

1. Resolve the configured global Agent skill directories only for reading.
2. Scan immediate child directories containing `SKILL.md` or `SKILL.md.disabled`.
3. For each Skill, compute content hash.
4. If same-name same-hash already exists, skip as already known.
5. If same-name different hash exists, produce Adoption Conflict.
6. Otherwise copy into Global Inventory and write `skills.toml`.

Global Agent Adopt never writes back to global Agent directories.

### Project Onboarding Scan

Triggered when GUI opens a project for the first time.

Flow:

1. Add project to Recent Projects.
2. For each enabled Agent, resolve project skill directories.
3. Scan immediate child directories for Skill deployments.
4. Detect toggle state.
5. Match existing Deployment Records.
6. Show discovered unmanaged project Skills.
7. Offer per-Skill adopt or Adopt all.

It must not auto-adopt and must not scan other recent projects.

### Project Adopt

Command:

```bash
skill-kits project adopt --agent <agent> [--project <path>]
```

Flow:

1. Resolve Project Scope.
2. Resolve Agent project skill directory.
3. Scan Skill directories.
4. Import non-conflicting Skills into Global Inventory.
5. Create Deployment Records for adopted project copies.
6. Record baseline hash without moving, overwriting, or renaming files.
7. Report partial success and conflicts.

Adoption Conflict resolution:

- Conflict Import: import as new Managed Skill with distinct Skill ID.
- Conflict Skip: leave unmanaged.

No merge or replace-existing behavior in v0.1.

### Project Deploy

Command:

```bash
skill-kits project deploy <skill> --agent <agent> [--project <path>]
```

Flow:

1. Resolve Managed Skill by ID or unique name.
2. Resolve Project Scope.
3. Resolve Agent project skill directory.
4. Compute target path: `<project_skill_dir>/<skill_name>`.
5. If target exists and is not recorded for this Skill, return Deploy Conflict.
6. Copy Managed Skill into target path.
7. Ensure `SKILL.md` exists and `SKILL.md.disabled` does not.
8. Compute baseline hash.
9. Write Deployment Record.
10. Return enabled deployment status.

Deploy creates enabled project deployments by default.

### Enable and Disable

Commands:

```bash
skill-kits project enable <skill> --agent <agent>
skill-kits project disable <skill> --agent <agent>
```

Flow:

1. Resolve deployment record by project, agent, and skill.
2. Read toggle state.
3. Enable: rename `SKILL.md.disabled` to `SKILL.md`.
4. Disable: rename `SKILL.md` to `SKILL.md.disabled`.
5. If both files exist or both are missing, return Invalid Toggle State.
6. Do not move or delete other files.

### Redeploy

Command:

```bash
skill-kits project redeploy <skill> --agent <agent> [--overwrite|--promote]
```

Flow:

1. Resolve deployment record and Managed Skill.
2. Compute current project hash.
3. Compute drift and outdated state.
4. If no drift, overwrite project copy from Managed Skill and update baseline.
5. If drift and no flag, block with Modified in Project.
6. If `--overwrite`, replace project copy with Managed Skill and update baseline.
7. If `--promote`, import project copy as new Managed Skill fork and update deployment to point to the fork.

Do not auto-merge.

### Remove From Project

Command:

```bash
skill-kits project remove <skill> --agent <agent> [--force]
```

Flow:

1. Resolve deployment record.
2. Compute drift.
3. If drift and no `--force`, block.
4. Delete only the selected deployment directory.
5. Remove Deployment Record.

Never delete the Agent project skill root.

### Uninstall

Command:

```bash
skill-kits uninstall <skill>
```

Flow:

1. Resolve Managed Skill.
2. Delete only `~/.skill-kits/skills/<skill_id>/`.
3. Remove the Managed Skill record.
4. Leave project deployments untouched.
5. Status may later show deployments with missing managed source.

### Missing Managed Source

A deployment has missing managed source when its project copy still exists but its `skill_id` no longer exists in Global Inventory.

Behavior:

1. Project status shows `Missing managed source`.
2. GUI Projects view offers only `Promote to managed` and `Remove from project`.
3. CLI does not auto-fix it.
4. `doctor --fix` does not auto-fix it.
5. `Promote to managed` creates a Managed Skill fork from the project copy and updates the Deployment Record.

## Scan

Scan output is advisory. It should produce risk entries with:

```rust
pub struct RiskFinding {
    pub severity: RiskSeverity,
    pub rule_id: String,
    pub path: Utf8PathBuf,
    pub line: Option<usize>,
    pub message: String,
}

pub enum RiskSeverity {
    Info,
    Warn,
    High,
}
```

Minimum rules:

- `curl | sh`, `wget | sh`, remote shell pipes.
- `rm -rf`, destructive deletes.
- `sudo`, `chmod +x`, privilege changes.
- token, secret, credential, env access patterns.
- network fetch instructions.
- unknown binary execution references.

Scan should read `SKILL.md` or `SKILL.md.disabled` plus obvious shell snippets in markdown. It does not execute anything.

## Doctor

Doctor check categories:

- Data root exists and is writable.
- Registry TOML files exist or can initialize.
- Registry TOML parses.
- Registry references existing Managed Skill directories.
- Managed Skill directories contain `SKILL.md`.
- Lock is not stale.
- Temp registry files are not stranded.
- Built-in and custom Agent project directories are valid relative paths.
- Recent Projects still exist.
- Known Deployment Records point to existing project paths when reachable.
- Known deployments do not have invalid toggle state.
- Deployment baseline exists where expected.
- Known deployments whose Managed Skill is missing are reported as missing managed source.

`doctor --fix` may:

- Remove stale locks.
- Remove missing Recent Projects.
- Delete leftover temp files.

`doctor --fix` must not:

- Delete project deployments.
- Overwrite project deployments.
- Promote project deployments.
- Modify Agent auth, API keys, or runtime settings.

## CLI Handlers

`cli::args` should define command enums with clap:

```rust
enum Command {
    List { format: OutputFormat },
    Status { format: OutputFormat },
    Install { command: InstallCommand },
    Uninstall { skill: String },
    Scan { skill: Option<String>, format: OutputFormat },
    Doctor { fix: bool },
    Adopt { global_agent: String },
    Project { command: ProjectCommand },
}

enum ProjectCommand {
    Status { project: Option<Utf8PathBuf>, format: OutputFormat },
    Adopt { agent: String, project: Option<Utf8PathBuf> },
    Deploy { skill: String, agent: String, project: Option<Utf8PathBuf> },
    Enable { skill: String, agent: String, project: Option<Utf8PathBuf> },
    Disable { skill: String, agent: String, project: Option<Utf8PathBuf> },
    Redeploy { skill: String, agent: String, project: Option<Utf8PathBuf>, overwrite: bool, promote: bool },
    Remove { skill: String, agent: String, project: Option<Utf8PathBuf>, force: bool },
}
```

Handlers should be thin:

1. Build `AppContext`.
2. Call core service.
3. Format result as table or JSON.
4. Map domain errors to exit codes.

Suggested exit codes:

- `0`: success.
- `1`: general error.
- `2`: invalid arguments.
- `3`: conflict or blocked operation.
- `4`: registry busy.
- `5`: doctor found errors.

## GUI Core Calls

GUI views should never mutate files directly. Each action calls a core service:

- Dashboard: `global_status`, `list_recent_projects`.
- Skills: `list_skills`, `scan_skill`, `uninstall_skill`.
- Agents: `load_config`, `update_agent_config`.
- Projects: `project_onboarding_scan`, `project_status`, `project_adopt`, `deploy`, `enable`, `disable`, `redeploy`, `remove_project`.

Long scans and copy operations should run outside the egui render path. v0.1 can use simple background threads and message channels; no async runtime is required.

## Error Model

Use `thiserror` for domain errors and `anyhow` at command boundaries.

Suggested domain errors:

```rust
pub enum SkillKitsError {
    RegistryBusy,
    RegistryParse { path: Utf8PathBuf },
    InvalidSkillDir { path: Utf8PathBuf, reason: String },
    SkillNotFound { query: String },
    AmbiguousSkill { query: String, matches: Vec<SkillId> },
    AgentNotFound { agent_id: AgentId },
    ProjectNotFound { path: Utf8PathBuf },
    DeployConflict { target: Utf8PathBuf },
    AdoptionConflict { name: String },
    InvalidToggleState { path: Utf8PathBuf },
    DeploymentDrift { deployment_id: String },
    MissingManagedSource { skill_id: SkillId, deployment_id: String },
    UnsafeRemoveRequiresForce { deployment_id: String },
    Io { source: std::io::Error },
}
```

Error messages should include the next action when possible:

- Deploy conflict: "target exists; adopt it, remove it, or choose another Skill name."
- Drift: "project copy has local changes; use `--overwrite`, `--promote`, or keep it."
- Registry busy: "another Skill-kits process is writing; retry or run doctor if stale."
- Missing managed source: "project copy still exists, but its Managed Skill is gone; promote it or remove it from the project."

## Test Strategy

Use temp directories for all filesystem tests.

P0 unit tests:

- Skill ID generation is stable and collision-safe.
- Directory hash changes when file content changes.
- Toggle state detects enabled, disabled, both present, both missing.
- Registry read/write round trip.
- Atomic registry write does not leave partial target on simulated failure where practical.
- Agent project directory resolution for Codex, Claude, Gemini, and Custom Agent.

P0 integration tests:

- Local install copies a Skill into Global Inventory and writes registry.
- Project deploy copies into the correct Agent directory and creates enabled `SKILL.md`.
- Disable and enable rename only the toggle file.
- Deploy blocks on unmanaged same-name target.
- Project adopt imports existing project Skill and records baseline.
- Adopt all produces partial success when conflicts exist.
- Redeploy blocks on drift.
- Redeploy `--overwrite` replaces project copy and updates baseline.
- Redeploy `--promote` creates a Managed Skill fork.
- Project remove requires `--force` when drift exists and deletes only selected deployment.
- Uninstall removes Global Inventory copy and leaves project deployments untouched.

P1 tests:

- Security scan flags minimum risky patterns.
- Doctor reports invalid TOML, missing managed directory, invalid toggle, stale lock.
- Doctor reports missing managed source without fixing it.
- `doctor --fix` removes stale lock, missing Recent Project, and temp files only.
- CLI JSON output is stable enough for scripts.
- CLI table output contains expected columns.

GUI tests can start as state-level tests rather than pixel tests. The first GUI milestone should verify that each view can load from a temp AppContext and trigger core actions without direct filesystem mutations from view code.

## Implementation Order

Recommended build order:

1. Paths, config defaults, registry read/write, lock, atomic write.
2. Skill validation, hashing, metadata parsing, Local Install.
3. Agent adapters and project scope resolution.
4. Project deploy, enable, disable, status.
5. Deployment baseline, drift, outdated, invalid toggle.
6. Project adopt and Adopt all partial success.
7. Redeploy, overwrite, promote, remove from project.
8. Scan and doctor.
9. CLI output formatting and exit codes.
10. egui Dashboard, Skills, Agents, Projects.
11. macOS binary packaging checks.

Do not start with GUI. Build the core and CLI first, then let GUI call stable core operations.

## Acceptance Criteria

v0.1 is releasable when, on macOS:

- `skill-kits` opens the egui GUI without external runtime dependencies.
- `skill-kits install local <path>` imports a valid Skill.
- `skill-kits project deploy/enable/disable/redeploy/remove` work against a temp project.
- Codex, Claude Code, and Gemini project directories resolve correctly.
- No command writes to real global Agent skill directories.
- Registry writes are locked and atomic.
- `scan` reports risks without blocking operations.
- `doctor --fix` performs only low-risk cleanup.
- `table` and `json` output work for status/list/project status.
- Core P0 tests pass.
