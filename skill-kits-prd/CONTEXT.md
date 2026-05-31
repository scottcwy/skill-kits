# Skill-kits

Skill-kits is a single-binary, local-first manager for AI Agent Skills. It owns imported skill copies, then deploys them into project-scoped Agent skill directories.

## Language · 词表

**Skill**:
A directory containing a `SKILL.md` file that describes reusable agent capability.
_Avoid_: Plugin, package, script

**Skill ID**:
A stable internal identifier used by Skill-kits to track one Managed Skill.
_Avoid_: Skill name, folder name

**Skill Name**:
The human-readable name shown in the GUI and used as the default Agent folder name.
_Avoid_: Skill ID, identity

**Skill Metadata**:
Optional descriptive information parsed from `SKILL.md` frontmatter or heading.
_Avoid_: Skill identity, required manifest

**Managed Skill**:
A Skill copy owned by Skill-kits under `~/.skill-kits/skills/`.
_Avoid_: Source skill, original skill

**Source Path**:
The original local path from which a Skill was imported.
_Avoid_: Managed path, install path

**Agent**:
A supported AI coding tool with a configured skills directory.
_Avoid_: Client, runtime, provider

**Uninstall**:
The removal of a Managed Skill from the Global Inventory.
_Avoid_: Delete, remove source

**Remove From Project**:
The removal of a Project Skill Deployment from one Project Scope.
_Avoid_: Uninstall, disable

**Forced Project Removal**:
A confirmed removal of a Project Skill Deployment that contains Deployment Drift.
_Avoid_: Disable, silent deletion

**Adopt**:
The import of an existing Agent skill into Skill-kits management.
_Avoid_: Sync, overwrite, takeover

**Global Agent Adopt**:
An Adopt operation that imports skills from real global Agent skills directories into Global Inventory without writing back.
_Avoid_: Global sync, deploy

**Global Skill Directory**:
A real Agent skills directory that Skill-kits may read during Global Agent Adopt.
_Avoid_: Global sync target, project skill directory

**Project Adopt**:
An Adopt operation that imports skills from a Project Skill Directory into Global Inventory.
_Avoid_: Deploy, redeploy

**Adoption Conflict**:
A same-name Agent skill set whose contents differ across Agents during Adopt.
_Avoid_: Duplicate, sync conflict

**Conflict Import**:
An Adoption Conflict resolution that imports the conflicting Skill as a new Managed Skill.
_Avoid_: Merge, replace existing

**Conflict Skip**:
An Adoption Conflict resolution that leaves the conflicting Skill unmanaged.
_Avoid_: Delete, overwrite

**Project Scope**:
A project-specific management scope identified by project name and path.
_Avoid_: Global scope, current working directory

**Project Skill Directory**:
An Agent-specific directory inside a project where that Agent reads project-local Skills.
_Avoid_: Active Directory, global Agent directory

**Project-supported Agent**:
An Agent that declares at least one Project Skill Directory.
_Avoid_: Launch-supported agent, global-only agent

**Custom Agent**:
A user-configured Agent with one or more custom Project Skill Directories.
_Avoid_: Built-in agent, deep adapter

**CLI Output Format**:
A supported command output mode for human or machine consumption.
_Avoid_: Registry format, config format

**Dashboard View**:
The default GUI section showing Global Inventory with the Scope Switcher.
_Avoid_: Project-only home, marketing page

**Skills View**:
The GUI section for Global Inventory listing, detail, scan risk, and Uninstall.
_Avoid_: Project deployments, Markets

**Agents View**:
The GUI section for built-in and custom Agent project skill directory configuration.
_Avoid_: Project deployments, launcher config

**Projects View**:
The GUI section for Recent Projects and project deployments, including enable, disable, remove, and redeploy.
_Avoid_: Global Inventory, global sync

**Project Skill Deployment**:
A physical copy of a Managed Skill into a Project Skill Directory.
_Avoid_: Agent Link, active overlay

**Deployment Baseline**:
The recorded content hash linking a Project Skill Deployment to the Managed Skill state it came from.
_Avoid_: Current hash, latest managed hash

