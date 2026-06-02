# Skill-kits PRD v0.1

## Product Position

Skill-kits is a single-binary, local-first AI Agent Skills manager. It keeps a global inventory of managed Skills, then deploys selected Skills into project-scoped Agent skill directories.

Update: `AGENT-SPACE-MANAGEMENT-SPEC.md` supersedes the managed-inventory-first parts of this v0.1 PRD. The active model is Agent Space first: Skill-kits scans and manages the directories Agents actually read from, and Managed Inventory is an install/deploy source rather than the enablement truth.

Core principles:

- Single binary first.
- No Electron, WebView, Node, or Python runtime.
- CLI and GUI share the same Rust core.
- Agent Space filesystem state is the enablement truth.
- Managed Inventory is separate from Agent Space instances and project deployments.
- Project-level enablement uses native Agent project directories.
- v0.1 is offline-first: local install and adopt only.

## v0.1 Scope

In scope:

- Global Inventory under `~/.skill-kits/skills/`.
- Agent Space scan read model for configured global Agent directories and Recent Projects.
- Local Skill install from a directory.
- Adopt from global Agent skill directories.
- Adopt from project Agent skill directories.
- Project-native deploy by copying Managed Skills into project Agent skill directories.
- Enable and disable through `SKILL.md` / `SKILL.md.disabled` rename.
- Drift, outdated, invalid toggle, and conflict detection.
- Advisory security scan.
- Lightweight doctor checks and low-risk `doctor --fix`.
- egui GUI with Dashboard, Skill, Agent, and Project views.
- macOS single binary first.

Out of scope for v0.1:

- Global `sync --agent`.
- Server mode and remote management.
- Rollback snapshots.
- GitHub download, marketplace, or remote index.
- Launcher, isolated home, active dir, shell hooks, or daemon.
- SQLite.
- YAML output.
- Agent auth, API key, network, or runtime telemetry checks.

## Agent Support

Built-in project skill directories:

| Agent | Project Skill Directory |
| --- | --- |
| Codex | `<project>/.agents/skills` |
| Claude Code | `<project>/.claude/skills` |
| Gemini CLI | `<project>/.gemini/skills` |

Agent Space global directories:

| Agent | Global Skill Directory |
| --- | --- |
| Codex | `~/.codex/skills` |
| Claude Code | `~/.claude/skills` |
| Gemini CLI | `~/.gemini/skills` |

Custom Agents can define project skill directories in config. v0.1 only supports project-level enablement for Agents with native project skill directories.

Codex defaults must not include `~/.skills-manager/skills` as an Agent Space directory. Plugin/cache/vendor directories may be shown as read-only Agent-visible sources when configured by the Agent adapter.

## Data Model

Global data lives under `~/.skill-kits/`:

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

Registry and config state are TOML. All writes to `config.toml`, `skills.toml`, and `deployments.toml` take `state.lock` and write through temp file plus atomic rename.

Agent Space scans do not write Registry state. Registry can support Managed Inventory, known deployments, recovery, and summaries, but it is not the enablement source of truth.

Each Managed Skill has a stable `skill_id`; display name comes from the imported directory name. `SKILL.md` frontmatter or heading is optional metadata only.

## Project Deployment

Deploy copies a Managed Skill into an Agent project directory:

```text
project/.agents/skills/frontend-design/
project/.claude/skills/frontend-design/
project/.gemini/skills/frontend-design/
```

Deploy creates an enabled deployment by default:

```text
enabled:  SKILL.md
disabled: SKILL.md.disabled
```

Disable only renames `SKILL.md` to `SKILL.md.disabled`; it does not move or delete the directory. Remove from project deletes only the selected Skill deployment directory, not the Agent skill root. If that deployment has local drift, GUI requires confirmation and CLI requires `--force`.

Deploy does not overwrite unmanaged same-name project directories. It reports a deploy conflict.

## Updates and Drift

Managed Skill updates do not automatically overwrite project copies.

Project deployment states:

- `outdated`: source Managed Skill changed since deployment.
- `drift`: project copy changed since deployment baseline.
- `invalid toggle`: both `SKILL.md` and `SKILL.md.disabled` exist, or both are missing.
- `missing managed source`: project copy exists, but the source Managed Skill is no longer in Global Inventory.

