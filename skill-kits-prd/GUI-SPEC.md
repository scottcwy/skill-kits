# Skill-kits GUI Spec v0.1

## Design Position

Skill-kits uses a Linear-inspired monochrome workbench style. The interface should feel like a precise local developer tool: quiet, dense, fast to scan, and trustworthy under repeated daily use.

This is not a marketing canvas. The GUI should borrow Linear's product qualities, not its website formula:

- Use black, white, and gray as the brand system.
- Use surface levels and hairline borders for hierarchy.
- Use compact lists, tables, inspectors, and toolbars as the main UI language.
- Use status color only when it carries operational meaning.
- Avoid hero sections, product screenshot framing, lavender brand accents, gradient atmosphere, and SaaS card grids.

## Scene

A developer opens Skill-kits on macOS to answer concrete questions:

- Which Skills are in my global inventory?
- Which project am I looking at?
- Which Agent directories are configured?
- Which Skills are deployed, disabled, outdated, drifted, or invalid?
- What action is safe to take next?

The UI should feel calm enough for long sessions and sharp enough for quick status checks.

## Visual Direction

Name: **Monochrome Local Workbench**

Keywords:

- quiet
- precise
- dense
- local-native
- developer-grade
- inspection-first
- low-noise

Use Linear as an inspiration for hierarchy and restraint, but do not clone Linear's brand.

## Color System

### Strategy

Brand color is monochrome. Black, white, and gray carry the identity.

Status colors are semantic, not brand colors. They should be muted and rare.

Do not use lavender, purple, blue-purple gradients, or saturated chromatic accents as a primary brand signal.

### Dark Theme Tokens

Dark theme is the v0.1 default.

| Token | Hex | Use |
| --- | --- | --- |
| `canvas` | `#08090b` | App background |
| `surface_1` | `#101114` | Sidebar and base panels |
| `surface_2` | `#17191d` | Main raised panels, rows on hover |
| `surface_3` | `#202227` | Selected rows, popovers, active controls |
| `surface_4` | `#2a2d33` | Strong focus surfaces |
| `hairline` | `#25272d` | Default 1px dividers and borders |
| `hairline_strong` | `#363942` | Focused borders and active outlines |
| `ink` | `#f2f3f3` | Primary text |
| `ink_muted` | `#b9bec7` | Secondary text |
| `ink_subtle` | `#858b96` | Metadata, paths, helper text |
| `ink_tertiary` | `#5f6570` | Disabled text |
| `inverse_ink` | `#111216` | Text on light controls |

### Semantic Tokens

| Token | Hex | Use |
| --- | --- | --- |
| `success` | `#67a878` | Enabled, healthy |
| `warning` | `#c5a365` | Outdated, caution |
| `danger` | `#d06b6b` | Destructive action, high risk |
| `info` | `#9ea4ad` | Neutral informational state |
| `focus` | `#e4e6eb` | Keyboard focus ring and selected outline |

Rules:

- `focus` is near-white or cool gray, not lavender.
- Status colors must not fill large surfaces.
- Most badges should use gray surfaces with colored text or a tiny dot.
- Destructive actions use muted red text until confirmation, not bright filled red buttons.

### Light Theme

v0.1 ships dark theme only. If light theme is added later, it should invert the same monochrome structure rather than introduce a new brand color.

## Typography

Use platform-native fonts. Skill-kits is a desktop utility, so native text rendering matters more than branded typography. v0.1 does not bundle custom fonts.

Recommended stack:

- UI text: `-apple-system`, `BlinkMacSystemFont`, `SF Pro`, `Segoe UI`, `system-ui`
- Mono: `JetBrains Mono`, `SF Mono`, `ui-monospace`, `Menlo`

Use mono only for:

- paths
- hashes
- Skill IDs
- commands
- registry snippets

Do not use aggressive negative letter spacing in the product UI. Keep letter spacing at `0`.

### Type Scale

| Token | Size | Weight | Use |
| --- | --- | --- | --- |
| `title` | 20 | 600 | Page title |
| `section` | 15 | 600 | Section headings |
| `body` | 13 | 400 | Default table and panel text |
| `body_strong` | 13 | 500 | Names, selected values |
| `caption` | 12 | 400 | Metadata, timestamps, helper text |
| `mono` | 12 | 400 | Paths, IDs, hashes |
| `button` | 13 | 500 | Button labels |