**Deployment Record**:
A Registry record linking one Project Skill Deployment to a Managed Skill, Agent, Project Scope, and Deployment Baseline.
_Avoid_: Project marker, global sync marker

**Deploy Conflict**:
A state where Deploy targets a Project Skill Directory entry that already exists and is not linked to the selected Managed Skill.
_Avoid_: Adoption Conflict, Deployment Drift

**Skill Toggle File**:
The `SKILL.md` or `SKILL.md.disabled` file whose name determines whether a Project Skill Deployment is enabled.
_Avoid_: Project Profile switch, registry state

**Default Enabled Deployment**:
A Project Skill Deployment created with `SKILL.md` active immediately after Deploy.
_Avoid_: Disabled by default

**Invalid Toggle State**:
A Project Skill Deployment state where `SKILL.md` and `SKILL.md.disabled` both exist or both are missing.
_Avoid_: Disabled skill, scan warning

**Outdated Deployment**:
A Project Skill Deployment whose source Managed Skill has changed since the last deployment.
_Avoid_: Disabled skill, conflict

**Deployment Drift**:
Project-local changes inside a Project Skill Deployment after it was copied from a Managed Skill.
_Avoid_: Outdated Deployment, source update

**Keep Project Copy**:
A drift resolution that preserves the Project Skill Deployment without overwriting it.
_Avoid_: Redeploy, promote

**Overwrite From Managed**:
A drift resolution that replaces a Project Skill Deployment with its source Managed Skill.
_Avoid_: Merge, promote

**Promote To Managed**:
A drift resolution that imports a Project Skill Deployment back into the Managed Skill inventory.
_Avoid_: Overwrite, automatic merge

**Managed Skill Fork**:
A new Managed Skill created from a project-local deployment instead of replacing the original Managed Skill.
_Avoid_: Managed replacement, global update

**Global Inventory**:
The full set of Managed Skills owned by Skill-kits.
_Avoid_: Project scope, deployed skills

**Registry**:
The TOML file set under `~/.skill-kits/registry/` that records Global Inventory and known deployments.
_Avoid_: Database, cache

**Registry Lock**:
The single write lock at `~/.skill-kits/locks/state.lock` required for Skill-kits state mutations.
_Avoid_: Project lock, database transaction

**Missing Managed Source**:
A Project Skill Deployment state where the project copy still exists but its source Managed Skill is no longer in Global Inventory.
_Avoid_: Deployment Drift, Outdated Deployment

**Security Scan**:
A non-blocking risk analysis of Skill content and deployment state.
_Avoid_: Sandbox, policy enforcement

**Risk Report**:
The output of Security Scan shown in CLI and GUI.
_Avoid_: Install failure, blocked deployment

**Doctor Check**:
A lightweight consistency check over Skill-kits-owned state.
_Avoid_: Agent health check, network test

**Doctor Fix**:
A low-risk automatic repair performed by `doctor --fix`.
_Avoid_: Project overwrite, destructive repair

**Project Onboarding Scan**:
A one-time full scan of a newly opened project, its Agent project directories, and Skill deployments.
_Avoid_: GUI startup scan, background filesystem scan

**Project Refresh Scan**:
A user-triggered or command-triggered scan of one Project Scope after onboarding.
_Avoid_: Startup scan, all-project scan

**Adopt All**:
A user-confirmed batch Project Adopt operation for discovered project Skills.
_Avoid_: Automatic adopt, scan result

**Partial Adopt Success**:
A batch Adopt result where non-conflicting Skills are imported and conflicts are reported separately.
_Avoid_: All-or-nothing adopt, silent skip

**Git Ignore Guidance**:
A non-mutating recommendation showing which project skill directories a user may want to ignore or commit.
_Avoid_: Automatic .gitignore edit

**Minimum Scan Rules**:
The v0.1 Security Scan rule set for suspicious markdown and command patterns.
_Avoid_: Sandbox enforcement, exhaustive static analysis

**Release Target**:
The supported operating system and packaging target for a release milestone.
_Avoid_: Runtime dependency, deployment backend

