## Summary

<!-- What does this PR change and why? -->

## Changelog

- [ ] I have updated `CHANGELOG.md` for any user-facing contract API change (new functions, breaking changes, deprecations, or security fixes).

## Scope

- [ ] This PR changes only the code needed for the referenced issue.
- [ ] This PR does not replace whole files from a stale base; existing public functions and unrelated functionality are preserved.
- [ ] If any public function was intentionally removed, this PR carries the `breaking-change` label.

## Test plan

- [ ] Unit / integration tests added or updated
- [ ] `cargo test` passes locally (if applicable)
- [ ] `cargo check --workspace --tests` passes
- [ ] `cargo build --target wasm32v1-none --release --workspace` passes