## Layout System

Use a stable desktop app shell:

```text
┌────────────────────────────────────────────────────────────┐
│ Top bar: scope, project, quick actions, status             │
├──────────────┬──────────────────────────────┬──────────────┤
│ Sidebar      │ Main list / table             │ Inspector    │
│ navigation   │                              │ details      │
└──────────────┴──────────────────────────────┴──────────────┘
```

### App Shell

- Sidebar width: 180 to 220px.
- Inspector width: 320 to 380px.
- Main content owns remaining width.
- Minimum usable desktop width: 960px.
- Use split panes, not nested cards.

### Navigation

Order:

1. Dashboard
2. Skills
3. Agents
4. Projects

The Scope Switcher sits at the lower-left or lower sidebar area. It switches between Global Inventory and Recent Projects.

### Density

Default density should be compact:

- Table row height: 32 to 36px.
- Toolbar height: 40px.
- Sidebar item height: 32px.
- Inspector section spacing: 16px.
- Control padding: 6px vertical, 10px horizontal.

Use space to separate meaning, not to make the UI feel luxurious.

## Surface and Borders

Depth is carried by surface levels and hairlines:

- App background: `canvas`.
- Sidebar: `surface_1`.
- Main panel: `canvas` or `surface_1`, depending on density.
- Hover row: `surface_2`.
- Selected row: `surface_3`.
- Inspector: `surface_1` with `hairline` separator.
- Popovers: `surface_3` with `hairline_strong`.

Do not use drop shadows as a primary depth mechanism. If egui rendering makes shadows useful for popovers, keep them extremely subtle.

## Shape

| Token | Radius | Use |
| --- | --- | --- |
| `xs` | 3px | small badges |
| `sm` | 5px | table selections, inline tags |
| `md` | 6px | buttons, inputs |
| `lg` | 8px | popovers, inspector groups |

Avoid large rounded cards. The app should feel crisp, not pillowy.

## Components

### Buttons

Primary actions are monochrome:

- Background: `ink`
- Text: `inverse_ink`
- Radius: `md`
- Height: 28 to 32px

Secondary buttons:

- Background: `surface_2`
- Text: `ink`
- Border: `hairline`

Tertiary buttons:

- Background: transparent
- Text: `ink_muted`
- Hover: `surface_2`

Destructive actions:

- Default text: `danger`
- Confirmation state may use muted danger fill
- Never place a large bright red button in a normal toolbar

### Tables and Lists

Tables are the core component.

Rules:

- First column is the human name.
- Status columns use compact badges.
- Paths and hashes use mono.
- Hover reveals row actions when useful.
- Selected row opens or updates the inspector.
- Empty cells use `-`, not blank space.

### Inspector

The right inspector shows details for the selected Skill, Agent, or Project deployment.

Common sections:

- Summary
- Source or deployment path
- Status
- Risk findings
- Actions
- Registry metadata

The inspector should avoid modal-first flows. Inline confirmation areas are preferred for destructive actions.

### Badges

Badges are small, quiet, and consistent.

| State | Visual |
| --- | --- |
| Enabled | gray badge with success dot |
| Disabled | gray badge, muted text |
| Outdated | gray badge with warning dot |
| Drift | warning text or warning dot |
| Invalid | danger text or danger dot |
| Risk high | danger badge |
| Risk warning | warning badge |

Avoid full-color pill floods. Status should be scannable, not loud.

### Inputs

Inputs use `surface_1`, `hairline`, and `focus` outline.

Path inputs should support:

- browse button
- reveal button
- validation message
- mono rendering for paths

### Empty States

Empty states are concise and action-led.

Examples:

- No Skills: `Install a local Skill or adopt existing Agent Skills.`
- No Recent Projects: `Open a project to scan project-level Skills.`
- No deployments: `Deploy a Managed Skill to this Agent project directory.`

Do not add explanatory marketing copy.

## Page Specifications

### Dashboard

Purpose: global overview.

Main content:

- Managed Skills count.
- Agent configuration summary.
- Recent Projects summary.
- Registry and lock health.
- Risk count last.

Layout:

- Compact summary strip at top.
- Recent Projects list below.
- Health panel in inspector or right rail.

Avoid large metric cards. Use rows and compact panels.

### Skills

Purpose: manage Global Inventory.

Main table columns:

