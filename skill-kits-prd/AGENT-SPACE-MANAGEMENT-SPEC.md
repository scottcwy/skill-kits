# Agent Space Skill Management Spec

Status: frozen for implementation planning on 2026-06-01.

## Goal

Skill-kits must manage the places Agents actually read from. A Skill is enabled when the Agent Space contains `SKILL.md`; it is disabled when the same Skill directory contains `SKILL.md.disabled`.

The Registry is not the enablement source of truth. It may support legacy records, summaries, risk reports, recovery, and managed inventory metadata, but Tranche 1 Agent Space scans do not write Registry state.

## Core Principle

```text
Agent reads it there, Skill-kits manages it there.
```

`SKILL.md` and `SKILL.md.disabled` are the toggle truth. A Registry flag must never be treated as sufficient to disable a Skill that an Agent can still see on disk.

## Concepts

**Agent Space** is any configured directory tree an enabled Agent reads for Skills. Examples include `~/.codex/skills`, `~/.claude/skills`, `~/.gemini/skills`, and project-local directories such as `<project>/.agents/skills`.

**Skill Instance** is one physical Skill directory discovered in an Agent Space. The Skills view is instance-first in Tranche 1: one row equals one `skill_dir`.

**Managed Inventory** is the Skill-kits-owned install/source area such as `~/.skill-kits/skills`. It remains useful for local install, deploy, backup, and managed-copy workflows, but it is not the Skills view's enablement truth.

**Scan Read Model** is the in-memory model produced by scanning Agent Spaces. Tranche 1 uses this model directly and does not persist discovered instances to `skills.toml`.

**Import Managed Copy** is the explicit operation that copies an Agent Space Skill into Managed Inventory. It replaces ambiguous "Adopt" wording for copy/import behavior.

## SkillInstance Model

The implementation should introduce a render/core model equivalent to:

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

`id` is a stable instance id:

```text
hash(agent_id + scope_key + canonical skill_dir path)
```

It must not include Skill name or content hash, because renaming `SKILL.md` or editing content must not change row identity.

`stable_id` or `skill_id` is only an association hook for managed copies, risk findings, and legacy deployment records. It is not the identity used for selection or toggle.

## Toggle State

Toggle state is derived from the filesystem:

| Files in `skill_dir` | State |
| --- | --- |
| `SKILL.md` only | `Enabled` |
| `SKILL.md.disabled` only | `Disabled` |
| both files | `InvalidBothPresent` |
| neither file, but a legacy record references the directory | `InvalidBothMissing` / `Missing` |

Disabled is a valid state. Metadata and content hash should be read from `SKILL.md.disabled`, with hash path normalization treating `SKILL.md.disabled` as `SKILL.md` so enable/disable rename does not change content identity.

`InvalidBothPresent` is strict: use the directory name, do not compute normal metadata or content hash, disable toggle, and show both file paths in the Inspector. Tranche 1 does not auto-repair this state.

Pure filesystem scanning should not invent both-missing rows. Missing rows only appear when a legacy Registry or deployment record points to a Skill directory whose toggle file is gone.

## Scan Rules

Routine Agent Space scan covers:

- all enabled Agents' configured global Skill directories
- all enabled Agents' project Skill directories inside Recent Projects
- read-only plugin/cache/vendor roots that the Agent configuration declares as visible to the Agent

Routine scan does not run a workspace crawler. HarnessKit-style `Discover Projects(root)` is deferred to GUI productization in Tranche 3 or 4.

Normal Agent Space roots use immediate-child scanning:

```text
<root>/<skill>/SKILL.md
<root>/<skill>/SKILL.md.disabled
```

Plugin/cache/vendor roots use bounded recursive discovery, maximum depth 4. Discovery stops at the first directory containing `SKILL.md` or `SKILL.md.disabled`. It should skip hidden and common generated/noisy directories such as `node_modules`, `target`, `__pycache__`, `vendor`, `dist`, `build`, `venv`, and `.venv`.

Tranche 1 scans do not write the Registry. They may read Registry state to mark `managed`, show legacy deployment links, or surface missing legacy records.

## Source Kinds

Source kind drives both copy and toggle behavior:

