# Workflows

This directory holds the GitHub Actions workflows for the repository.

## Required checks

Before a PR can merge, branch protection must require these checks to pass:

- `cargo check --workspace --tests`
- `cargo build --target wasm32v1-none --release --workspace`

## Guard scripts

The `scripts/` directory contains guards that run on every PR to catch
"fix" PRs that rewrite whole files from stale bases and silently drop
functionality:

- `scripts/check-truncation.sh` — fails when any file under `contracts/`
  contains a truncation marker such as `/* … truncated 1234 chars */`.
- `scripts/check-api-surface.sh` — extracts `pub fn` names inside
  `#[contractimpl]` blocks from the base branch and the PR branch and
  fails when a public function is removed. A PR may bypass this by
  adding the `breaking-change` label.
- `scripts/check-test-count.sh` — fails when `cargo test -p <crate>`
  reports `0 passed` for a crate that has a `test.rs`.

## PR template

Every PR must confirm the checkbox: "This PR changes only the code
needed for the referenced issue."