**Policy Enforcement**:
An optional future mode that can block operations based on Security Scan results.
_Avoid_: Default scan behavior

**Atomic Registry Write**:
A Registry write performed through a temporary file followed by atomic rename.
_Avoid_: In-place write, partial write

**Scope Switcher**:
The GUI control that switches between Global Inventory and a selected Project Scope.
_Avoid_: Agent selector, theme switcher

**Recent Project**:
A project directory explicitly opened by the user and remembered by Skill-kits.
_Avoid_: Auto-discovered project, workspace scan result

**Project Command Scope**:
The project path used by CLI project commands, defaulting to the current working directory unless `--project` is provided.
_Avoid_: Global command scope, Recent Project

**Deploy**:
The project command that copies a Managed Skill into an Agent Project Skill Directory.
_Avoid_: Sync, install

**Redeploy**:
The project command that updates an existing Project Skill Deployment from its Managed Skill.
_Avoid_: Sync, automatic update

**Local Install**:
The import of a local Skill directory into Global Inventory.
_Avoid_: Online download, deploy

**Global Command**:
A CLI command that manages Global Inventory and never writes to project skill directories.
_Avoid_: Project command, Agent sync

**Project Command**:
A CLI command that manages one Project Scope and may write to Project Skill Directories.
_Avoid_: Global command, Agent sync

**Global Status**:
The global CLI summary of inventory counts, agent configuration, recent projects, and registry health.
_Avoid_: Project deployment status, per-project detail

**Project Status**:
The project CLI summary of Agent project directories and deployment states in one Project Scope.
_Avoid_: Global inventory status, Agent runtime status

## Relationships · 关系

