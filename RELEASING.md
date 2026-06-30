# Releasing cedar-for-agents

This document walks you through releasing crates and packages from the `cedar-for-agents` repository. It covers Rust crates published to crates.io, WASM bindings to npm, and Python bindings to PyPI.

Throughout this guide, replace `<CRATE>` with the crate name (e.g., `mcp-tools-sdk` or `cedar-policy-mcp-schema-generator`) and `<MAJOR>.<MINOR>.<PATCH>` with the target version.

## Packages

| Package                                    | Registry                                                                     |
| ------------------------------------------ | ---------------------------------------------------------------------------- |
| `mcp-tools-sdk`                            | [crates.io](https://crates.io/crates/mcp-tools-sdk)                          |
| `cedar-policy-mcp-schema-generator`        | [crates.io](https://crates.io/crates/cedar-policy-mcp-schema-generator)      |
| `cedar-policy-mcp-schema-generator-wasm`   | [npm](https://www.npmjs.com/package/@cedar-policy/mcp-schema-generator-wasm) |
| `cedar-policy-mcp-schema-generator-python` | [PyPI](https://pypi.org/project/cedar-policy-mcp-schema-generator/)                 |

When releasing `cedar-policy-mcp-schema-generator`, you should also release the WASM and Python bindings at the same version.

## Prerequisites

You need a GitHub account with write access to this repository, a local fork and clone, and the Rust stable toolchain.

Most of the process (opening PRs, bumping versions, updating changelogs) only requires standard write access. A few key steps require elevated permissions:

- **Creating GitHub releases and tags** on the upstream repo requires write access to releases.
- **Creating release branches** on the upstream repo requires branch creation permissions.
- **Approving the publish deployment** requires access to the `release` environment (configured in repo Settings → Environments). You should have engaged with the repository owners if you don't have this.

```bash
rustup update stable && rustup default stable

# Optional: cargo-release is used to bump versions, but you can edit Cargo.toml by hand instead.
cargo install cargo-release

# Clone your fork (skip if already done)
git clone git@github.com:<your-username>/cedar-for-agents.git
cd cedar-for-agents
git remote add upstream git@github.com:cedar-policy/cedar-for-agents.git
```

---

## Branch Strategy

Development happens on `main`. Each minor release series gets a long-lived release branch named `release/<CRATE>-<MAJOR>.<MINOR>.x` (e.g., `release/mcp-tools-sdk-1.2.x`). This branch is created at the time of the minor release and serves as the base for all patch releases in that series.

You never commit directly to `main` or release branches. All changes go through short-lived feature/fix branches with PRs targeting the appropriate base.

---

## Phase 1: Preparation

### Check outstanding PRs

Review the [open PRs](https://github.com/cedar-policy/cedar-for-agents/pulls) and make sure everything intended for this release has been merged.

### Port changes to the release branch (PATCH releases only)

If this is a patch release, create a branch off the release branch, cherry-pick the relevant commits, and open a PR:

```bash
git fetch --all
git checkout -b patch/<CRATE>-<MAJOR>.<MINOR>.<PATCH> upstream/release/<CRATE>-<MAJOR>.<MINOR>.x
git cherry-pick <commit-hash>  # for each change to include
git push origin patch/<CRATE>-<MAJOR>.<MINOR>.<PATCH>
```

Open a PR targeting `release/<CRATE>-<MAJOR>.<MINOR>.x`.

### Bump the crate version

Create a branch from the appropriate base:

```bash
git fetch --all

# For a MINOR release, branch from main:
git checkout -b update-versions/<CRATE>-<MAJOR>.<MINOR>.<PATCH> upstream/main

# For a PATCH release, branch from the release branch:
git checkout -b update-versions/<CRATE>-<MAJOR>.<MINOR>.<PATCH> upstream/release/<CRATE>-<MAJOR>.<MINOR>.x
```

Run `cargo release version` from the workspace root to bump the version:

```bash
cd rust
cargo release version <minor|patch> --package <CRATE> --execute
```

If you are releasing `cedar-policy-mcp-schema-generator`, also bump the WASM and Python bindings to the same version:

```bash
cargo release version <minor|patch> --package cedar-policy-mcp-schema-generator-wasm --execute
cargo release version <minor|patch> --package cedar-policy-mcp-schema-generator-python --execute
```

> We may automate the version bump with a script or GitHub Action in the future.

Stage the manifest changes and push:

```bash
find . -name "Cargo.toml" -exec git add {} +
find . -name "Cargo.lock" -exec git add {} +
git diff --staged --name-only  # verify only manifest/lock files changed
git commit --signoff -m "Bump <CRATE> version to <MAJOR>.<MINOR>.<PATCH>"
git push origin update-versions/<CRATE>-<MAJOR>.<MINOR>.<PATCH>
```

Open a PR targeting `main` (minor) or `release/<CRATE>-<MAJOR>.<MINOR>.x` (patch).

### Update the changelog

```bash
git fetch --all
git checkout -b update-changelog/<CRATE>-<MAJOR>.<MINOR>.<PATCH> upstream/main
```

Edit `rust/<CRATE>/CHANGELOG.md`:

For a **minor release**, replace the `## [Unreleased]` heading with:

```markdown
## [Unreleased]

## [<MAJOR>.<MINOR>.<PATCH>] - Coming soon
```

For a patch release, add a new section above the previous patch entry for this minor version, with descriptions of the cherry-picked changes (bug fixes, security patches, etc.) that make up this patch release.

Then commit and push:

```bash
git add rust/<CRATE>/CHANGELOG.md
git diff --staged --name-only  # verify only the changelog changed
git commit --signoff -m "Create <CRATE> changelog entry for <MAJOR>.<MINOR>.<PATCH>"
git push origin update-changelog/<CRATE>-<MAJOR>.<MINOR>.<PATCH>
```

Open a PR targeting `main`. The changelog on `main` serves as the canonical history for all releases regardless of which branch they were published from.

### Create the release branch (MINOR releases only)

After all preparation PRs against `main` are merged:

1. Go to the [branches page](https://github.com/cedar-policy/cedar-for-agents/branches).
2. Click "New branch".
3. Name it `release/<CRATE>-<MAJOR>.<MINOR>.x` with source `main`.

### Update path dependencies in the release branch (MINOR releases only, skip for `mcp-tools-sdk`)

The release branch needs versioned dependencies instead of path-only references so that `cargo publish` works in isolation.

```bash
git fetch --all
git checkout -b update-dependencies/<CRATE>-<MAJOR>.<MINOR>.<PATCH> upstream/release/<CRATE>-<MAJOR>.<MINOR>.x
```

In `rust/<CRATE>/Cargo.toml`, change path dependencies to versioned:

```toml
# Before:
mcp-tools-sdk = { path = "../mcp-tools-sdk", version = "*" }
# After:
mcp-tools-sdk = { version = "x.y.z" }
```

For `cedar-policy-mcp-schema-generator`, also update the WASM and Python binding Cargo.toml files (`rust/cedar-policy-mcp-schema-generator-wasm/Cargo.toml` and `rust/cedar-policy-mcp-schema-generator-python/Cargo.toml`) the same way.

```bash
git add rust/cedar-policy-mcp-schema-generator/Cargo.toml
git add rust/cedar-policy-mcp-schema-generator-wasm/Cargo.toml
git add rust/cedar-policy-mcp-schema-generator-python/Cargo.toml
git diff --staged --name-only  # verify
git commit --signoff -m "Pin <CRATE> dependencies for release"
git push origin update-dependencies/<CRATE>-<MAJOR>.<MINOR>.<PATCH>
```

Open a PR targeting `release/<CRATE>-<MAJOR>.<MINOR>.x`.

### Wait for all PRs to be approved and merged before continuing.

### Verify CI on the release branch

Before moving to Phase 2, confirm that CI is green on the release branch HEAD:

1. Go to the [Actions tab](https://github.com/cedar-policy/cedar-for-agents/actions) and check the latest run of the **"CI"** workflow (`ci.yaml`) on `release/<CRATE>-<MAJOR>.<MINOR>.x`.
2. If CI hasn't run automatically, push an empty commit or re-run the workflow manually.

Do not proceed to publishing if tests are failing.

---

## Phase 2: Publish Rust crate to crates.io

If you need to publish a Rust crates, follow these steps. If you are publishing only bindings, follow the steps described in the appropriate sections (we do not publish the bindings as an independent crate).
Typically, you will publish `mcp-tools-sdk`, then `cedar-policy-mcp-schema-generator` and then the bindings for `cedar-policy-mcp-schema-generator`.

### Create a GitHub release

> In the future, we may automate GitHub release creation with a script or GitHub Action.

1. Go to [Releases](https://github.com/cedar-policy/cedar-for-agents/releases) → "Create a new release".
2. Create a new tag: `<CRATE>-v<MAJOR>.<MINOR>.<PATCH>`.
3. Target: `release/<CRATE>-<MAJOR>.<MINOR>.x`.
4. Set the previous tag to the prior release of this crate.
5. Title: `<CRATE>-v<MAJOR>.<MINOR>.<PATCH>`.
6. Body: paste the changelog entry for this version.
7. Uncheck "Set as the latest release" if this isn't the highest version per semver.
8. Click "Publish release".

Verify the release exists at `https://github.com/cedar-policy/cedar-for-agents/releases/tag/<CRATE>-v<MAJOR>.<MINOR>.<PATCH>`.

### Run the publish workflow

1. Go to ["Publish Rust crate to crates.io"](https://github.com/cedar-policy/cedar-for-agents/actions/workflows/publish.yml).
2. Click "Run workflow". Make sure the branch selector says `main` (the workflow enforces this).
3. Select the crate from the dropdown.
4. Enter the tag: `<CRATE>-v<MAJOR>.<MINOR>.<PATCH>`.
5. Click "Run workflow".
6. Wait for the "validate" job to pass.
7. Approve deployment to the "release" environment when prompted.
8. Monitor until the run completes successfully.

### Validate the crates.io release

1. Navigate to `https://crates.io/crates/<CRATE>/<MAJOR>.<MINOR>.<PATCH>`.
2. Verify metadata and ownership look correct.
3. Check `https://docs.rs/crate/<CRATE>/<MAJOR>.<MINOR>.<PATCH>/source/` to make sure no unexpected files were published.

---

## Phase 3: Publish WASM bindings to npm

> Skip this phase for `mcp-tools-sdk` releases.

### Create a GitHub release for the WASM package

1. Go to [Releases](https://github.com/cedar-policy/cedar-for-agents/releases) → "Create a new release".
2. Create a new tag: `cedar-policy-mcp-schema-generator-wasm-v<MAJOR>.<MINOR>.<PATCH>`.
3. Target: `release/cedar-policy-mcp-schema-generator-<MAJOR>.<MINOR>.x`.
4. Title: `cedar-policy-mcp-schema-generator-wasm-v<MAJOR>.<MINOR>.<PATCH>`.
5. Click "Publish release".

### Run the npm publish workflow

1. Go to ["Publish MCP schema generator WASM to npm"](https://github.com/cedar-policy/cedar-for-agents/actions/workflows/publish_wasm_npm.yml).
2. Click "Run workflow" (branch selector must be `main`).
3. Enter the tag: `cedar-policy-mcp-schema-generator-wasm-v<MAJOR>.<MINOR>.<PATCH>`.
4. Click "Run workflow".
5. Wait for validation, approve deployment, and monitor to completion.

### Validate the npm release

1. Check `https://www.npmjs.com/package/@cedar-policy/mcp-schema-generator-wasm`.
2. Verify the version:
   ```bash
   npm view @cedar-policy/mcp-schema-generator-wasm version
   ```
3. Test installation:
   ```bash
   npm install @cedar-policy/mcp-schema-generator-wasm@<MAJOR>.<MINOR>.<PATCH>
   ```

---

## Phase 4: Publish Python bindings to PyPI

> Skip this phase for `mcp-tools-sdk` releases.

### Create a GitHub release for the Python package

1. Go to [Releases](https://github.com/cedar-policy/cedar-for-agents/releases) → "Create a new release".
2. Create a new tag: `cedar-policy-mcp-schema-generator-python-v<MAJOR>.<MINOR>.<PATCH>`.
3. Target: `release/cedar-policy-mcp-schema-generator-<MAJOR>.<MINOR>.x`.
4. Title: `cedar-policy-mcp-schema-generator-python-v<MAJOR>.<MINOR>.<PATCH>`.
5. Click "Publish release".

### Run the PyPI publish workflow

1. Go to ["Publish Python bindings to PyPI"](https://github.com/cedar-policy/cedar-for-agents/actions/workflows/publish_python.yml).
2. Click "Run workflow" (branch selector must be `main`).
3. Enter the tag: `cedar-policy-mcp-schema-generator-python-v<MAJOR>.<MINOR>.<PATCH>`.
4. Click "Run workflow".
5. Wait for validation, approve deployment, and monitor to completion.

### Validate the PyPI release

1. Check `https://pypi.org/project/cedar-policy-mcp-schema-generator/<MAJOR>.<MINOR>.<PATCH>/`.
2. Test installation:
   ```bash
   pip install cedar-policy-mcp-schema-generator==<MAJOR>.<MINOR>.<PATCH>
   python -c "from cedar_policy_mcp_schema_generator import generate_schema; print('OK')"
   ```

---

## Phase 5: Finalize

After everything is published, update the changelog to reflect the actual release date.

```bash
git fetch --all
git checkout -b finalize-changelog/<CRATE>-<MAJOR>.<MINOR>.<PATCH> upstream/main
```

In `rust/<CRATE>/CHANGELOG.md`, replace `Coming soon` with today's date:

```markdown
## [<MAJOR>.<MINOR>.<PATCH>] - YYYY-MM-DD
```

Then commit and push:

```bash
git add rust/<CRATE>/CHANGELOG.md
git diff --staged --name-only  # verify
git commit --signoff -m "Finalize <CRATE> changelog for <MAJOR>.<MINOR>.<PATCH>"
git push origin finalize-changelog/<CRATE>-<MAJOR>.<MINOR>.<PATCH>
```

Open a PR targeting `main`.

---

## Rollback

Published releases cannot be fully rolled back. We follow the [official crates.io guidance on yanking](https://doc.rust-lang.org/nightly/cargo/commands/cargo-yank.html#when-to-yank), which generally advises against it.
In the unusual case where a published release needs to be yanked, you should first release the corrective patch (i.e. semver-compatible replacement of the release to be yanked), and only then yank the release.

If a release contains a critical defect:

1. Prepare and publish a corrective patch release following the process above.
2. Once the patch is available, yank the defective version:
   - **crates.io**: `cargo yank --vers <VERSION> <CRATE>`
   - **npm**: `npm deprecate @cedar-policy/mcp-schema-generator-wasm@<VERSION> "reason"`
   - **PyPI**: Yank the version via the PyPI web UI.

---

## Environment Protection

All publish workflows are gated by a protected GitHub environment (`release`) that requires manual approval before publishing proceeds. Contact a repository maintainer if you need approval access.

## Security Note

Before every `git push`, verify staged files with `git diff --staged --name-only` to ensure no unintended files are included. Pushes to GitHub are public and effectively irreversible.
