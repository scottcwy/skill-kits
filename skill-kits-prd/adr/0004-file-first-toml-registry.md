# File-first TOML registry

Skill-kits v0.1 stores registry state as TOML files under `~/.skill-kits/registry/` instead of using SQLite. The registry records Global Inventory, known project deployments, recent projects, and lightweight metadata in files that are easy to inspect, back up, and repair. SQLite remains unnecessary for v0.1 because the data size is small and avoiding migrations, database locks, and recovery tooling supports the single-binary first goal.