- A **Skill** must have exactly one `SKILL.md`.
- A **Managed Skill** must have exactly one **Skill ID**.
- A **Managed Skill** must have exactly one **Skill Name**.
- A **Managed Skill** may have zero or one **Skill Metadata** record.
- A **Managed Skill** may remember zero or one **Source Path**.
- **Adopt** creates exactly one **Managed Skill** from exactly one existing Agent skill.
- **Global Agent Adopt** reads from one or more real **Global Skill Directories**.
- Built-in **Global Skill Directories** are Codex `~/.codex/skills`, Claude Code `~/.claude/skills`, and Gemini CLI `~/.gemini/skills`.
- **Project Adopt** reads from one Project Skill Directory in one Project Scope.
- **Adopt** may produce zero or more **Adoption Conflicts**.
- An **Adoption Conflict** contains two or more same-name Agent skills with different content hashes.
- An **Adoption Conflict** may resolve through **Conflict Import** or **Conflict Skip** in v0.1.
- A **Project Scope** must identify exactly one project path.
- An **Agent** may define zero or more **Project Skill Directories**.
- A **Project-supported Agent** must define one or more **Project Skill Directories**.
- A **Custom Agent** may define one or more user-configured **Project Skill Directories**.
- v0.1 **CLI Output Format** values are `table` and `json`.
- v0.1 GUI sections are ordered as **Dashboard View**, **Skills View**, **Agents View**, and **Projects View**.
- A **Project Skill Deployment** belongs to exactly one **Project Scope** and exactly one **Agent**.
- A **Project Skill Deployment** is a copy of one **Managed Skill** by default.
- A **Project Skill Deployment** may have exactly one **Deployment Baseline**.
- A **Project Skill Deployment** may have exactly one **Deployment Record**.
- A **Project Skill Deployment** has exactly one **Skill Toggle File**.
- **Deploy** creates a **Default Enabled Deployment** by default.
- **Deploy** must stop on **Deploy Conflict** unless the user chooses a later explicit resolution.
- A **Project Skill Deployment** must not contain an **Invalid Toggle State**.
- A **Project Skill Deployment** may become an **Outdated Deployment** when its Managed Skill changes.
- A **Project Skill Deployment** may contain **Deployment Drift** when project-local files change.
- A **Project Skill Deployment** may have **Missing Managed Source** when its Managed Skill is removed from Global Inventory.
- **Deployment Drift** may be resolved by **Keep Project Copy**, **Overwrite From Managed**, or **Promote To Managed**.
- **Promote To Managed** creates exactly one **Managed Skill Fork** by default.
- The GUI opens to **Global Inventory** by default.
- The **Registry** is stored as TOML files in v0.1.
- `config.toml`, `skills.toml`, and `deployments.toml` mutations must hold the **Registry Lock**.
- `config.toml`, `skills.toml`, and `deployments.toml` mutations must use **Atomic Registry Write**.
- **Security Scan** creates one **Risk Report** and does not block install, adopt, or deploy by default.
- **Security Scan** uses **Minimum Scan Rules** in v0.1.
- **Doctor Check** must stay limited to Skill-kits-owned or recorded state.
- **Doctor Fix** may clear stale locks, forget missing Recent Projects, and delete leftover temporary files.
- GUI startup must not recursively scan all Recent Projects.
- **Project Onboarding Scan** scans one newly opened project fully.
- **Project Refresh Scan** scans one selected Project Scope after onboarding.
- **Project Onboarding Scan** must not perform **Adopt All** automatically.
- **Adopt All** may produce **Partial Adopt Success**.
- Skill-kits provides **Git Ignore Guidance** but does not edit `.gitignore` automatically in v0.1.
- A **Scope Switcher** may select one **Project Scope** for project-level management.
- A **Recent Project** must come from an explicit user-opened project directory.
- A **Project Command Scope** must resolve to exactly one project path.
- **Deploy** creates one **Project Skill Deployment**.
- **Redeploy** updates one existing **Project Skill Deployment** unless blocked by Deployment Drift.
- **Local Install** creates one **Managed Skill** from one local Source Path.
- v0.1 **Global Commands** are GUI launch, list, status, local install, uninstall, scan, doctor, and global-agent adopt.
- v0.1 **Project Commands** are project status, project adopt, deploy, enable, disable, redeploy, and remove.
- **Global Status** reports Managed Skill count, Agent count, Agent configuration state, Recent Project count, registry, lock, and cache health, and risk count last.
- **Project Status** reports Project Skill Directories, deployed Skills, enabled or disabled state, Outdated Deployments, Deployment Drift, Missing Managed Source, and Invalid Toggle State.
- v0.1 **Release Target** is a macOS single binary first, with Windows and Linux builds after the core flow passes.
- **Uninstall** removes exactly one **Managed Skill** from **Global Inventory** and does not delete project copies by default.
- **Remove From Project** removes exactly one **Project Skill Deployment** from one **Project Scope**.
- **Forced Project Removal** may remove one Project Skill Deployment with Deployment Drift after explicit confirmation.

## Example dialogue · 示例对话

> **Dev:** "When a user uninstalls a Skill in the GUI, do we delete the original folder?"
> **Domain expert:** "No. Uninstall removes the Managed Skill from Global Inventory, but never deletes the Source Path."

> **Dev:** "Does Uninstall remove copies already deployed into projects?"
> **Domain expert:** "No. Project copies are removed only through Remove From Project."

> **Dev:** "Does Remove From Project delete every skill in the project Agent directory?"
> **Domain expert:** "No. It deletes only the selected Project Skill Deployment directory."

> **Dev:** "Can a user remove a project copy that has local changes?"
> **Domain expert:** "Yes, but only through Forced Project Removal with GUI confirmation or CLI `--force`."

> **Dev:** "How do we automatically synchronize skills that already exist in Codex before Skill-kits is installed?"
> **Domain expert:** "Run Adopt first. Adopt imports existing Agent skills into Global Inventory; project usage then goes through Deploy."

> **Dev:** "Can Adopt import project-local skills too?"
> **Domain expert:** "Yes. Global Agent Adopt imports from real global Agent directories; Project Adopt imports from one Project Skill Directory."

> **Dev:** "After Project Adopt, does Skill-kits know that the existing project copy belongs to the new Managed Skill?"
> **Domain expert:** "Yes. Project Adopt records a Deployment Baseline linking the project copy to its Managed Skill without moving or modifying files."