| Source kind | Meaning | Toggle |
| --- | --- | --- |
| `AgentSpace` | Writable global Agent Space | Allowed when writable |
| `ProjectDeployment` | Writable project Agent Space | Allowed when writable |
| `PluginCache` | Agent-visible plugin cache | Read-only |
| `Vendor` | Agent-visible vendor/import cache | Read-only |
| `ManagedInventory` | Skill-kits-owned source/inventory copy | Not part of instance table by default |

`~/.skills-manager/skills` must not remain in Codex built-in global Agent Space defaults. If it is shown, it should be shown as managed/source summary, not as a Codex-readable Agent Space unless an Agent config explicitly declares it.

## Toggle Rules

Enable and disable operate only on the selected Skill Instance.

Enable:

```text
SKILL.md.disabled -> SKILL.md
```

Disable:

```text
SKILL.md -> SKILL.md.disabled
```

The operation must not delete the Skill directory, mutate Managed Inventory, write an enablement flag to Registry, or affect same-name copies in other scopes.

Toggle is blocked when:

- both toggle files are present
- both toggle files are missing
- the instance path is not writable
- the source kind is `PluginCache` or `Vendor`
- the selected row is only a Managed Inventory summary

## GUI Shape

The primary GUI views are:

```text
Dashboard / Skill / Agent / Project
```

The Skill view shows Agent Space scan results, not only Managed Inventory. Tranche 1 is instance-first: one row equals one physical Skill directory.

The Skill view should expose these instance fields:

- Skill
- Agent
- Scope
- Status
- Source
- Managed
- Updated

Status values:

- Enabled
- Disabled
- Invalid
- Missing
- Read-only

The Inspector shows:

- real `skill_dir`
- enabled and disabled file paths
- Agent
- Scope
- writable/read-only state
- managed/unmanaged state
- metadata
- content hash when available
- risk findings when available
- project deployment links when available

Action wording:

- `Scan Agent Spaces` refreshes the read model.
- `Install local` imports a local Skill into Managed Inventory.
- `Import managed copy` copies an Agent Space Skill into Managed Inventory.
- `Deploy to Project` copies from Managed Inventory into a project Agent Space.
- `Enable` / `Disable` rename the selected instance toggle file only.
- `Uninstall managed copy` removes only the Managed Inventory copy.

Disable copy:

```text
Disable changes SKILL.md to SKILL.md.disabled in the Agent Space.
It does not delete the Skill directory.
```

Dashboard primary counts should lead with Agent Space instances. Managed Inventory becomes a secondary summary.

## Tranches

### Tranche 1: Agent Space Scan / Read Model

- Add `SkillInstance` or equivalent render model.
- Scan enabled Agent global dirs and Recent Projects.
- Recognize `SKILL.md.disabled`.
- Show Agent Space instances in the Skill view.
- Keep scans read-only and Registry-free.
- Treat plugin/cache/vendor as read-only.

Tests:

- enabled instance
- disabled instance
- invalid both present
- legacy both missing / missing
- plugin/cache/vendor read-only
- Recent Projects project scan

### Tranche 2: Toggle

- Enable/disable through rename only.
- Restrict toggle to writable Agent Space instances.
- Block invalid, missing, plugin/cache/vendor, and read-only paths.

Tests:

- disable does not delete directory
- enable/disable does not mutate Managed Inventory
- toggle does not affect unrelated project/global copies
- rescan preserves disabled state

### Tranche 3: GUI Productization

- Status column and filters.
- Agent and Scope filters.
- Inspector copy for invalid/read-only state.
- Inline confirmation copy for disable.
- Optional `Discover Projects(root)` entry point modeled after HarnessKit.

### Tranche 4: Inventory Relationship Cleanup

- Rename ambiguous Adopt surfaces.
- Make `Import managed copy` an explicit secondary action.
- Update Dashboard counts to Agent Space instances plus Managed Inventory summary.
- Keep Registry as auxiliary cache/recovery state, not enablement truth.

## Non-goals

- Do not implement `skill-kits run codex`.
- Do not add a scheduler or launcher integration.
- Do not rely on `enabled=false` Registry flags for Agent-visible Skills.
- Do not bulk-copy all Agent Space Skills into Managed Inventory by default.
- Do not modify plugin/cache/vendor roots by default.
