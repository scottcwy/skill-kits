# Workbench Grid Polish Design

Status: draft for user review.

## Purpose

This polish tranche improves the existing egui desktop interface without changing Skill-kits behavior. It focuses on the left sidebar, Dashboard alignment, and Skill view hover/read-only states.

The goal is a stricter traditional workbench: more generous navigation hit areas, cleaner grid alignment, and more reliable hover feedback.

## Non-Goals

- Do not rename the Skill view.
- Do not change native Skill enable/disable behavior.
- Do not introduce plugin management UI in this tranche.
- Do not replace egui with a web frontend.
- Do not introduce new runtime dependencies.
- Do not change CLI behavior.

## Design Register

Product UI. Design serves repeated operational use. The app should stay dense, monochrome, and quiet.

## Current Problems

### Sidebar

The sidebar is too compressed. Navigation items are readable, but they do not yet feel like first-class app sections. The current presentation relies on default selectable labels, which makes icon, text, active fill, and hover spacing less controlled.

### Dashboard

Dashboard sections use separators, but the divider and content alignment can feel uneven. Horizontal lines should align to the same content grid so sections read as one system.

Hover feedback also feels too soft. Rows should respond immediately and locally.

### Skill View

Read-only rows or badges can look like they are already hovered. Hover feedback can also make multiple regions feel active at once, especially when the row and badge backgrounds both shift too strongly.

The desired behavior is:

- read-only default is quiet
- hover uses one consistent row color
- selected uses a stronger but still monochrome color
- status badges stay readable without competing with row hover

## Proposed Design

### 1. Sidebar As Grid Tabs

Increase sidebar width from `204.0` to approximately `244.0`.

Render primary navigation with a custom row instead of plain `selectable_label`:

- row height: `36.0`
- icon column: `24.0`
- label starts on a fixed x position
- horizontal inset: `10.0`
- row radius: `5.0`
- active fill: `surface_3`
- hover fill: `surface_2`
- active text: `ink`
- inactive text: `ink_muted`
- icon color follows label color

Navigation order remains:

1. Dashboard
2. Skill
3. Agent
4. Project

Scope and Recent Projects remain below primary navigation, but should use the same left inset grid. Recent project rows may stay quieter than primary tabs.

### 2. Dashboard Divider Alignment

Create a small section rendering helper for Dashboard rows and groups:

- common left inset for section headings and rows
- dividers span from the same x origin to the same right edge
- section title, divider, and content rows share a vertical rhythm
- avoid mixed separators that start at different x positions

Dashboard should continue to avoid large metric cards. Use compact summary rows and section groups.

### 3. More Immediate Hover Feedback

Use explicit row painting where the current default widget hover is too soft or too broad.

Rules:

- hover fill: `surface_2`
- selected fill: `surface_3`
- focus outline: `focus`
- no secondary hover fill inside badges that makes both row and badge compete
- disabled/read-only text uses `ink_subtle` or `ink_tertiary`, not a hover-like background

### 4. Skill View Read-Only Treatment

Adjust status badge and row interaction so `Read-only` starts visually quiet:

- default badge background: `surface_1` or transparent-like dark surface
- badge border or text carries the read-only meaning
- read-only icon can use the lock icon already available through Font Awesome
- hover should not make the read-only badge look separately selected

When hovering a Skill row:

- the whole row gets `surface_2`
- status badges keep their compact badge shape
- selected row gets `surface_3`
- only the selected row should look persistent

## Implementation Notes

Likely files:

- `src/gui/mod.rs`
- `src/gui/dashboard.rs`
- `src/gui/skills.rs`
- `src/gui/icons.rs` only if icon helper names need minor extension
- `tests/gui_skeleton.rs` for pure mapping/layout helper tests

Suggested extraction:

- `render_sidebar_nav_item(...)`
- `render_sidebar_scope_item(...)`
- `render_dashboard_section(...)`
- `row_fill(selected, hovered, colors)`
- `status_badge_surface(value, row_hovered, selected, colors)`

Keep helpers small and local to GUI modules unless reuse becomes obvious.

## Testing

Add or update tests for pure helper behavior where possible:

- sidebar nav labels preserve `Dashboard`, `Skill`, `Agent`, `Project`
- hover/selected fill precedence is selected over hovered
- `Read-only` status maps to read-only icon and quiet status category
- Dashboard section helper preserves stable section ordering if represented in renderable data

Manual verification:

- launch `rtk cargo run -- --gui`
- inspect sidebar width and tab alignment
- hover Dashboard rows and Skill rows
- confirm Read-only default is not confused with hover
- confirm selected Skill row remains visually distinct from hover

## Acceptance Criteria

- Sidebar primary navigation feels like stable tab navigation with strict icon and label alignment.
- Dashboard dividers and section content align to one visible grid.
- Dashboard hover feedback is immediate and local to the hovered item.
- Skill row hover no longer makes unrelated left/right areas look active.
- Read-only default state is quieter than hover.
- Hover color is consistent across Skill rows.
- No business logic or CLI behavior changes.
- `cargo fmt --check` and `cargo test` pass.