> **Dev:** "If Codex and Claude both have `frontend-design`, do we merge them?"
> **Domain expert:** "Only when their content hashes match; otherwise Adopt creates an Adoption Conflict for the user to resolve."

> **Dev:** "How can a user resolve an Adoption Conflict in v0.1?"
> **Domain expert:** "Import it as a new Managed Skill or skip it; v0.1 does not merge or replace existing Skills."

> **Dev:** "How does project-level enablement work?"
> **Domain expert:** "Skill-kits physically deploys a Managed Skill into the Agent's Project Skill Directory, scans it as Project Scope, and toggles `SKILL.md` versus `SKILL.md.disabled`."

> **Dev:** "Can Skill-kits enable project skills for an Agent that only supports global skills?"
> **Domain expert:** "No. v0.1 project-level support requires a Project-supported Agent with a declared Project Skill Directory."

> **Dev:** "Which Agents are built in for v0.1?"
> **Domain expert:** "Codex uses `.agents/skills`, Claude Code uses `.claude/skills`, and Gemini CLI uses `.gemini/skills`; other Agents are Custom Agents."

> **Dev:** "Does v0.1 support YAML CLI output?"
> **Domain expert:** "No. v0.1 supports table for humans and JSON for scripts."

> **Dev:** "What is the v0.1 GUI navigation order?"
> **Domain expert:** "Dashboard, Skills, Agents, then Projects."

> **Dev:** "When disabling a project Skill, do we move or delete the whole directory?"
> **Domain expert:** "No. Disable only renames `SKILL.md` to `SKILL.md.disabled`; other files stay in place."

> **Dev:** "Does project deployment use symlinks by default?"
> **Domain expert:** "No. v0.1 copies Managed Skills into Project Skill Directories by default."

> **Dev:** "Is a newly deployed project Skill enabled immediately?"
> **Domain expert:** "Yes. Deploy creates an enabled Project Skill Deployment by default."

> **Dev:** "What happens if Deploy finds an existing same-name project directory?"
> **Domain expert:** "Deploy stops with Deploy Conflict unless that directory is already recorded as the selected Managed Skill deployment."

> **Dev:** "When a Managed Skill changes, do project copies update automatically?"
> **Domain expert:** "No. They become Outdated Deployments until the user explicitly redeploys them."

> **Dev:** "If a project copy was modified and the Managed Skill also changed, who wins?"
> **Domain expert:** "Nobody wins automatically. The user must keep the project copy, overwrite from managed, or promote the project copy to managed."

> **Dev:** "Does Promote To Managed replace the original Managed Skill?"
> **Domain expert:** "No. v0.1 creates a Managed Skill Fork and keeps the original Managed Skill unchanged."

> **Dev:** "Should the GUI start in project view?"
> **Domain expert:** "No. The GUI starts with Global Inventory, with a Scope Switcher in the lower-left for project views."

> **Dev:** "Does v0.1 need SQLite?"
> **Domain expert:** "No. Registry is plain TOML files under `~/.skill-kits/registry/`."

> **Dev:** "How do GUI and CLI avoid corrupting TOML registry files?"
> **Domain expert:** "All registry writes take Registry Lock and write through temp file plus atomic rename."

> **Dev:** "Does scan block risky skills in v0.1?"
> **Domain expert:** "No. Security Scan produces a Risk Report by default; Policy Enforcement is a future optional mode."

> **Dev:** "Does doctor inspect Agent auth, API keys, or network?"
> **Domain expert:** "No. Doctor Check stays lightweight and only checks Skill-kits-owned or recorded state."

> **Dev:** "Can doctor --fix repair everything automatically?"
> **Domain expert:** "No. Doctor Fix only performs low-risk cleanup and never deletes, overwrites, or promotes project copies."

> **Dev:** "Does the GUI scan every recent project at startup?"
> **Domain expert:** "No. Startup reads summaries only; Project Onboarding Scan performs one full scan when a project is first opened."

> **Dev:** "Does Project Onboarding Scan automatically import discovered project Skills?"
> **Domain expert:** "No. It shows discovered Skills and waits for Adopt All or per-Skill Project Adopt."

