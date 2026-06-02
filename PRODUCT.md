# Skill-kits Product Context

## Register

product

## Product Purpose

Skill-kits is a local desktop utility for inspecting and managing agent-readable Skills across Codex, Claude Code, Gemini CLI, and custom Agents. It helps developers answer which Skills are visible to an Agent, which copies are enabled or disabled, which project deployments are stale or drifted, and which action is safe to take next.

The product is not a marketplace, launcher, or marketing surface. It is a workbench for local Skill operations, filesystem-backed state, and careful enablement boundaries.

## Users

Primary users are developers who work with multiple coding Agents and need repeatable control over Skill directories. They are comfortable with filesystem paths, Agent configuration, and project-local state, but they should not have to remember every Agent-specific directory convention.

Secondary users are maintainers validating Skill package behavior, plugin boundaries, and project adoption flows.

## Product Principles

- The Agent-readable filesystem is the operational truth for native Skill instances.
- Plugin packages are managed separately from native Skills.
- Every state-changing action should make its scope clear before it runs.
- Read-only and disabled states must be visually distinct.
- The app should favor inspection, alignment, and low-noise density over decorative personality.
- Dangerous actions require clear inline confirmation, not modal-first interruption.

## Tone

Quiet, precise, local-native, developer-grade, inspection-first. The interface should feel like a traditional desktop workbench with a strict grid, compact rows, stable panes, and restrained monochrome surfaces.

## Anti-References

Avoid marketing pages, hero metrics, SaaS card grids, neon dashboards, lavender or purple gradients, decorative glassmorphism, pill-heavy controls, and large rounded surfaces. Avoid wording that suggests global sync or launcher behavior when the product is operating on concrete Skill directories.

## Success Criteria

- A developer can identify the current view, scope, selected row, and available actions at a glance.
- Sidebar navigation feels stable and easy to hit without consuming excessive space.
- Dashboard sections align on a clear grid.
- Skill table hover and selected states are immediate, local to the row, and visually consistent.
- Read-only, disabled, invalid, and enabled states remain distinguishable without relying on color alone.
