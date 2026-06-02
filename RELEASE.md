# Skill-kits v0.1 macOS single binary checklist

This checklist is for a local macOS release build of the `skill-kits` binary.

## Verify

Run from the repository root:

```bash
rtk /opt/homebrew/bin/cargo fmt --all --check
rtk /opt/homebrew/bin/cargo clippy --all-targets --all-features -- -D warnings
rtk /opt/homebrew/bin/cargo test
```

## Build

```bash
rtk /opt/homebrew/bin/cargo build --release
```

The release binary is:

```text
target/release/skill-kits
```

## Runtime

The v0.1 macOS release is a single Rust binary. It must not require a Node.js or
Python runtime to list, install, deploy, adopt, scan, run doctor checks, or open
the native GUI.

## GUI Smoke

Run GUI smoke checks with an isolated home so local release testing does not
mutate a real `~/.skill-kits` directory:

```bash
rtk rm -rf /tmp/skill-kits-smoke-home /tmp/skill-kits-smoke-project
rtk mkdir -p /tmp/skill-kits-smoke-home
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits --gui
```

### Seeded GUI Smoke

Use the release smoke fixture to create a deterministic GUI acceptance state.
All commands use the isolated home above. Re-run this baseline seed block before
each destructive state variation below.

```bash
rtk rm -rf /tmp/skill-kits-smoke-home /tmp/skill-kits-smoke-project
rtk mkdir -p /tmp/skill-kits-smoke-home/.skill-kits
rtk cp tests/fixtures/release-smoke/config.toml /tmp/skill-kits-smoke-home/.skill-kits/config.toml
rtk cp -R tests/fixtures/release-smoke/project /tmp/skill-kits-smoke-project
rtk perl -0pi -e 's#path = "/tmp/skill-kits-smoke-project"#path = "'$(cd /tmp/skill-kits-smoke-project && pwd -P)'"#' /tmp/skill-kits-smoke-home/.skill-kits/config.toml
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits install local tests/fixtures/release-smoke/source-skill
rtk mkdir -p /tmp/skill-kits-smoke-home/.codex/skills/global-seed
rtk cp tests/fixtures/release-smoke/project/.agents/skills/project-seed/SKILL.md /tmp/skill-kits-smoke-home/.codex/skills/global-seed/SKILL.md
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits adopt --global-agent codex
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project adopt project-seed --agent codex --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project deploy source-skill --agent codex --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project disable source-skill --agent codex --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project enable source-skill --agent codex --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project redeploy source-skill --agent codex --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project disable source-skill --agent codex --project /tmp/skill-kits-smoke-project
rtk printf 'release smoke drift\n' > /tmp/skill-kits-smoke-project/.agents/skills/source-skill/local-edit.txt
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project status --project /tmp/skill-kits-smoke-project
```

To seed outdated state, replace the first managed hash in the fixture-seeded
registry:

```bash
rtk perl -0pi -e 's/(name = "source-skill"(?s:.*?content_hash = )")[^"]+"/$1release-smoke-new-managed-hash"/' /tmp/skill-kits-smoke-home/.skill-kits/registry/skills.toml
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project status --project /tmp/skill-kits-smoke-project
```

To seed invalid toggle state, create both toggle files for the deployment:

```bash
rtk cp /tmp/skill-kits-smoke-project/.agents/skills/source-skill/SKILL.md.disabled /tmp/skill-kits-smoke-project/.agents/skills/source-skill/SKILL.md
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project status --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits doctor
```

To seed missing managed source, uninstall the managed Skill while leaving the
project deployment in place:

```bash
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits uninstall source-skill
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project status --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits doctor
```

After seeding invalid toggle or missing managed source, `doctor` should report
errors and exit `5`. Re-run the baseline seed block before checking a clean
runtime smoke.

Then open the GUI against the seeded home:

```bash
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits --gui
```

Manual acceptance:

- Dashboard opens first in Global Inventory scope with Dashboard, Skills,
  Agents, and Projects navigation in that order.
- The app uses the dark monochrome workbench style, compact tables, a sidebar,
  and a right inspector without marketing panels or gradient/card-heavy UI.
- Empty Skills and Projects states explain the next safe action.
- Clicking Adopt Agent Skills scans Global Skill Directories for every enabled
  Agent, imports detectable non-conflicting Skills into Global Inventory,
  reports partial success/conflicts/failures, and does not modify Agent source
  directories.
- Clicking Refresh, Adopt all, Import as new, Skip, Scan, Deploy, Enable,
  Disable, Redeploy, Overwrite, Promote, Remove, and Uninstall either completes
  the action or shows a visible success/error message.
- Destructive remove with local drift requires confirmation and states that only
  the deployed Skill directory is removed.
- Global uninstall requires confirmation and states that source files and
  project deployments are not deleted.
- The top bar shows the next queued action, not just a pending count, when work
  is queued.
- Projects shows outdated, drift, invalid toggle, and missing managed source
  states with only safe actions available for missing managed source.
- Agents view does not imply global Agent sync or unsupported v0.1 runtime
  behavior.

## Runtime Smoke

Confirm the release binary can run core CLI paths without Node.js or Python.
Run this after the baseline seed block and before destructive state variations:

```bash
rtk target/release/skill-kits --version
rtk otool -L target/release/skill-kits
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits list
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits list --format json
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits status
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits status --format json
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project status --project /tmp/skill-kits-smoke-project
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits project status --project /tmp/skill-kits-smoke-project --format json
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits scan --format json
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits doctor
rtk env HOME=/tmp/skill-kits-smoke-home target/release/skill-kits doctor --fix
```

## Package

Create a local macOS archive and checksum after the runtime smoke passes:

```bash
rtk rm -rf dist
rtk mkdir -p dist/skill-kits-v0.1.0-macos-arm64
rtk cp target/release/skill-kits dist/skill-kits-v0.1.0-macos-arm64/skill-kits
rtk cp RELEASE.md dist/skill-kits-v0.1.0-macos-arm64/RELEASE.md
rtk cp RELEASE_NOTES-v0.1.0.md dist/skill-kits-v0.1.0-macos-arm64/RELEASE_NOTES-v0.1.0.md
rtk tar -C dist -czf dist/skill-kits-v0.1.0-macos-arm64.tar.gz skill-kits-v0.1.0-macos-arm64
rtk shasum -a 256 dist/skill-kits-v0.1.0-macos-arm64.tar.gz > dist/skill-kits-v0.1.0-macos-arm64.tar.gz.sha256
```

v0.1 is not signed or notarized. `RELEASE_NOTES-v0.1.0.md` records that
decision for local/internal release distribution.

## Tag

Only tag after all checks above pass and the work is merged to the release
branch:

```bash
rtk git status --short
rtk git tag -a v0.1.0 -m "Skill-kits v0.1.0"
```