> **Dev:** "Should Adopt All stop at the first conflict?"
> **Domain expert:** "No. Adopt All imports non-conflicting Skills and reports conflicts separately as Partial Adopt Success."

> **Dev:** "Does Skill-kits automatically edit `.gitignore`?"
> **Domain expert:** "No. v0.1 only shows Git Ignore Guidance."

> **Dev:** "How does the GUI find projects?"
> **Domain expert:** "The user opens a project explicitly; Skill-kits adds it to Recent Projects and does not scan the whole machine."

> **Dev:** "How does the CLI know which project to manage?"
> **Domain expert:** "Project commands use the current directory by default and accept `--project <path>` for explicit scope."

> **Dev:** "Should project copy actions be called sync?"
> **Domain expert:** "No. Project actions are Deploy and Redeploy because they copy Managed Skills into project directories."

> **Dev:** "Does v0.1 download Skills from GitHub or a marketplace?"
> **Domain expert:** "No. v0.1 only supports Local Install, Global Agent Adopt, and Project Adopt."

> **Dev:** "How is the CLI divided?"
> **Domain expert:** "Global Commands manage Global Inventory; Project Commands manage project deployments."

> **Dev:** "What is the difference between global status and project status?"
> **Domain expert:** "Global Status summarizes inventory and system health; Project Status summarizes deployments inside one Project Scope."

> **Dev:** "What platform does v0.1 ship first?"
> **Domain expert:** "macOS single binary first; Windows and Linux follow after the core flow is stable."

> **Dev:** "Can we use the skill folder name as the registry key?"
> **Domain expert:** "No. Registry and markers use Skill ID; Skill Name is only human-facing and can change."

> **Dev:** "Where does the displayed skill name come from in v0.1?"
> **Domain expert:** "Use the imported directory name as Skill Name; parse SKILL.md only as optional metadata."

## Flagged ambiguities · 已澄清歧义

