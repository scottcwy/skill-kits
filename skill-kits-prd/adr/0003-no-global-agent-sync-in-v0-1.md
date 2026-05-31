# No global Agent sync in v0.1

Skill-kits v0.1 does not implement `sync --agent` or write to real global Agent skills directories such as `~/.codex/skills`. The global layer manages the Global Inventory only, while Agent usage happens through project-scoped Deploy, Redeploy, enable, and disable operations. This avoids symlink or marker ownership logic in global directories and keeps the v0.1 product centered on project-native skill deployment.