Redeploy is explicit. If drift exists, redeploy blocks by default. User choices:

- Keep project copy.
- Overwrite from managed.
- Promote to managed.

Promote creates a new Managed Skill fork by default and never replaces the original Managed Skill in v0.1.

If a project deployment has a missing managed source, the Projects view shows `Missing managed source`. The available actions are `Promote to managed` or `Remove from project`. It is not auto-fixed.

## Adopt

Adopt imports existing Skills into Global Inventory in legacy v0.1 flows. New Agent Space UI language should prefer `Scan Agent Spaces` for read-only refreshes and `Import managed copy` for explicit copy into Managed Inventory.

Commands:

```bash
skill-kits adopt --global-agent <agent>
skill-kits project adopt --agent <agent> [--project <path>]
```

Project Adopt also records a deployment baseline linking the existing project copy to the new Managed Skill. It does not move, overwrite, or rename files.

Project onboarding scan discovers project Skills but does not automatically adopt. User chooses `Adopt all` or per-Skill adopt.

`Adopt all` uses partial success: non-conflicting Skills import; conflicts are reported separately. Adoption conflicts can be resolved by importing as a new Managed Skill or skipping. v0.1 does not merge or replace existing Skills.

## CLI

Global commands:

```bash
skill-kits
skill-kits list [--format table|json]
skill-kits status [--format table|json]
skill-kits install local <path>
skill-kits uninstall <skill>
skill-kits scan [<skill>] [--format table|json]
skill-kits doctor [--fix]
skill-kits adopt --global-agent <agent>
```

Project commands:

```bash
skill-kits project status [--project <path>] [--format table|json]
skill-kits project adopt --agent <agent> [--project <path>]
skill-kits project deploy <skill> --agent <agent> [--project <path>]
skill-kits project enable <skill> --agent <agent> [--project <path>]
skill-kits project disable <skill> --agent <agent> [--project <path>]
skill-kits project redeploy <skill> --agent <agent> [--project <path>] [--overwrite|--promote]
skill-kits project remove <skill> --agent <agent> [--project <path>] [--force]
```

Project commands default to current directory. `--project <path>` overrides scope.

## Status and Doctor

Global status reports:

- Managed Skill count.
- Agent count and configuration state.
- Recent Project count.
- Registry, lock, and cache health.
- Risk count last.

Project status reports:

- Agent project skill directories.
- Deployed Skills.
- Enabled or disabled state.
- Outdated deployments.
- Drift.
- Invalid toggles.

Doctor checks only Skill-kits-owned or recorded state. It does not inspect Agent auth, API keys, network, model availability, or runtime telemetry.

`doctor --fix` may clear stale locks, remove missing Recent Projects, and delete leftover temp files. It must not delete, overwrite, or promote project copies.

## Security Scan

Security scan is advisory in v0.1. It produces a risk report and does not block install, adopt, or deploy.

Minimum scan rules flag:

- `curl | sh` and similar network-to-shell patterns.
- `rm -rf` and destructive filesystem commands.
- `sudo`, `chmod +x`, and privilege-changing instructions.
- token, secret, env, or credential access patterns.
- network fetch instructions.
- unknown binary execution references.

Policy enforcement is future work.

## GUI

Navigation order:

1. Dashboard
2. Skill
3. Agent
4. Project

Dashboard opens by default and leads with Agent Space instance counts plus Managed Inventory summary.

Skill view handles Agent Space Skill Instance rows, status, paths, risk panel, enable/disable, and explicit managed-copy actions.

Agent view handles Codex, Claude Code, Gemini CLI, and Custom Agent project directory configuration.

Project view handles Recent Projects, onboarding scan, deployments, enable, disable, remove, redeploy, drift, outdated state, missing managed source, and invalid toggle state.

GUI startup reads registry, config, and Recent Project summaries, then builds the Agent Space read model from configured global directories and Recent Projects. It does not run workspace-wide project discovery; HarnessKit-style `Discover Projects(root)` is deferred to later GUI productization.

Skill-kits may show Git ignore guidance, but v0.1 does not edit `.gitignore` automatically and does not force a commit-or-ignore recommendation. The GUI ships dark theme only in v0.1 and uses platform system fonts without bundling custom fonts.

## Release

v0.1 ships macOS single binary first. Windows and Linux builds follow after the core flow is stable and verified.