- "Delete" was ambiguous between deleting a managed copy and deleting the original source folder; resolved: GUI uses **Uninstall** for managed removal and does not delete **Source Path** in v0.1.
- "Uninstall" was ambiguous between deleting global inventory and deleting project copies; resolved: **Uninstall** affects Global Inventory only, while **Remove From Project** deletes a Project Skill Deployment.
- Remove From Project deletes one selected Project Skill Deployment, not the whole Project Skill Directory; resolved: deployments with Deployment Drift require **Forced Project Removal**.
- Agent directory ownership for global sync is out of scope; resolved: v0.1 does not write to real global Agent skill directories.
- "Sync existing skills" was ambiguous between distributing Managed Skills and importing historical Agent skills; resolved: **Adopt** imports existing Agent skills into Global Inventory, while **Deploy** copies Managed Skills into project scopes.
- Adopt source scope must be explicit; resolved: v0.1 supports **Global Agent Adopt** and **Project Adopt**, both import-only.
- Project Adopt does not only import inventory; resolved: it also records a **Deployment Baseline** for the existing Project Skill Deployment.
- Same-name Agent skills cannot be assumed identical; resolved: Adopt merges by content hash and creates an **Adoption Conflict** when hashes differ.
- Adoption Conflict resolution is minimal in v0.1; resolved: use **Conflict Import** or **Conflict Skip**, with no merge or replace existing.
- Global `sync --agent` is out of scope for v0.1; resolved: the global layer only manages **Global Inventory**, and project layer handles **Deploy**, **Redeploy**, enable, and disable.
- Skill identity cannot be based only on display name; resolved: registry and project deployment records reference **Skill ID**.
- Registry storage is file-first; resolved: v0.1 uses TOML files and does not use SQLite.
- Concurrent registry writes are serialized; resolved: mutations require **Registry Lock** and **Atomic Registry Write**.
- Security scanning is advisory in v0.1; resolved: **Security Scan** produces **Risk Report** and does not perform **Policy Enforcement** by default.
- Security scan is intentionally small; resolved: v0.1 uses **Minimum Scan Rules** for suspicious commands, token access, and network-fetch patterns.
- Doctor is intentionally lightweight; resolved: **Doctor Check** avoids Agent auth/network checks, and **Doctor Fix** only performs low-risk cleanup.
- GUI startup stays lightweight; resolved: startup reads summaries only, while **Project Onboarding Scan** and **Project Refresh Scan** operate on one selected project.
- Project onboarding discovery and import are separate; resolved: **Project Onboarding Scan** does not automatically perform **Adopt All**.
- Batch adoption is not all-or-nothing; resolved: **Adopt All** uses **Partial Adopt Success** and reports Adoption Conflicts separately.
- Git ignore behavior is advisory; resolved: v0.1 provides **Git Ignore Guidance** and does not mutate `.gitignore`.
- `SKILL.md` metadata is not required for naming in v0.1; resolved: directory name becomes **Skill Name**, while frontmatter or heading becomes optional **Skill Metadata**.
- Project-level enablement is not represented by global Agent directories; resolved: **Project Skill Deployments** inside Agent-specific project directories are the v0.1 mechanism.
- `Active Directory`, `Launcher`, and `Isolated Home` were considered for project injection and rejected; resolved: v0.1 does not use them as primary, fallback, or later-version concepts.
- Project-level support is limited to Agents with native project skill directories; resolved: v0.1 only supports **Project-supported Agents** for project enablement.
- Built-in v0.1 project directories are Codex `.agents/skills`, Claude Code `.claude/skills`, and Gemini CLI `.gemini/skills`; resolved: other Agents use **Custom Agent** path configuration.
- CLI output is intentionally narrow in v0.1; resolved: **CLI Output Format** supports `table` and `json`, not `yaml`.
- GUI sections are fixed for v0.1; resolved: **Dashboard View**, **Skills View**, **Agents View**, and **Projects View** in that order.
- Project disable does not move or delete the skill directory; resolved: only the **Skill Toggle File** is renamed, and simultaneous or missing toggle files create **Invalid Toggle State**.
- Project deployment does not use symlink as the default; resolved: **Project Skill Deployment** is a physical copy of a Managed Skill.
- New project deployments are enabled by default; resolved: **Deploy** creates a **Default Enabled Deployment** unless disabled deployment is added later.
- Deploy does not overwrite existing project directories; resolved: same-name unmanaged targets create **Deploy Conflict**.
- Project deployment linkage lives in Registry, not project sidecar markers; resolved: **Deployment Record** references Project Scope, Agent, Skill ID, and Deployment Baseline.
- Managed Skill updates do not automatically overwrite project copies; resolved: changed project copies become **Outdated Deployments** and may also contain **Deployment Drift** that must be resolved before redeploy.
- Redeploy does not auto-merge or auto-overwrite project modifications; resolved: **Deployment Drift** requires explicit **Keep Project Copy**, **Overwrite From Managed**, or **Promote To Managed**.
- Promote does not overwrite the original Managed Skill in v0.1; resolved: **Promote To Managed** creates a **Managed Skill Fork** by default.
- GUI default scope is global, not project; resolved: the app opens to **Global Inventory** and exposes project context through the **Scope Switcher**.
- Project discovery is explicit; resolved: v0.1 remembers **Recent Projects** opened by the user and does not scan the filesystem for projects.
- CLI project scope is explicit or cwd-based; resolved: **Project Command Scope** defaults to current directory and can be overridden with `--project`.
- Project commands do not use `sync-project`; resolved: **Deploy** and **Redeploy** name project copy operations.
- Online downloads are out of scope for v0.1; resolved: Skill sources are **Local Install**, **Global Agent Adopt**, and **Project Adopt** only.
- CLI commands are split by scope; resolved: **Global Commands** never deploy to projects, and **Project Commands** operate on one Project Scope.
- Status commands are scope-specific; resolved: **Global Status** reports inventory and health, while **Project Status** reports project deployment state.
- Agent recent activity is out of scope for v0.1; resolved: **Global Status** stays focused on inventory, Agent configuration, Recent Projects, registry health, and risk count last.
- Release is staged; resolved: v0.1 targets macOS single binary first, then Windows and Linux after core flow verification.
