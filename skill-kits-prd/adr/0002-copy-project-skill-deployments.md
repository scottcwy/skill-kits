# Copy project skill deployments

Skill-kits v0.1 copies Managed Skills into Agent-specific project skill directories by default instead of using symlinks. Copy-based deployment keeps projects self-contained for CI, Docker, remote machines, and sandboxed Agents that may not read paths outside the workspace, while the global `~/.skill-kits/skills/` store remains the inventory source for dashboard management. The trade-off is that updates to a Managed Skill must be explicitly redeployed to project copies and local project edits can create drift.
