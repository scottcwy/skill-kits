# Skill-kits v0.1.0 Release Notes

Release target: macOS arm64 single binary.

Release commit: `6a41504`
Release date: 2026-06-01
Build environment: macOS 15.7.3 arm64, cargo 1.95.0, rustc 1.95.0

## What Is Included

- Native Rust CLI and GUI in one `skill-kits` binary.
- Global Skill inventory install, uninstall, list, status, scan, and doctor flows.
- Project deploy, enable, disable, redeploy, overwrite, promote, remove, status, and onboarding adopt flows.
- GUI Dashboard, Skills, Agents, and Projects views with scoped project management.
- GUI local Skill install, all-enabled-Agent global Skill adopt, global uninstall confirmation, project drift remove confirmation, explicit deploy target, discovered project Skill list, per-Skill project adopt, and background action execution.
- Release smoke fixture covering install, project adopt, deploy, disabled, drift, outdated, missing managed source, and GUI-model acceptance states.

## Install

Unpack the archive and put the binary on your `PATH`:

```bash
tar -xzf skill-kits-v0.1.0-macos-arm64.tar.gz
install -m 0755 skill-kits-v0.1.0-macos-arm64/skill-kits /usr/local/bin/skill-kits
```

Run:

```bash
skill-kits --version
skill-kits status
skill-kits --gui
```

## Signing And Notarization

v0.1.0 is unsigned and not notarized. Treat it as a local/internal release build unless a later distribution step signs and notarizes the binary.

## Known Limits

- No remote registry or network install flow.
- No automatic edits to project `.gitignore`.
- Path inputs are text-first; native browse/reveal controls are deferred.
- The release tag should be created only after this branch is merged to the release branch.

## Acceptance Evidence

Final acceptance should be recorded after the final release commit with:

- `rtk cargo test`
- `rtk cargo clippy --all-targets --all-features -- -D warnings`
- `rtk cargo fmt --check`
- `rtk git diff --check`
- `rtk cargo build --release`
- RELEASE.md seeded runtime smoke and package checksum
