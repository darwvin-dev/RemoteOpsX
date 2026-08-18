# `main` Branch Protection Contract

`main` is the release-bearing branch. GitHub repository settings must enforce the policy below; this file documents the contract so the intended protections and required check names are reviewable with the codebase.

## Required merge policy

- Require a pull request before merging.
- Require all required status checks to pass before merging.
- Require the branch to be up to date before merge when GitHub can evaluate the merge queue/base safely.
- Block force pushes.
- Block branch deletion.
- Require conversation resolution before merge.
- Do not allow bypass of required checks for routine maintainer merges.

## Required status checks

At minimum, require:

```text
CI / linux
CI / macos
PR Title / conventional-title
```

If the GitHub UI displays only job names for the selected workflow contexts, select the corresponding `linux`, `macos`, and `conventional-title` checks produced by `.github/workflows/ci.yml` and `.github/workflows/pr-title.yml`.

## Release invariant

A change is eligible for `main` only when the exact PR head has passed the required checks. Tagged releases are created only from `main`, and `.github/workflows/release.yml` additionally verifies that the tag exactly matches the version shared by `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

## Administrative note

This Markdown file does not itself configure GitHub. The repository owner must apply the equivalent Branch Protection Rule or Repository Ruleset in GitHub settings. Any future automation that manages repository rules must preserve or strengthen this contract rather than silently weakening it.