- Skill
- Source
- Risk
- Project deployments
- Updated

Inspector:

- Skill name and Skill ID.
- Managed path.
- Source path.
- Metadata from `SKILL.md`.
- Risk findings.
- Deployment references in Recent Projects.
- Actions: scan, uninstall.

Uninstall copy:

- `Uninstall removes this Skill from Global Inventory. Project copies are not deleted.`

### Agents

Purpose: configure Agent project skill directories.

Rows:

- Codex
- Claude Code
- Gemini CLI
- Custom Agents

Each row shows:

- Agent label.
- Project skill directories.
- Enabled state.
- Validation state.

Actions:

- edit path
- reset to default
- add custom Agent
- remove custom Agent

Do not show global Agent sync settings.

### Projects

Purpose: manage one Project Scope.

Top area:

- project path
- refresh
- onboarding scan state
- Git ignore guidance

Agent grouping:

- Codex deployments
- Claude Code deployments
- Gemini deployments
- Custom Agent deployments

Deployment table columns:

- Skill
- Agent
- Toggle
- Outdated
- Drift
- Missing managed source
- Risk
- Path

Inspector actions:

- enable
- disable
- redeploy
- overwrite from managed
- promote to managed
- remove from project

Onboarding:

- Show discovered unmanaged project Skills.
- Offer Adopt all.
- Show partial success as `8 adopted, 2 conflicts`.
- Conflict actions: import as new, skip.

## Interaction Rules

### Deploy

Default deploy creates an enabled project deployment.

If target directory exists and is unmanaged:

- Show Deploy Conflict.
- Do not overwrite.
- Offer explanation and next steps: adopt, remove manually, or choose another Skill.

### Enable and Disable

Enable/disable only renames the toggle file.

Invalid toggle state:

- Both `SKILL.md` and `SKILL.md.disabled` exist.
- Both are missing.

Show invalid state as a blocking row status with repair guidance.

### Missing Managed Source

A deployment has missing managed source when the project copy still exists but the source Managed Skill no longer exists in Global Inventory.

Show this as `Missing managed source`.

Available actions:

- Promote to managed.
- Remove from project.

Do not auto-fix it.

### Redeploy

If drift exists, block default redeploy.

Show inline choices:

- Keep project copy.
- Overwrite from managed.
- Promote to managed.

Do not use a modal as the first interaction.

### Remove From Project

Remove deletes only the selected deployment directory.

If drift exists:

- GUI requires confirmation.
- Copy: `This project copy has local changes. Removing it deletes only this deployed Skill, not the Agent skill root.`

## Motion

Use minimal motion:

- row hover
- selection change
- inspector content fade or slide
- small progress indicator during scan/copy

Do not animate layout-heavy transitions. No bounce, elastic, or decorative motion.

## Accessibility

Requirements:

- Keyboard navigation for sidebar, tables, toolbar actions, and inspector actions.
- Visible focus state using `focus`.
- Do not rely on color alone for status. Pair color with label or icon/dot.
- Text contrast must remain high on dark surfaces.
- Destructive actions require clear copy and confirmation when data can be lost.

## egui Notes

egui does not have CSS. Implement tokens as Rust structs:

```rust
pub struct UiColors {
    pub canvas: Color32,
    pub surface_1: Color32,
    pub surface_2: Color32,
    pub surface_3: Color32,
    pub surface_4: Color32,
    pub hairline: Color32,
    pub hairline_strong: Color32,
    pub ink: Color32,
    pub ink_muted: Color32,
    pub ink_subtle: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub focus: Color32,
}
```

Use egui visuals to set:

- dark mode
- panel fill
- faint widget background
- selection fill
- hyperlink color as monochrome or muted gray, not lavender
- window rounding and widget rounding

Prefer custom row rendering for the main tables if standard widgets cannot reach the desired density.

## Bans

Do not use:

- lavender as brand color
- purple or blue gradients
- atmospheric glows
- glassmorphism
- hero sections
- large marketing metric cards
- repeated icon cards
- pill-shaped primary buttons
- nested cards
- global sync settings
- launcher or active directory language

## Locked Decisions

- v0.1 ships dark theme only.
- v0.1 uses platform system fonts and does not bundle custom fonts.
- `.gitignore` behavior is guidance only. The app does not edit `.gitignore` and does not force a commit-or-ignore recommendation.
